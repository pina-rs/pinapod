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

/// Classify a compact field while rejecting dynamic shapes whose wire layout
/// is not part of the supported grammar.
pub fn classify_compact_field(ty: &Type) -> Result<FieldKind, TokenStream> {
    match recognized_dynamic_name(ty) {
        None => Ok(FieldKind::Inline),
        Some(DynamicName::String) => classify_string_checked(ty).map(FieldKind::Tail),
        Some(DynamicName::Vec) => classify_vec_checked(ty).map(FieldKind::Tail),
        Some(DynamicName::Option) => classify_option_checked(ty),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DynamicName {
    String,
    Vec,
    Option,
}

fn recognized_dynamic_name(ty: &Type) -> Option<DynamicName> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    if type_path.qself.is_some() {
        return None;
    }
    let segments: Vec<_> = type_path.path.segments.iter().collect();
    let name = segments.last()?;
    let namespaced_by_pinapod =
        segments.len() >= 2 && segments[segments.len() - 2].ident == "pinapod";
    let namespaced_by_pinapod_pod = segments.len() >= 3
        && segments[segments.len() - 2].ident == "pod"
        && segments[segments.len() - 3].ident == "pinapod";
    let direct_pina_reexport = segments.len() == 2 && segments[0].ident == "pina";
    let standard_option = segments.len() == 3
        && (segments[0].ident == "core" || segments[0].ident == "std")
        && segments[1].ident == "option"
        && name.ident == "Option";
    if segments.len() != 1
        && !namespaced_by_pinapod
        && !namespaced_by_pinapod_pod
        && !direct_pina_reexport
        && !standard_option
    {
        return None;
    }
    match name.ident.to_string().as_str() {
        "String" | "PodString" => Some(DynamicName::String),
        "Vec" | "PodVec" => Some(DynamicName::Vec),
        "Option" => Some(DynamicName::Option),
        _ => None,
    }
}

pub fn validate_dynamic_prefix_args(ty: &Type) -> Result<(), TokenStream> {
    let Some(segment) = last_path_segment(ty) else {
        return Ok(());
    };
    let dynamic_name = recognized_dynamic_name(ty);
    let Some(args) = angle_args(&segment.arguments) else {
        return Ok(());
    };

    let prefix_index = match dynamic_name {
        Some(DynamicName::String) => Some(1),
        Some(DynamicName::Vec) => Some(2),
        _ => None,
    };
    if let Some(prefix) = prefix_index.and_then(|index| args.iter().nth(index)) {
        if parse_prefix_arg(prefix).is_none() {
            let message = format!(
                "{} length prefix must be the byte width 1, 2, 4, or 8",
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
    if recognized_dynamic_name(ty) != Some(DynamicName::String) {
        return None;
    }
    let seg = last_path_segment(ty)?;
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
    if recognized_dynamic_name(ty) != Some(DynamicName::Vec) {
        return None;
    }
    let seg = last_path_segment(ty)?;
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

fn classify_string_checked(ty: &Type) -> Result<TailField, TokenStream> {
    let segment = last_path_segment(ty).expect("checked by caller");
    let arguments = angle_args(&segment.arguments).ok_or_else(|| {
        compact_type_error(
            ty,
            "compact strings must be `String<CAPACITY>` or `PodString<CAPACITY, PREFIX>`",
        )
    })?;
    if !(1..=2).contains(&arguments.len()) {
        return Err(compact_type_error(
            ty,
            "compact strings require a capacity and an optional 1, 2, 4, or 8-byte prefix",
        ));
    }

    let max = extract_const_expr(&arguments[0]).ok_or_else(|| {
        compact_type_error(ty, "compact string capacity must be a const expression")
    })?;
    let pfx = match arguments.get(1) {
        Some(argument) => parse_prefix_arg(argument).ok_or_else(|| {
            compact_type_error(ty, "compact string prefix must be 1, 2, 4, or 8 bytes")
        })?,
        None => 1,
    };

    Ok(TailField::Segment {
        presence: TailPresence::Always,
        payload: TailPayload::String { max, pfx },
    })
}

fn classify_vec_checked(ty: &Type) -> Result<TailField, TokenStream> {
    let segment = last_path_segment(ty).expect("checked by caller");
    let arguments = angle_args(&segment.arguments).ok_or_else(|| {
        compact_type_error(
            ty,
            "compact vectors must be `Vec<T, CAPACITY>` or `PodVec<T, CAPACITY, PREFIX>`",
        )
    })?;
    if !(2..=3).contains(&arguments.len()) {
        return Err(compact_type_error(
            ty,
            "compact vectors require an element type, capacity, and optional 1, 2, 4, or 8-byte prefix",
        ));
    }

    let elem = match &arguments[0] {
        GenericArgument::Type(element) => element.clone(),
        _ => {
            return Err(compact_type_error(
                ty,
                "compact vector element must be a type",
            ));
        }
    };
    let max = extract_const_expr(&arguments[1]).ok_or_else(|| {
        compact_type_error(ty, "compact vector capacity must be a const expression")
    })?;
    let pfx = match arguments.get(2) {
        Some(argument) => parse_prefix_arg(argument).ok_or_else(|| {
            compact_type_error(ty, "compact vector prefix must be 1, 2, 4, or 8 bytes")
        })?,
        None => 2,
    };

    if is_string_type(&elem) {
        classify_string_checked(&elem)?;
        return Ok(TailField::Segment {
            presence: TailPresence::Always,
            payload: TailPayload::Vec {
                elem: Box::new(elem),
                max,
                pfx,
            },
        });
    }
    if contains_dynamic_type(&elem) {
        return Err(compact_type_error(
            &elem,
            "unsupported dynamic compact vector element; supported compact fields are: `String<N>`, `Vec<T, N>` for fixed `T`, `Option<T>` for fixed `T`, `Option<String<N>>`, `Option<Vec<T, N>>` for fixed `T`, and `Vec<String<M>, N>`",
        ));
    }

    Ok(TailField::Segment {
        presence: TailPresence::Always,
        payload: TailPayload::Vec {
            elem: Box::new(elem),
            max,
            pfx,
        },
    })
}

fn classify_option_checked(ty: &Type) -> Result<FieldKind, TokenStream> {
    let segment = last_path_segment(ty).expect("checked by caller");
    let arguments = angle_args(&segment.arguments)
        .ok_or_else(|| compact_type_error(ty, "compact options must be `Option<T>`"))?;
    if arguments.len() != 1 {
        return Err(compact_type_error(
            ty,
            "compact options require exactly one type argument",
        ));
    }
    let inner = match &arguments[0] {
        GenericArgument::Type(inner) => inner,
        _ => {
            return Err(compact_type_error(
                ty,
                "compact option payload must be a type",
            ));
        }
    };

    if is_string_type(inner) {
        let TailField::Segment {
            payload: TailPayload::String { max, pfx },
            ..
        } = classify_string_checked(inner)?
        else {
            unreachable!("string classification must produce a string payload")
        };
        return Ok(FieldKind::Tail(TailField::Segment {
            presence: TailPresence::OptionTag,
            payload: TailPayload::String { max, pfx },
        }));
    }
    if is_vec_type(inner) {
        if vec_element(inner).is_some_and(is_string_type) {
            return Err(compact_type_error(
                inner,
                "unsupported dynamic nesting: `Option<Vec<String<_>, _>>` is not supported; supported compact fields are: `String<N>`, `Vec<T, N>` for fixed `T`, `Option<T>` for fixed `T`, `Option<String<N>>`, `Option<Vec<T, N>>` for fixed `T`, and `Vec<String<M>, N>`",
            ));
        }
        let tail = classify_vec_checked(inner)?;
        let TailField::Segment { payload, .. } = tail;
        return match payload {
            TailPayload::Vec { elem, max, pfx } => Ok(FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::Vec { elem, max, pfx },
            })),
            TailPayload::String { .. } => unreachable!("vector classification cannot be a string"),
        };
    }
    if contains_dynamic_type(inner) {
        return Err(compact_type_error(
            inner,
            "unsupported dynamic compact option payload; supported compact fields are: `String<N>`, `Vec<T, N>` for fixed `T`, `Option<T>` for fixed `T`, `Option<String<N>>`, `Option<Vec<T, N>>` for fixed `T`, and `Vec<String<M>, N>`",
        ));
    }

    Ok(FieldKind::Inline)
}

fn classify_option_dynamic(ty: &Type) -> Option<TailField> {
    if recognized_dynamic_name(ty) != Some(DynamicName::Option) {
        return None;
    }
    let seg = last_path_segment(ty)?;
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
    // 1. String / PodString
    if let Some(ts) = try_map_string(ty) {
        return ts;
    }

    // 2. Vec / PodVec
    if let Some(ts) = try_map_vec(ty) {
        return ts;
    }

    // 3. PodOption (must check before Option to avoid matching PodOption as generic)
    if let Some(ts) = try_map_pod_option(ty) {
        return ts;
    }

    // 4. Option
    if let Some(ts) = try_map_option(ty) {
        return ts;
    }

    // 5. Array types → keep as-is
    if matches!(ty, Type::Array(_)) {
        return quote! { #ty };
    }

    // 6. Delegate scalar and custom types through their representation
    // contract. Never infer a primitive representation from an unqualified
    // spelling: a caller-local `struct i8(bool)` must not become the builtin
    // integer merely because its final token is named `i8`.
    quote! { <#ty as pinapod::ZcField>::Pod }
}

fn try_map_string(ty: &Type) -> Option<TokenStream> {
    if recognized_dynamic_name(ty) != Some(DynamicName::String) {
        return None;
    }
    let seg = last_path_segment(ty)?;
    let args = angle_args(&seg.arguments)?;
    let mut iter = args.iter();
    let n_arg = iter.next()?;
    let pfx: usize = iter.next().and_then(parse_prefix_arg).unwrap_or(1);
    Some(quote! { pinapod::pod::PodString<#n_arg, #pfx> })
}

fn try_map_vec(ty: &Type) -> Option<TokenStream> {
    if recognized_dynamic_name(ty) != Some(DynamicName::Vec) {
        return None;
    }
    let seg = last_path_segment(ty)?;
    let args = angle_args(&seg.arguments)?;
    let mut iter = args.iter();
    let t_arg = match iter.next()? {
        GenericArgument::Type(t) => t,
        _ => return None,
    };
    let n_arg = iter.next()?;
    let pfx: usize = iter.next().and_then(parse_prefix_arg).unwrap_or(2);
    let mapped_t = map_to_pod_type(t_arg);
    Some(quote! { pinapod::pod::PodVecRepr<#mapped_t, #n_arg, #pfx> })
}

fn try_map_option(ty: &Type) -> Option<TokenStream> {
    if recognized_dynamic_name(ty) != Some(DynamicName::Option) {
        return None;
    }
    let seg = last_path_segment(ty)?;
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

/// Return the last segment of a path such as `pinapod::String<32>`.
fn last_path_segment(ty: &Type) -> Option<&syn::PathSegment> {
    if let Type::Path(type_path) = ty {
        return type_path.path.segments.last();
    }
    None
}

fn is_string_type(ty: &Type) -> bool {
    recognized_dynamic_name(ty) == Some(DynamicName::String)
}

fn is_vec_type(ty: &Type) -> bool {
    recognized_dynamic_name(ty) == Some(DynamicName::Vec)
}

fn contains_dynamic_type(ty: &Type) -> bool {
    let Some(segment) = last_path_segment(ty) else {
        return false;
    };
    if is_string_type(ty) || is_vec_type(ty) {
        return true;
    }
    if segment.ident != "Option" {
        return false;
    }
    angle_args(&segment.arguments)
        .and_then(|arguments| arguments.first())
        .and_then(|argument| match argument {
            GenericArgument::Type(inner) => Some(contains_dynamic_type(inner)),
            _ => None,
        })
        .unwrap_or(false)
}

fn vec_element(ty: &Type) -> Option<&Type> {
    if !is_vec_type(ty) {
        return None;
    }
    let arguments = angle_args(&last_path_segment(ty)?.arguments)?;
    match arguments.first()? {
        GenericArgument::Type(element) => Some(element),
        _ => None,
    }
}

fn compact_type_error(ty: &Type, message: &str) -> TokenStream {
    syn::Error::new_spanned(ty, message).to_compile_error()
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
    fn scalar_mapping_uses_the_resolved_zc_field_contract() {
        let shadowable: Type = syn::parse_quote!(i8);
        let qualified: Type = syn::parse_quote!(core::primitive::u64);

        assert_eq!(
            map_to_pod_type(&shadowable).to_string(),
            quote!(<i8 as pinapod::ZcField>::Pod).to_string()
        );
        assert_eq!(
            map_to_pod_type(&qualified).to_string(),
            quote!(<core::primitive::u64 as pinapod::ZcField>::Pod).to_string()
        );
    }

    #[test]
    fn pod_option_mapping_uses_the_default_prefix_when_omitted() {
        let ty: Type = syn::parse_quote!(PodOption<u64>);
        let mapped = map_to_pod_type(&ty);
        let inner = quote!(<u64 as pinapod::ZcField>::Pod);

        assert_eq!(
            mapped.to_string(),
            quote!(pinapod::pod::PodOption<#inner>).to_string()
        );
    }

    #[test]
    fn prefix_mapping_accepts_only_supported_const_widths() {
        let u8_type: GenericArgument = syn::parse_quote!(u8);
        let u16_type: GenericArgument = syn::parse_quote!(u16);
        let u32_type: GenericArgument = syn::parse_quote!(u32);
        let u64_type: GenericArgument = syn::parse_quote!(u64);
        let one: GenericArgument = syn::parse_quote!(1);
        let two: GenericArgument = syn::parse_quote!(2);
        let four: GenericArgument = syn::parse_quote!(4);
        let eight: GenericArgument = syn::parse_quote!(8);

        assert_eq!(parse_prefix_arg(&u8_type), None);
        assert_eq!(parse_prefix_arg(&u16_type), None);
        assert_eq!(parse_prefix_arg(&u32_type), None);
        assert_eq!(parse_prefix_arg(&u64_type), None);
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

    #[test]
    fn compact_classifier_does_not_match_unrelated_final_segments() {
        let string: Type = syn::parse_quote!(unrelated::String<32>);
        let vector: Type = syn::parse_quote!(unrelated::Vec<u8, 16>);

        assert!(matches!(
            classify_compact_field(&string),
            Ok(FieldKind::Inline)
        ));
        assert!(matches!(
            classify_compact_field(&vector),
            Ok(FieldKind::Inline)
        ));
    }

    #[test]
    fn compact_classifier_accepts_pinapod_and_pina_reexport_paths() {
        let nested_string: Type = syn::parse_quote!(pina::pinapod::String<32>);
        let nested_vector: Type = syn::parse_quote!(pina::pinapod::pod::PodVec<u8, 16, 2>);
        let direct_string: Type = syn::parse_quote!(pina::String<32>);
        let direct_vector: Type = syn::parse_quote!(pina::Vec<u8, 16>);

        for ty in [nested_string, nested_vector, direct_string, direct_vector] {
            assert!(matches!(
                classify_compact_field(&ty),
                Ok(FieldKind::Tail(_))
            ));
        }
    }

    #[test]
    fn compact_string_vectors_use_the_fixed_vector_payload() {
        let ty: Type = syn::parse_quote!(pinapod::Vec<pinapod::String<12>, 4>);
        let kind = classify_compact_field(&ty).unwrap();
        let FieldKind::Tail(TailField::Segment {
            presence: TailPresence::Always,
            payload: TailPayload::Vec { elem, max, pfx },
        }) = kind
        else {
            panic!("string vectors must use the fixed-stride vector path");
        };

        assert_eq!(quote!(#elem).to_string(), "pinapod :: String < 12 >");
        assert_eq!(quote!(#max).to_string(), "4");
        assert_eq!(pfx, 2);
    }

    #[test]
    fn compact_classifier_rejects_nested_dynamic_payloads() {
        let nested: Type = syn::parse_quote!(Option<pinapod::Vec<pinapod::String<12>, 4>>);
        let error = classify_compact_field(&nested).unwrap_err().to_string();

        assert!(error.contains("Option<Vec<String<_>, _>>"));
    }
}
