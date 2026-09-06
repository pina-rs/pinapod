#![allow(
    clippy::manual_let_else,
    reason = "the upstream-compatible parser reads more clearly as nested matches"
)]

use {
    proc_macro2::TokenStream,
    quote::quote,
    syn::{Expr, ExprLit, GenericArgument, Lit, PathArguments, Type},
};

// ---------------------------------------------------------------------------
// Field classification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum FieldKind {
    Inline,
    Tail(TailField),
}

#[derive(Debug, Clone)]
pub enum TailField {
    Segment {
        presence: TailPresence,
        payload: TailPayload,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TailPresence {
    Always,
    OptionTag,
}

#[derive(Debug, Clone)]
pub enum TailPayload {
    String {
        max: Expr,
        pfx: usize,
    },
    Vec {
        elem: Box<Type>,
        max: Expr,
        pfx: usize,
    },
}

pub fn classify_field(ty: &Type) -> FieldKind {
    if let Some(tail) = classify_option_dynamic(ty) {
        return FieldKind::Tail(tail);
    }
    if let Some(tail) = classify_string(ty) {
        return FieldKind::Tail(tail);
    }
    if let Some(tail) = classify_vec(ty) {
        return FieldKind::Tail(tail);
    }
    FieldKind::Inline
}

pub fn validate_dynamic_prefix_args(ty: &Type) -> Result<(), TokenStream> {
    let Some(segment) = last_path_segment(ty) else {
        return Ok(());
    };
    let Some(args) = angle_args(&segment.arguments) else {
        return Ok(());
    };

    let prefix_index = match segment.ident.to_string().as_str() {
        "String" | "PodString" => Some(1),
        "Vec" | "PodVec" => Some(2),
        _ => None,
    };
    if let Some(prefix) = prefix_index.and_then(|index| args.iter().nth(index)) {
        if parse_prefix_arg(prefix).is_none() {
            let message = format!(
                "{} length prefix must be u8, u16, u32, u64, or the equivalent byte width 1, 2, 4, or 8",
                segment.ident
            );
            return Err(syn::Error::new_spanned(prefix, message).to_compile_error());
        }
    }

    for argument in args {
        if let GenericArgument::Type(inner) = argument {
            validate_dynamic_prefix_args(inner)?;
        }
    }

    Ok(())
}

fn classify_string(ty: &Type) -> Option<TailField> {
    let seg = last_path_segment(ty)?;
    if seg.ident != "String" && seg.ident != "PodString" {
        return None;
    }
    let args = angle_args(&seg.arguments)?;
    let mut iter = args.iter();
    let max = extract_const_expr(iter.next()?)?;
    let pfx = iter.next().and_then(parse_prefix_arg).unwrap_or(1);
    Some(TailField::Segment {
        presence: TailPresence::Always,
        payload: TailPayload::String { max, pfx },
    })
}

fn classify_vec(ty: &Type) -> Option<TailField> {
    let seg = last_path_segment(ty)?;
    if seg.ident != "Vec" && seg.ident != "PodVec" {
        return None;
    }
    let args = angle_args(&seg.arguments)?;
    let mut iter = args.iter();
    let elem = match iter.next()? {
        GenericArgument::Type(t) => t.clone(),
        _ => return None,
    };
    let max = extract_const_expr(iter.next()?)?;
    let pfx = iter.next().and_then(parse_prefix_arg).unwrap_or(2);
    Some(TailField::Segment {
        presence: TailPresence::Always,
        payload: TailPayload::Vec {
            elem: Box::new(elem),
            max,
            pfx,
        },
    })
}

fn classify_option_dynamic(ty: &Type) -> Option<TailField> {
    let seg = last_path_segment(ty)?;
    if seg.ident != "Option" {
        return None;
    }
    let args = angle_args(&seg.arguments)?;
    let inner = match args.first()? {
        GenericArgument::Type(t) => t,
        _ => return None,
    };
    if let Some(TailField::Segment {
        payload: TailPayload::String { max, pfx },
        ..
    }) = classify_string(inner)
    {
        return Some(TailField::Segment {
            presence: TailPresence::OptionTag,
            payload: TailPayload::String { max, pfx },
        });
    }
    if let Some(TailField::Segment {
        payload: TailPayload::Vec { elem, max, pfx },
        ..
    }) = classify_vec(inner)
    {
        return Some(TailField::Segment {
            presence: TailPresence::OptionTag,
            payload: TailPayload::Vec { elem, max, pfx },
        });
    }
    None
}

impl TailField {
    pub fn presence(&self) -> TailPresence {
        match self {
            Self::Segment { presence, .. } => *presence,
        }
    }

    pub fn payload(&self) -> &TailPayload {
        match self {
            Self::Segment { payload, .. } => payload,
        }
    }
}

impl TailPayload {
    pub fn pfx(&self) -> usize {
        match self {
            Self::String { pfx, .. } | Self::Vec { pfx, .. } => *pfx,
        }
    }
}

// ---------------------------------------------------------------------------
// Type mapping: schema type → pod storage type
// ---------------------------------------------------------------------------

pub fn map_to_pod_type(ty: &Type) -> TokenStream {
    // 1. Primitives that are already align-1
    if let Some(ts) = try_primitive(ty) {
        return ts;
    }

    // 2. String / PodString
    if let Some(ts) = try_map_string(ty) {
        return ts;
    }

    // 3. Vec / PodVec
    if let Some(ts) = try_map_vec(ty) {
        return ts;
    }

    // 4. PodOption (must check before Option to avoid matching PodOption as generic)
    if let Some(ts) = try_map_pod_option(ty) {
        return ts;
    }

    // 5. Option
    if let Some(ts) = try_map_option(ty) {
        return ts;
    }

    // 6. Array types → keep as-is
    if matches!(ty, Type::Array(_)) {
        return quote! { #ty };
    }

    // 7. Fallback: delegate via ZcField trait
    quote! { <#ty as pinapod::ZcField>::Pod }
}

fn try_primitive(ty: &Type) -> Option<TokenStream> {
    let seg = single_path_segment(ty)?;
    if !seg.arguments.is_none() {
        return None;
    }
    let name = seg.ident.to_string();
    match name.as_str() {
        "u8" => Some(quote! { u8 }),
        "i8" => Some(quote! { i8 }),
        "u16" => Some(quote! { pinapod::pod::PodU16 }),
        "u32" => Some(quote! { pinapod::pod::PodU32 }),
        "u64" => Some(quote! { pinapod::pod::PodU64 }),
        "u128" => Some(quote! { pinapod::pod::PodU128 }),
        "i16" => Some(quote! { pinapod::pod::PodI16 }),
        "i32" => Some(quote! { pinapod::pod::PodI32 }),
        "i64" => Some(quote! { pinapod::pod::PodI64 }),
        "i128" => Some(quote! { pinapod::pod::PodI128 }),
        "bool" => Some(quote! { pinapod::pod::PodBool }),
        _ => None,
    }
}

fn try_map_string(ty: &Type) -> Option<TokenStream> {
    let seg = last_path_segment(ty)?;
    if seg.ident != "String" && seg.ident != "PodString" {
        return None;
    }
    let args = angle_args(&seg.arguments)?;
    let mut iter = args.iter();
    let n_arg = iter.next()?;
    let pfx: usize = iter.next().and_then(parse_prefix_arg).unwrap_or(1);
    Some(quote! { pinapod::pod::PodString<#n_arg, #pfx> })
}

fn try_map_vec(ty: &Type) -> Option<TokenStream> {
    let seg = last_path_segment(ty)?;
    if seg.ident != "Vec" && seg.ident != "PodVec" {
        return None;
    }
    let args = angle_args(&seg.arguments)?;
    let mut iter = args.iter();
    let t_arg = match iter.next()? {
        GenericArgument::Type(t) => t,
        _ => return None,
    };
    let n_arg = iter.next()?;
    let pfx: usize = iter.next().and_then(parse_prefix_arg).unwrap_or(2);
    let mapped_t = map_to_pod_type(t_arg);
    Some(quote! { pinapod::pod::PodVec<#mapped_t, #n_arg, #pfx> })
}

fn try_map_option(ty: &Type) -> Option<TokenStream> {
    let seg = last_path_segment(ty)?;
    if seg.ident != "Option" {
        return None;
    }
    let args = angle_args(&seg.arguments)?;
    let inner = match args.first()? {
        GenericArgument::Type(t) => t,
        _ => return None,
    };
    let mapped_inner = map_to_pod_type(inner);
    Some(quote! { pinapod::pod::PodOption<#mapped_inner> })
}

fn try_map_pod_option(ty: &Type) -> Option<TokenStream> {
    let seg = last_path_segment(ty)?;
    if seg.ident != "PodOption" {
        return None;
    }
    let args = angle_args(&seg.arguments)?;
    let mut iter = args.iter();
    let inner = match iter.next()? {
        GenericArgument::Type(t) => t,
        _ => return None,
    };
    let mapped_inner = map_to_pod_type(inner);
    // Pass through PFX if present.
    let pfx = iter.next();
    match pfx {
        Some(pfx_arg) => Some(quote! { pinapod::pod::PodOption<#mapped_inner, #pfx_arg> }),
        None => Some(quote! { pinapod::pod::PodOption<#mapped_inner> }),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn single_path_segment(ty: &Type) -> Option<&syn::PathSegment> {
    if let Type::Path(type_path) = ty {
        if type_path.path.segments.len() == 1 {
            return type_path.path.segments.last();
        }
    }
    None
}

/// Like `single_path_segment`, but also matches the *last* segment
/// of a multi-segment path (e.g. `pinapod::String<32>` → `String<32>`).
fn last_path_segment(ty: &Type) -> Option<&syn::PathSegment> {
    if let Type::Path(type_path) = ty {
        return type_path.path.segments.last();
    }
    None
}

fn angle_args(
    arguments: &PathArguments,
) -> Option<&syn::punctuated::Punctuated<GenericArgument, syn::token::Comma>> {
    if let PathArguments::AngleBracketed(ab) = arguments {
        Some(&ab.args)
    } else {
        None
    }
}

fn extract_const_expr(arg: &GenericArgument) -> Option<Expr> {
    match arg {
        GenericArgument::Const(expr) => Some(expr.clone()),
        GenericArgument::Type(Type::Path(type_path))
            if type_path.qself.is_none()
                && type_path.path.leading_colon.is_none()
                && type_path.path.segments.len() == 1 =>
        {
            let ident = &type_path.path.segments[0].ident;
            Some(syn::parse_quote!(#ident))
        }
        _ => None,
    }
}

fn parse_prefix_arg(arg: &GenericArgument) -> Option<usize> {
    match arg {
        GenericArgument::Type(Type::Path(type_path)) => {
            let seg = type_path.path.segments.last()?;
            match seg.ident.to_string().as_str() {
                "u8" => Some(1),
                "u16" => Some(2),
                "u32" => Some(4),
                "u64" => Some(8),
                _ => None,
            }
        }
        GenericArgument::Const(Expr::Lit(ExprLit {
            lit: Lit::Int(n), ..
        })) => match n.base10_parse::<usize>().ok()? {
            prefix @ (1 | 2 | 4 | 8) => Some(prefix),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec_mapping_rejects_a_non_type_element_argument() {
        let ty: Type = syn::parse_quote!(Vec<3, 4>);
        assert!(try_map_vec(&ty).is_none());
    }

    #[test]
    fn pod_option_mapping_uses_the_default_prefix_when_omitted() {
        let ty: Type = syn::parse_quote!(PodOption<u64>);
        let mapped = map_to_pod_type(&ty);

        assert_eq!(
            mapped.to_string(),
            quote!(pinapod::pod::PodOption<pinapod::pod::PodU64>).to_string()
        );
    }

    #[test]
    fn prefix_mapping_accepts_supported_types_and_widths() {
        let u8_type: GenericArgument = syn::parse_quote!(u8);
        let u16_type: GenericArgument = syn::parse_quote!(u16);
        let u32_type: GenericArgument = syn::parse_quote!(u32);
        let u64_type: GenericArgument = syn::parse_quote!(u64);
        let one: GenericArgument = syn::parse_quote!(1);
        let two: GenericArgument = syn::parse_quote!(2);
        let four: GenericArgument = syn::parse_quote!(4);
        let eight: GenericArgument = syn::parse_quote!(8);

        assert_eq!(parse_prefix_arg(&u8_type), Some(1));
        assert_eq!(parse_prefix_arg(&u16_type), Some(2));
        assert_eq!(parse_prefix_arg(&u32_type), Some(4));
        assert_eq!(parse_prefix_arg(&u64_type), Some(8));
        assert_eq!(parse_prefix_arg(&one), Some(1));
        assert_eq!(parse_prefix_arg(&two), Some(2));
        assert_eq!(parse_prefix_arg(&four), Some(4));
        assert_eq!(parse_prefix_arg(&eight), Some(8));
    }

    #[test]
    fn dynamic_prefix_validation_rejects_unsupported_widths() {
        let string: Type = syn::parse_quote!(pinapod::PodString<32, 3>);
        let vector: Type = syn::parse_quote!(Option<pinapod::PodVec<u8, 16, 0>>);

        let string_error = validate_dynamic_prefix_args(&string)
            .unwrap_err()
            .to_string();
        let vector_error = validate_dynamic_prefix_args(&vector)
            .unwrap_err()
            .to_string();

        assert!(string_error.contains("PodString length prefix must be"));
        assert!(vector_error.contains("PodVec length prefix must be"));
    }

    #[test]
    fn dynamic_prefix_validation_accepts_supported_nested_widths() {
        let ty: Type = syn::parse_quote!(Option<pinapod::PodVec<pinapod::PodString<32, 1>, 16, 8>>);

        assert!(validate_dynamic_prefix_args(&ty).is_ok());
    }
}
