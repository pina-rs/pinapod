use {
    crate::type_map::{classify_field, validate_dynamic_prefix_args, FieldKind},
    proc_macro2::TokenStream,
    quote::quote,
    syn::{DeriveInput, Fields},
};

pub struct Schema {
    pub name: syn::Ident,
    pub generics: syn::Generics,
    pub fields: Vec<SchemaField>,
    pub is_compact: bool,
}

pub struct SchemaField {
    pub name: syn::Ident,
    pub ty: syn::Type,
    pub kind: FieldKind,
    pub vis: syn::Visibility,
    pub skip_accessor: bool,
    /// Preserve `#[pinapod(...)]` attributes for pass-through.
    #[allow(dead_code)]
    pub pinapod_attrs: Vec<syn::Attribute>,
}

impl Schema {
    pub fn parse(input: &DeriveInput) -> Result<Schema, TokenStream> {
        // Only structs with named fields.
        let named_fields = match &input.data {
            syn::Data::Struct(data) => match &data.fields {
                Fields::Named(named) => &named.named,
                _ => {
                    return Err(
                        quote! { compile_error!("ZeroPod only supports structs with named fields"); },
                    );
                }
            },
            _ => {
                return Err(
                    quote! { compile_error!("ZeroPod Schema::parse only accepts structs"); },
                );
            }
        };

        // Check for #[pinapod(compact)] attribute.
        let is_compact = input.attrs.iter().any(|attr| {
            if !attr.path().is_ident("pinapod") {
                return false;
            }
            let mut found = false;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("compact") {
                    found = true;
                }
                Ok(())
            });
            found
        });

        // Classify fields.
        let fields: Vec<SchemaField> = named_fields
            .iter()
            .map(|f| {
                let name = f.ident.clone().expect("named field must have ident");
                let ty = f.ty.clone();
                validate_dynamic_prefix_args(&ty)?;
                let kind = classify_field(&ty);
                let (skip_accessor, pinapod_attrs) = parse_pinapod_field_attrs(&f.attrs);
                Ok(SchemaField {
                    name,
                    ty,
                    kind,
                    vis: f.vis.clone(),
                    skip_accessor,
                    pinapod_attrs,
                })
            })
            .collect::<Result<_, TokenStream>>()?;

        // Enforce suffix-only rule for compact mode:
        // once a tail field appears, no inline fields may follow.
        if is_compact {
            let mut seen_tail = false;
            let mut first_tail_name: Option<&syn::Ident> = None;
            for f in &fields {
                match &f.kind {
                    FieldKind::Tail(_) => {
                        if !seen_tail {
                            seen_tail = true;
                            first_tail_name = Some(&f.name);
                        }
                    }
                    FieldKind::Inline => {
                        if seen_tail {
                            let inline_name = &f.name;
                            let tail_name = first_tail_name.unwrap();
                            let msg = format!(
                                "inline field `{inline_name}` cannot come after tail field \
                                 `{tail_name}` in compact mode"
                            );
                            return Err(quote! { compile_error!(#msg); });
                        }
                    }
                }
            }
        }

        Ok(Schema {
            name: input.ident.clone(),
            generics: input.generics.clone(),
            fields,
            is_compact,
        })
    }
}

/// Parse `#[pinapod(...)]` attributes on a field.
/// Returns (`skip_accessor`, `pinapod_attrs_to_preserve`).
fn parse_pinapod_field_attrs(attrs: &[syn::Attribute]) -> (bool, Vec<syn::Attribute>) {
    let mut skip_accessor = false;
    let mut pinapod_attrs = Vec::new();

    for attr in attrs {
        if !attr.path().is_ident("pinapod") {
            continue;
        }
        pinapod_attrs.push(attr.clone());
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip_accessor") {
                skip_accessor = true;
            }
            Ok(())
        });
    }

    (skip_accessor, pinapod_attrs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_field_attributes_and_preserves_pinapod_attributes() {
        let input: DeriveInput = syn::parse_quote! {
            struct AttributeFixture {
                #[doc = "A non-Pinapod attribute exercises the filtering path."]
                #[pinapod(skip_accessor)]
                hidden: u8,
            }
        };

        let schema = Schema::parse(&input).unwrap();
        assert!(schema.fields[0].skip_accessor);
        assert_eq!(schema.fields[0].pinapod_attrs.len(), 1);
    }

    #[test]
    fn rejects_an_invalid_dynamic_prefix_at_the_field() {
        let input: DeriveInput = syn::parse_quote! {
            #[pinapod(compact)]
            struct InvalidPrefix {
                values: pinapod::PodVec<u8, 8, 3>,
            }
        };

        let error = Schema::parse(&input)
            .err()
            .expect("invalid prefix should be rejected")
            .to_string();

        assert!(error.contains("PodVec length prefix must be"));
    }
}

impl Schema {
    pub fn inline_fields(&self) -> impl Iterator<Item = &SchemaField> {
        self.fields
            .iter()
            .filter(|f| matches!(f.kind, FieldKind::Inline))
    }

    pub fn tail_fields(&self) -> impl Iterator<Item = &SchemaField> {
        self.fields
            .iter()
            .filter(|f| matches!(f.kind, FieldKind::Tail(_)))
    }
}
