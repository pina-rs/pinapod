use {
    crate::type_map::{
        classify_compact_field, classify_field, validate_dynamic_prefix_args, FieldKind,
    },
    proc_macro2::TokenStream,
    quote::quote,
    syn::{DeriveInput, Fields},
};

pub struct Schema {
    pub name: syn::Ident,
    pub vis: syn::Visibility,
    pub generics: syn::Generics,
    pub fields: Vec<SchemaField>,
    pub is_compact: bool,
    pub no_inherent: bool,
}

#[derive(Clone, Debug, Default)]
pub struct LayoutOptions {
    pub is_compact: bool,
    pub no_inherent: bool,
    pub crate_path: Option<syn::Path>,
}

pub struct SchemaField {
    pub name: syn::Ident,
    pub ty: syn::Type,
    pub kind: FieldKind,
    pub vis: syn::Visibility,
    pub skip_accessor: bool,
    /// Keep this field in storage and validation, but omit it from generated
    /// compact patch APIs. Frameworks use this for owned metadata such as an
    /// account discriminator.
    pub skip_patch: bool,
    /// Preserve `#[pinapod(...)]` attributes for pass-through.
    #[allow(dead_code)]
    pub pinapod_attrs: Vec<syn::Attribute>,
}

impl Schema {
    #[allow(
        dead_code,
        reason = "unit tests exercise the convenience parser directly"
    )]
    pub fn parse(input: &DeriveInput) -> Result<Schema, TokenStream> {
        let options = parse_layout(&input.attrs).map_err(|error| error.to_compile_error())?;

        Self::parse_with_options(input, &options)
    }

    pub fn parse_with_options(
        input: &DeriveInput,
        options: &LayoutOptions,
    ) -> Result<Schema, TokenStream> {
        // Only structs with named fields.
        let named_fields = match &input.data {
            syn::Data::Struct(data) => match &data.fields {
                Fields::Named(named) => &named.named,
                _ => {
                    return Err(
                        quote! { compile_error!("PinaPod only supports structs with named fields"); },
                    );
                }
            },
            _ => {
                return Err(
                    quote! { compile_error!("PinaPod Schema::parse only accepts structs"); },
                );
            }
        };

        let is_compact = options.is_compact;

        // Classify fields.
        let fields: Vec<SchemaField> = named_fields
            .iter()
            .map(|f| {
                let name = f.ident.clone().expect("named field must have ident");
                let ty = f.ty.clone();
                validate_dynamic_prefix_args(&ty)?;
                let kind = if is_compact {
                    classify_compact_field(&ty)?
                } else {
                    classify_field(&ty)
                };
                let (skip_accessor, skip_patch, pinapod_attrs) =
                    parse_pinapod_field_attrs(&f.attrs)?;
                Ok(SchemaField {
                    name,
                    ty,
                    kind,
                    vis: f.vis.clone(),
                    skip_accessor,
                    skip_patch,
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
            vis: input.vis.clone(),
            generics: input.generics.clone(),
            fields,
            is_compact,
            no_inherent: options.no_inherent,
        })
    }
}

/// Parse `#[pinapod(...)]` attributes on a field.
/// Returns (`skip_accessor`, `skip_patch`, `pinapod_attrs_to_preserve`).
fn parse_pinapod_field_attrs(
    attrs: &[syn::Attribute],
) -> Result<(bool, bool, Vec<syn::Attribute>), TokenStream> {
    let mut skip_accessor = false;
    let mut skip_patch = false;
    let mut pinapod_attrs = Vec::new();

    for attr in attrs {
        if !attr.path().is_ident("pinapod") {
            continue;
        }

        pinapod_attrs.push(attr.clone());

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip_accessor") {
                if skip_accessor {
                    return Err(meta.error("duplicate `skip_accessor` option"));
                }

                if meta.input.peek(syn::Token![=]) || meta.input.peek(syn::token::Paren) {
                    return Err(meta.error("`skip_accessor` does not accept a value"));
                }

                skip_accessor = true;

                return Ok(());
            }

            if meta.path.is_ident("skip_patch") {
                if skip_patch {
                    return Err(meta.error("duplicate `skip_patch` option"));
                }

                if meta.input.peek(syn::Token![=]) || meta.input.peek(syn::token::Paren) {
                    return Err(meta.error("`skip_patch` does not accept a value"));
                }

                skip_patch = true;

                return Ok(());
            }

            if meta.path.is_ident("prefix") {
                return Err(meta.error(
                    "prefix width belongs in the field type; use `PodString<N, PFX>` or `PodVec<T, N, PFX>`",
                ));
            }

            Err(meta.error(
                "unsupported PinaPod field option; expected `skip_accessor` or `skip_patch`",
            ))
        })
        .map_err(|error| error.to_compile_error())?;
    }

    Ok((skip_accessor, skip_patch, pinapod_attrs))
}

pub(crate) fn parse_layout(attrs: &[syn::Attribute]) -> syn::Result<LayoutOptions> {
    let mut options = LayoutOptions::default();

    for attr in attrs {
        if !attr.path().is_ident("pinapod") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("compact") {
                if options.is_compact {
                    return Err(meta.error("duplicate `compact` option"));
                }
                if meta.input.peek(syn::Token![=]) || meta.input.peek(syn::token::Paren) {
                    return Err(meta.error("`compact` does not accept a value"));
                }

                options.is_compact = true;
                return Ok(());
            }

            if meta.path.is_ident("no_inherent") {
                if options.no_inherent {
                    return Err(meta.error("duplicate `no_inherent` option"));
                }
                if meta.input.peek(syn::Token![=]) || meta.input.peek(syn::token::Paren) {
                    return Err(meta.error("`no_inherent` does not accept a value"));
                }

                options.no_inherent = true;
                return Ok(());
            }

            if meta.path.is_ident("crate") {
                if options.crate_path.is_some() {
                    return Err(meta.error("duplicate `crate` option"));
                }

                options.crate_path = Some(meta.value()?.parse()?);
                return Ok(());
            }

            Err(meta.error(
                "unsupported PinaPod option; expected `compact`, `no_inherent`, or `crate = path`",
            ))
        })?;
    }

    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_field_attributes_and_preserves_pinapod_attributes() {
        let input: DeriveInput = syn::parse_quote! {
            struct AttributeFixture {
                #[doc = "A non-PinaPod attribute exercises the filtering path."]
                #[pinapod(skip_accessor, skip_patch)]
                hidden: u8,
            }
        };

        let schema = Schema::parse(&input).unwrap();
        assert!(schema.fields[0].skip_accessor);
        assert!(schema.fields[0].skip_patch);
        assert_eq!(schema.fields[0].pinapod_attrs.len(), 1);
    }

    #[test]
    fn rejects_duplicate_and_valued_skip_patch_options() {
        let duplicate: DeriveInput = syn::parse_quote! {
            struct Duplicate {
                #[pinapod(skip_patch, skip_patch)]
                discriminator: [u8; 8],
            }
        };
        let valued: DeriveInput = syn::parse_quote! {
            struct Valued {
                #[pinapod(skip_patch = true)]
                discriminator: [u8; 8],
            }
        };

        let duplicate_error = Schema::parse(&duplicate)
            .err()
            .expect("duplicate skip_patch should be rejected")
            .to_string();
        let valued_error = Schema::parse(&valued)
            .err()
            .expect("valued skip_patch should be rejected")
            .to_string();

        assert!(duplicate_error.contains("duplicate `skip_patch` option"));
        assert!(valued_error.contains("`skip_patch` does not accept a value"));
    }

    #[test]
    fn parses_framework_options_strictly() {
        let input: DeriveInput = syn::parse_quote! {
            #[pinapod(compact, no_inherent, crate = pina::pinapod)]
            struct FrameworkSchema {
                value: u8,
            }
        };

        let options = parse_layout(&input.attrs).unwrap();

        assert!(options.is_compact);
        assert!(options.no_inherent);
        assert_eq!(
            options.crate_path.unwrap(),
            syn::parse_quote!(pina::pinapod)
        );
    }

    #[test]
    fn rejects_prefix_attributes_with_a_type_directed_remedy() {
        let input: DeriveInput = syn::parse_quote! {
            struct AttributeFixture {
                #[pinapod(prefix = u16)]
                values: pinapod::Vec<u64, 1024>,
            }
        };

        let error = Schema::parse(&input)
            .err()
            .expect("prefix attributes should be rejected")
            .to_string();

        assert!(error.contains("prefix width belongs in the field type"));
        assert!(error.contains("PodVec<T, N, PFX>"));
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
