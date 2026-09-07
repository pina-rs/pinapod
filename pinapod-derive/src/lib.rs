#![allow(
    clippy::match_wildcard_for_single_variants,
    reason = "the wildcard preserves a concise fallback for future syn data variants"
)]

use {
    proc_macro::TokenStream,
    proc_macro2::{Group, TokenStream as TokenStream2, TokenTree},
    proc_macro_crate::{crate_name, FoundCrate},
    quote::{format_ident, quote},
    syn::{parse_macro_input, DeriveInput},
};

mod compact;
mod compact_enum;
mod fixed;
mod schema;
mod type_map;

#[proc_macro_derive(PinaPod, attributes(pinapod))]
pub fn derive_pina_pod(input: TokenStream) -> TokenStream {
    derive(input)
}

fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let options = match schema::parse_layout(&input.attrs) {
        Ok(options) => options,
        Err(error) => return error.to_compile_error().into(),
    };
    let is_compact = options.is_compact;
    let crate_path = match resolve_pinapod_path(options.crate_path.as_ref()) {
        Ok(path) => path,
        Err(error) => return error.to_compile_error().into(),
    };

    let output = match &input.data {
        syn::Data::Enum(_) => {
            if options.no_inherent {
                return syn::Error::new_spanned(
                    &input.ident,
                    "`no_inherent` is only supported on PinaPod structs",
                )
                .to_compile_error()
                .into();
            }

            if let Err(error) = validate_enum_attributes(&input, is_compact) {
                return error.to_compile_error().into();
            }

            if is_compact {
                compact_enum::generate(&input)
            } else {
                fixed::generate_enum(&input)
            }
        }
        syn::Data::Struct(_) => {
            let schema = match schema::Schema::parse_with_options(&input, &options) {
                Ok(s) => s,
                Err(e) => return e.into(),
            };
            if schema.is_compact {
                compact::generate(&schema)
            } else {
                fixed::generate(&schema)
            }
        }
        _ => {
            let msg = "PinaPod only supports structs and enums";
            return quote::quote! { compile_error!(#msg); }.into();
        }
    };

    replace_pinapod_paths(output, &crate_path).into()
}

fn resolve_pinapod_path(explicit: Option<&syn::Path>) -> syn::Result<TokenStream2> {
    if let Some(path) = explicit {
        return Ok(quote! { #path });
    }

    match crate_name("pinapod") {
        Ok(FoundCrate::Itself) => Ok(quote! { crate }),
        Ok(FoundCrate::Name(name)) => {
            let ident = format_ident!("{}", name.replace('-', "_"));
            Ok(quote! { ::#ident })
        }
        Err(error) => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!(
                "could not resolve the `pinapod` crate: {error}; reexports must set `#[pinapod(crate = path::to::pinapod)]`"
            ),
        )),
    }
}

/// Replace leading generated `pinapod::` paths after generation.
///
/// User-written nested paths such as `pina::pinapod::String` remain intact;
/// only an identifier that begins a path is replaced. Keeping this as a final
/// token pass lets the fixed and compact generators share one resolution rule.
fn replace_pinapod_paths(input: TokenStream2, replacement: &TokenStream2) -> TokenStream2 {
    let mut output = TokenStream2::new();
    let mut preceding_colons = 0_u8;

    for token in input {
        match token {
            TokenTree::Ident(ident) if ident == "pinapod" && preceding_colons < 2 => {
                output.extend(replacement.clone());
                preceding_colons = 0;
            }
            TokenTree::Group(group) => {
                let mut replaced = Group::new(
                    group.delimiter(),
                    replace_pinapod_paths(group.stream(), replacement),
                );
                replaced.set_span(group.span());
                output.extend([TokenTree::Group(replaced)]);
                preceding_colons = 0;
            }
            token => {
                preceding_colons = match &token {
                    TokenTree::Punct(punct) if punct.as_char() == ':' => {
                        preceding_colons.saturating_add(1)
                    }
                    _ => 0,
                };
                output.extend([token]);
            }
        }
    }

    output
}

fn validate_enum_attributes(input: &DeriveInput, is_compact: bool) -> syn::Result<()> {
    let syn::Data::Enum(data) = &input.data else {
        return Ok(());
    };

    for variant in &data.variants {
        let mut seen_compact = false;

        for attr in &variant.attrs {
            if !attr.path().is_ident("pinapod") {
                continue;
            }

            attr.parse_nested_meta(|meta| {
                if !meta.path.is_ident("compact") {
                    return Err(meta.error(
                        "unsupported PinaPod enum-variant option; only `compact` is supported",
                    ));
                }

                if !is_compact {
                    return Err(meta.error(
                        "`compact` payloads are only supported on variants of a compact PinaPod enum",
                    ));
                }

                if seen_compact {
                    return Err(meta.error("duplicate `compact` option"));
                }

                if meta.input.peek(syn::Token![=]) || meta.input.peek(syn::token::Paren) {
                    return Err(meta.error("`compact` does not accept a value"));
                }

                seen_compact = true;
                Ok(())
            })?;
        }

        for field in &variant.fields {
            if let Some(attr) = field
                .attrs
                .iter()
                .find(|attr| attr.path().is_ident("pinapod"))
            {
                return Err(syn::Error::new_spanned(
                    attr,
                    "PinaPod attributes are not supported on enum payload fields",
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_parser_rejects_unknown_and_duplicate_options() {
        let unknown: DeriveInput = syn::parse_quote! {
            #[pinapod(prefix = u16)]
            struct Invalid { value: u8 }
        };
        let duplicate: DeriveInput = syn::parse_quote! {
            #[pinapod(compact, compact)]
            struct Invalid { value: u8 }
        };

        let unknown_error = schema::parse_layout(&unknown.attrs)
            .unwrap_err()
            .to_string();
        let duplicate_error = schema::parse_layout(&duplicate.attrs)
            .unwrap_err()
            .to_string();

        assert!(unknown_error.contains("expected `compact`, `no_inherent`, or `crate = path`"));
        assert!(duplicate_error.contains("duplicate `compact` option"));
    }

    #[test]
    fn generated_crate_paths_are_replaced_without_touching_nested_reexports() {
        let input = quote! {
            pinapod::PinaPod;
            <pinapod::String<8> as pinapod::ZcField>::Pod;
            pina::pinapod::String<8>;
        };
        let replacement = quote! { ::renamed_pinapod };
        let output = replace_pinapod_paths(input, &replacement).to_string();

        assert!(output.contains(":: renamed_pinapod :: PinaPod"));
        assert!(output.contains("as :: renamed_pinapod :: ZcField"));
        assert!(output.contains("pina :: pinapod :: String"));
    }
}
