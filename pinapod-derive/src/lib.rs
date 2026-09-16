//! Derive macros for `pinapod`.
//!
//! This crate provides `#[derive(PinaPod)]`, which generates the alignment-one storage
//! companion, the reader, and the writer for a schema. Use it through the `pinapod`
//! crate; the derive is re-exported there and expands paths against the dependency that
//! is actually in scope.
//!
//! # Layouts
//!
//! A derive without arguments generates a fixed schema whose size is known at compile
//! time. `#[pinapod(compact)]` generates a compact schema instead: fixed fields stay in a
//! header while string and vector payloads move to dynamic tails, so the allocation
//! tracks active data.
//!
//! ```ignore
//! use pinapod::PinaPod;
//!
//! #[derive(PinaPod)]
//! struct Fixed {
//!     authority: [u8; 32],
//!     amount: u64,
//! }
//!
//! #[derive(PinaPod)]
//! #[pinapod(compact)]
//! struct Compact {
//!     authority: [u8; 32],
//!     note: pinapod::String<128>,
//! }
//! ```
//!
//! # Container fields
//!
//! A field is treated as a dynamic container when it is spelled `String<..>`, `Vec<..>`,
//! or `Option<..>` as a single unqualified segment, or when the segment before the name
//! is literally `pinapod`, `pinapod::pod`, or a `pina` re-export. Every other type must
//! provide its own `ZcField` mapping.
//!
//! The check compares path segments, so a module of yours that is itself named `pinapod`
//! or `pina` is read as the `PinaPod` containers rather than as your types. Avoid module
//! names that shadow the dependency, or spell such a field through an unambiguous path.
//! A type name alone never grants a built-in representation.
//!
//! Prefix width belongs in the field type, not in an attribute:
//!
//! ```ignore
//! use pinapod::PinaPod;
//!
//! #[derive(PinaPod)]
//! struct Archive {
//!     label: pinapod::PodString<300, 2>,
//!     values: pinapod::PodVec<u64, 1024, 2>,
//! }
//! ```
//!
//! # Errors
//!
//! Every unsupported declaration is a compile error rather than a silent fallback. The
//! derive rejects a wrong layout for a type, a capacity that cannot fit its prefix, an
//! unsupported nesting inside a compact tail, and a caller-local type that shadows a
//! primitive's name.

// Generated code lands in downstream crates, so the derive's own surface has to
// document itself for those crates to keep `missing_docs` enabled.
#![deny(missing_docs)]
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

/// Generates the `PinaPod` storage, reader, and writer for a struct or enum.
///
/// # Struct options
///
/// Options are passed as arguments to the `pinapod` attribute, for example
/// `#[pinapod(compact)]`.
///
/// - `compact` — generate a compact schema with a fixed header and dynamic tails.
///   Without it, the schema is fixed and occupies `Type::SIZE` bytes.
/// - `no_inherent` — suppress the inherent `impl` block, so the schema is usable only
///   through the `PinaPodFixed` or `PinaPodCompact` trait constants and methods.
///   Framework code sets this when it owns the inherent surface. It is rejected on enums.
/// - `crate = path::to::pinapod` — override the resolved `pinapod` path. Re-exporting
///   crates set this instead of relying on dependency-name resolution.
///
/// # Field options
///
/// - `skip_accessor` — keep the field in storage and validation, but generate no
///   accessor method for it.
/// - `skip_patch` — keep the field in storage and validation, but omit it from the
///   compact patch API. Use it for framework-owned metadata such as a discriminator.
///
/// `prefix` is not a field option. Prefix width belongs in the field type, so
/// `#[pinapod(prefix = u16)]` is rejected; write `pinapod::PodString<N, 2>` instead.
///
/// # Generated surface
///
/// The derive generates an inherent `impl` with schema helpers, plus the `pinapod` trait
/// implementations for the chosen layout. A fixed schema gets a `SIZE` constant and
/// forwarding read, validate, and initialize methods; a compact schema gets the size
/// constants and its patch methods. The helpers forward to the trait, so a schema works
/// with or without the trait in scope.
///
/// `no_inherent` removes the helpers while keeping the trait contract. Framework crates
/// set it when they own the inherent surface for a schema.
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
