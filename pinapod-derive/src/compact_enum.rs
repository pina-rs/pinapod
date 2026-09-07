#![allow(
    clippy::manual_let_else,
    clippy::needless_pass_by_value,
    clippy::single_match_else,
    clippy::too_many_lines,
    tail_expr_drop_order,
    reason = "the upstream-compatible generator keeps parsing and token emission phases together"
)]

use {
    crate::type_map::{
        classify_compact_field, map_to_pod_type, validate_dynamic_prefix_args, FieldKind,
        TailField, TailPayload, TailPresence,
    },
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
    syn::{Data, DeriveInput, Expr, Fields, Type, Variant},
};

enum VariantPayload {
    Unit,
    String {
        max: Expr,
        pfx: usize,
    },
    Vec {
        elem: Box<Type>,
        max: Expr,
        pfx: usize,
    },
    Fixed {
        ty: Type,
    },
}

struct CompactVariant<'a> {
    name: &'a syn::Ident,
    disc: TokenStream,
    payload: VariantPayload,
}

pub fn generate(input: &DeriveInput) -> TokenStream {
    let enum_name = &input.ident;
    let vis = &input.vis;
    let module_name = format_ident!("__pinapod_compact_{}", enum_name);
    let ref_name = format_ident!("{}Ref", enum_name);
    let patch_name = format_ident!("{}Patch", enum_name);
    let header_name = format_ident!("{}Header", enum_name);

    let repr = match parse_enum_repr(input) {
        Some(r) => r,
        None => {
            return quote! {
                compile_error!("compact PinaPod enums require #[repr(u8)], #[repr(u16)], #[repr(u32)], or #[repr(u64)]");
            };
        }
    };

    if !input.generics.params.is_empty() {
        return syn::Error::new_spanned(
            &input.generics,
            "compact PinaPod enums do not support generic parameters",
        )
        .to_compile_error();
    }

    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => unreachable!("compact enum generation called on non-enum"),
    };
    let is_fieldless = variants
        .iter()
        .all(|variant| matches!(variant.fields, Fields::Unit));

    let (native_ty, tag_size): (TokenStream, usize) = match repr.as_str() {
        "u8" => (quote! { u8 }, 1),
        "u16" => (quote! { u16 }, 2),
        "u32" => (quote! { u32 }, 4),
        "u64" => (quote! { u64 }, 8),
        _ => unreachable!(),
    };

    let mut parsed = Vec::new();
    for variant in variants {
        let disc = match &variant.discriminant {
            Some(_) if is_fieldless => {
                let name = &variant.ident;
                quote! { (#enum_name::#name as #native_ty) }
            }
            Some((
                _,
                Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Int(value),
                    ..
                }),
            )) => match value.base10_parse::<u64>() {
                Ok(parsed) => {
                    let value = syn::LitInt::new(&parsed.to_string(), value.span());
                    quote! { #value }
                }
                Err(error) => return error.to_compile_error(),
            },
            Some((_, expression)) => {
                return syn::Error::new_spanned(
                    expression,
                    "data-carrying compact PinaPod enum discriminants must be unsigned integer literals; fieldless enums may use any compiler-checked discriminant expression",
                )
                .to_compile_error();
            }
            None => {
                let msg = format!(
                    "compact PinaPod enum variant `{}` must have an explicit discriminant",
                    variant.ident
                );
                return quote! { compile_error!(#msg); };
            }
        };

        let payload = match parse_payload(variant) {
            Ok(payload) => payload,
            Err(tokens) => return tokens,
        };

        parsed.push(CompactVariant {
            name: &variant.ident,
            disc,
            payload,
        });
    }

    let tag_name = format_ident!("__{}Tag", enum_name);
    let tag_variants = parsed.iter().map(|variant| {
        let name = variant.name;
        let disc = &variant.disc;
        quote! { #name = #disc }
    });

    let mut ref_variants: Vec<_> = parsed
        .iter()
        .map(|variant| {
            let name = variant.name;
            match &variant.payload {
                VariantPayload::Unit => quote! { #name },
                VariantPayload::String { .. } => quote! { #name(&'a str) },
                VariantPayload::Vec { elem, .. } => {
                    let mapped_elem = map_to_pod_type(elem);
                    quote! { #name(&'a [#mapped_elem]) }
                }
                VariantPayload::Fixed { ty } => {
                    quote! { #name(&'a <#ty as pinapod::PinaPodFixed>::Zc) }
                }
            }
        })
        .collect();
    if is_fieldless {
        ref_variants.push(quote! {
            #[doc(hidden)]
            __Lifetime(core::marker::PhantomData<&'a ()>)
        });
    }

    let validate_arms: Vec<_> = parsed
        .iter()
        .map(|variant| {
            let name = variant.name;
            let validate = validate_payload_tokens(&variant.payload, tag_size);
            quote! { x if x == (#tag_name::#name as #native_ty) => { #validate } }
        })
        .collect();

    let ref_arms: Vec<_> = parsed
        .iter()
        .map(|variant| {
            let name = variant.name;
            let construct = construct_ref_tokens(&variant.payload, name, tag_size);
            quote! { x if x == (#tag_name::#name as #native_ty) => { #construct } }
        })
        .collect();

    let read_tag = read_tag_expr(tag_size, quote! { data });
    let support = enum_support();
    let min_size = enum_min_size(&parsed, tag_size);
    let max_size = enum_max_size(&parsed, tag_size);
    let patch_impl = generate_patch_impl(
        enum_name,
        &tag_name,
        &patch_name,
        &parsed,
        tag_size,
        &native_ty,
        &ref_name,
    );

    let generated = quote! {
        #support

        #[repr(#native_ty)]
        enum #tag_name {
            #( #tag_variants ),*
        }

        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct #header_name {
            __tag: [u8; #tag_size],
        }

        impl pinapod::ZcValidate for #header_name {
            fn validate_ref(_value: &Self) -> Result<(), pinapod::PinaPodError> {
                Ok(())
            }
        }

        // SAFETY: the header is `#[repr(C)]` over one byte array.
        unsafe impl pinapod::ZcElem for #header_name {}

        pub enum #ref_name<'a> {
            #( #ref_variants ),*
        }

        impl pinapod::PinaPod for #enum_name {}

        unsafe impl pinapod::PinaPodCompact for #enum_name {
            type Header = #header_name;
            const MIN_SIZE: usize = #min_size;
            const MAX_SIZE: usize = #max_size;
            const TAIL_ALIGNMENT: usize = 1;
            const HEADER_SIZE: usize = #tag_size;

            fn validate(data: &[u8]) -> Result<(), pinapod::PinaPodError> {
                Self::validate_storage_len(data.len())?;
                if data.len() < #tag_size {
                    return Err(pinapod::PinaPodError::BufferTooSmall);
                }
                let __tag: #native_ty = #read_tag;
                match __tag {
                    #( #validate_arms, )*
                    _ => Err(pinapod::PinaPodError::InvalidDiscriminant),
                }
            }
        }

        impl<'a> #ref_name<'a> {
            pub fn new(data: &'a [u8]) -> Result<Self, pinapod::PinaPodError> {
                <#enum_name as pinapod::PinaPodCompact>::validate(data)?;
                let __tag: #native_ty = #read_tag;
                match __tag {
                    #( #ref_arms, )*
                    _ => Err(pinapod::PinaPodError::InvalidDiscriminant),
                }
            }
        }

        #patch_impl
    };

    quote! {
        #[doc(hidden)]
        #[allow(dead_code, non_snake_case, unused_imports)]
        mod #module_name {
            use super::*;

            #generated
        }

        #[allow(unused_imports)]
        #vis use #module_name::{#header_name, #patch_name, #ref_name};
    }
}

fn enum_min_size(variants: &[CompactVariant<'_>], tag_size: usize) -> TokenStream {
    let candidates = variants.iter().map(|variant| {
        let payload_min = match &variant.payload {
            VariantPayload::Unit => quote! { 0usize },
            VariantPayload::String { pfx, .. } | VariantPayload::Vec { pfx, .. } => {
                quote! { #pfx }
            }
            VariantPayload::Fixed { ty } => {
                quote! { core::mem::size_of::<<#ty as pinapod::PinaPodFixed>::Zc>() }
            }
        };
        quote! {
            {
                let __candidate = match (#tag_size as usize).checked_add(#payload_min) {
                    Some(value) => value,
                    None => panic!("compact enum minimum size overflows usize"),
                };
                if __candidate < __min {
                    __min = __candidate;
                }
            }
        }
    });

    quote! {{
        let mut __min = usize::MAX;
        #( #candidates )*
        __min
    }}
}

fn enum_max_size(variants: &[CompactVariant<'_>], tag_size: usize) -> TokenStream {
    let candidates = variants.iter().map(|variant| {
        let payload_max = match &variant.payload {
            VariantPayload::Unit => quote! { 0usize },
            VariantPayload::String { max, pfx } => quote! {
                match (#max as usize).checked_add(#pfx) {
                    Some(value) => value,
                    None => panic!("compact enum maximum size overflows usize"),
                }
            },
            VariantPayload::Vec { elem, max, pfx } => {
                let mapped_elem = map_to_pod_type(elem);
                quote! {
                    match (match (#max as usize)
                        .checked_mul(core::mem::size_of::<#mapped_elem>())
                    {
                        Some(value) => value,
                        None => panic!("compact enum maximum size overflows usize"),
                    }).checked_add(#pfx) {
                        Some(value) => value,
                        None => panic!("compact enum maximum size overflows usize"),
                    }
                }
            }
            VariantPayload::Fixed { ty } => {
                quote! { core::mem::size_of::<<#ty as pinapod::PinaPodFixed>::Zc>() }
            }
        };
        quote! {
            {
                let __candidate = match (#tag_size as usize).checked_add(#payload_max) {
                    Some(value) => value,
                    None => panic!("compact enum maximum size overflows usize"),
                };
                if __candidate > __max {
                    __max = __candidate;
                }
            }
        }
    });

    quote! {{
        let mut __max = 0usize;
        #( #candidates )*
        __max
    }}
}

fn enum_support() -> TokenStream {
    quote! {
        fn __pinapod_checked_add(
            left: usize,
            right: usize,
        ) -> Result<usize, pinapod::PinaPodError> {
            left.checked_add(right).ok_or(pinapod::PinaPodError::Overflow)
        }

        fn __pinapod_checked_mul(
            left: usize,
            right: usize,
        ) -> Result<usize, pinapod::PinaPodError> {
            left.checked_mul(right).ok_or(pinapod::PinaPodError::Overflow)
        }

        fn __pinapod_prefix_max(width: usize) -> Option<usize> {
            match width {
                1 => Some(u8::MAX as usize),
                2 => Some(u16::MAX as usize),
                4 => usize::try_from(u32::MAX).ok(),
                8 => Some(usize::MAX),
                _ => None,
            }
        }

        fn __pinapod_check_prefix(
            value: usize,
            width: usize,
        ) -> Result<(), pinapod::PinaPodError> {
            match __pinapod_prefix_max(width) {
                Some(max) if value <= max => Ok(()),
                _ => Err(pinapod::PinaPodError::Overflow),
            }
        }

        fn __pinapod_read_prefix(
            data: &[u8],
            offset: usize,
            width: usize,
        ) -> Result<usize, pinapod::PinaPodError> {
            let end = __pinapod_checked_add(offset, width)?;
            let bytes = data
                .get(offset..end)
                .ok_or(pinapod::PinaPodError::BufferTooSmall)?;
            let value = match bytes {
                [a] => u64::from(*a),
                [a, b] => u64::from(u16::from_le_bytes([*a, *b])),
                [a, b, c, d] => u64::from(u32::from_le_bytes([*a, *b, *c, *d])),
                [a, b, c, d, e, f, g, h] => {
                    u64::from_le_bytes([*a, *b, *c, *d, *e, *f, *g, *h])
                }
                _ => return Err(pinapod::PinaPodError::InvalidLength),
            };
            usize::try_from(value).map_err(|_| pinapod::PinaPodError::Overflow)
        }
    }
}

fn generate_patch_impl(
    enum_name: &syn::Ident,
    tag_name: &syn::Ident,
    patch_name: &syn::Ident,
    variants: &[CompactVariant<'_>],
    tag_size: usize,
    native_ty: &TokenStream,
    ref_name: &syn::Ident,
) -> TokenStream {
    let borrows_payload = variants
        .iter()
        .any(|variant| !matches!(variant.payload, VariantPayload::Unit));
    let declaration_generics = if borrows_payload {
        quote! { <'a> }
    } else {
        TokenStream::new()
    };
    let patch_ty = if borrows_payload {
        quote! { #patch_name<'_> }
    } else {
        quote! { #patch_name }
    };

    let mut patch_variants = Vec::new();
    let mut length_arms = Vec::new();
    let mut write_arms = Vec::new();
    let mut current_size_arms = Vec::new();

    for variant in variants {
        let name = variant.name;
        let write_tag = quote! {
            let __tag = (#tag_name::#name as #native_ty).to_le_bytes();
            data[..#tag_size].copy_from_slice(&__tag[..#tag_size]);
        };

        match &variant.payload {
            VariantPayload::Unit => {
                patch_variants.push(quote! { #name });
                length_arms.push(quote! { Self::#name => Ok(#tag_size) });
                write_arms.push(quote! {
                    Self::#name => {
                        #write_tag
                    }
                });
                current_size_arms.push(quote! {
                    x if x == (#tag_name::#name as #native_ty) => Ok(#tag_size)
                });
            }
            VariantPayload::String { max, pfx } => {
                patch_variants.push(quote! { #name(&'a str) });
                length_arms.push(quote! {
                    Self::#name(value) => {
                        if value.len() > #max {
                            return Err(pinapod::PinaPodError::Overflow);
                        }
                        __pinapod_check_prefix(value.len(), #pfx)?;
                        __pinapod_checked_add(
                            __pinapod_checked_add(#tag_size, #pfx)?,
                            value.len(),
                        )
                    }
                });
                write_arms.push(quote! {
                    Self::#name(value) => {
                        #write_tag
                        let __prefix = (value.len() as u64).to_le_bytes();
                        data[#tag_size..#tag_size + #pfx]
                            .copy_from_slice(&__prefix[..#pfx]);
                        data[#tag_size + #pfx..#tag_size + #pfx + value.len()]
                            .copy_from_slice(value.as_bytes());
                    }
                });
                current_size_arms.push(quote! {
                    x if x == (#tag_name::#name as #native_ty) => {
                        let __len = __pinapod_read_prefix(data, #tag_size, #pfx)?;
                        __pinapod_checked_add(
                            __pinapod_checked_add(#tag_size, #pfx)?,
                            __len,
                        )
                    }
                });
            }
            VariantPayload::Vec { elem, max, pfx } => {
                let mapped_elem = map_to_pod_type(elem);
                patch_variants.push(quote! { #name(&'a [#mapped_elem]) });
                length_arms.push(quote! {
                    Self::#name(value) => {
                        if value.len() > #max {
                            return Err(pinapod::PinaPodError::Overflow);
                        }
                        __pinapod_check_prefix(value.len(), #pfx)?;
                        let __elem_size = core::mem::size_of::<#mapped_elem>();
                        if __elem_size == 0 {
                            return Err(pinapod::PinaPodError::InvalidLength);
                        }
                        for item in *value {
                            <#mapped_elem as pinapod::ZcValidate>::validate_ref(item)?;
                        }
                        let __byte_len = __pinapod_checked_mul(value.len(), __elem_size)?;
                        __pinapod_checked_add(
                            __pinapod_checked_add(#tag_size, #pfx)?,
                            __byte_len,
                        )
                    }
                });
                write_arms.push(quote! {
                    Self::#name(value) => {
                        #write_tag
                        let __prefix = (value.len() as u64).to_le_bytes();
                        data[#tag_size..#tag_size + #pfx]
                            .copy_from_slice(&__prefix[..#pfx]);
                        let __byte_len = value.len() * core::mem::size_of::<#mapped_elem>();
                        if __byte_len > 0 {
                            let __source = unsafe {
                                core::slice::from_raw_parts(
                                    value.as_ptr() as *const u8,
                                    __byte_len,
                                )
                            };
                            data[#tag_size + #pfx..#tag_size + #pfx + __byte_len]
                                .copy_from_slice(__source);
                        }
                    }
                });
                current_size_arms.push(quote! {
                    x if x == (#tag_name::#name as #native_ty) => {
                        let __count = __pinapod_read_prefix(data, #tag_size, #pfx)?;
                        let __byte_len = __pinapod_checked_mul(
                            __count,
                            core::mem::size_of::<#mapped_elem>(),
                        )?;
                        __pinapod_checked_add(
                            __pinapod_checked_add(#tag_size, #pfx)?,
                            __byte_len,
                        )
                    }
                });
            }
            VariantPayload::Fixed { ty } => {
                patch_variants.push(quote! { #name(&'a <#ty as pinapod::PinaPodFixed>::Zc) });
                length_arms.push(quote! {
                    Self::#name(value) => {
                        <<#ty as pinapod::PinaPodFixed>::Zc as pinapod::ZcValidate>::validate_ref(
                            value,
                        )?;
                        __pinapod_checked_add(
                            #tag_size,
                            core::mem::size_of::<<#ty as pinapod::PinaPodFixed>::Zc>(),
                        )
                    }
                });
                write_arms.push(quote! {
                    Self::#name(value) => {
                        #write_tag
                        let __byte_len =
                            core::mem::size_of::<<#ty as pinapod::PinaPodFixed>::Zc>();
                        if __byte_len > 0 {
                            let __source = unsafe {
                                core::slice::from_raw_parts(
                                    (*value as *const <#ty as pinapod::PinaPodFixed>::Zc)
                                        as *const u8,
                                    __byte_len,
                                )
                            };
                            data[#tag_size..#tag_size + __byte_len]
                                .copy_from_slice(__source);
                        }
                    }
                });
                current_size_arms.push(quote! {
                    x if x == (#tag_name::#name as #native_ty) => {
                        __pinapod_checked_add(
                            #tag_size,
                            core::mem::size_of::<<#ty as pinapod::PinaPodFixed>::Zc>(),
                        )
                    }
                });
            }
        }
    }

    let read_tag = read_tag_expr(tag_size, quote! { data });

    quote! {
        pub enum #patch_name #declaration_generics {
            #( #patch_variants ),*
        }

        impl #declaration_generics #patch_name #declaration_generics {
            fn encoded_len(&self) -> Result<usize, pinapod::PinaPodError> {
                match self {
                    #( #length_arms, )*
                }
            }

            fn current_encoded_len(data: &[u8]) -> Result<usize, pinapod::PinaPodError> {
                let __tag: #native_ty = #read_tag;
                match __tag {
                    #( #current_size_arms, )*
                    _ => Err(pinapod::PinaPodError::InvalidDiscriminant),
                }
            }

            fn write(&self, data: &mut [u8]) {
                match self {
                    #( #write_arms, )*
                }
            }

            pub fn updated_len(&self, data: &[u8]) -> Result<usize, pinapod::PinaPodError> {
                <#enum_name as pinapod::PinaPodCompact>::validate(data)?;
                self.encoded_len()
            }

            pub fn update(&self, data: &mut [u8]) -> Result<usize, pinapod::PinaPodError> {
                <#enum_name as pinapod::PinaPodCompact>::validate(data)?;
                let old_encoded_len = Self::current_encoded_len(data)?;
                let encoded_len = self.encoded_len()?;
                if encoded_len > data.len() {
                    return Err(pinapod::PinaPodError::BufferTooSmall);
                }

                self.write(data);
                if encoded_len < old_encoded_len {
                    data[encoded_len..old_encoded_len].fill(0);
                }
                Ok(encoded_len)
            }

            fn try_initialize(&self, data: &mut [u8]) -> Result<usize, pinapod::PinaPodError> {
                <#enum_name as pinapod::PinaPodCompact>::validate_storage_len(data.len())?;
                let encoded_len = self.encoded_len()?;
                if encoded_len > data.len() {
                    return Err(pinapod::PinaPodError::BufferTooSmall);
                }
                self.write(data);
                <#enum_name as pinapod::PinaPodCompact>::validate(data)?;
                Ok(encoded_len)
            }

            pub fn initialize(&self, data: &mut [u8]) -> Result<usize, pinapod::PinaPodError> {
                data.fill(0);
                let result = self.try_initialize(data);
                if result.is_err() {
                    data.fill(0);
                }
                result
            }
        }

        impl #declaration_generics pinapod::PinaPodPatch<#enum_name>
            for #patch_name #declaration_generics
        {
            fn updated_len(&self, data: &[u8]) -> Result<usize, pinapod::PinaPodError> {
                <#patch_name #declaration_generics>::updated_len(self, data)
            }

            fn update(&self, data: &mut [u8]) -> Result<usize, pinapod::PinaPodError> {
                <#patch_name #declaration_generics>::update(self, data)
            }

            fn initialize(&self, data: &mut [u8]) -> Result<usize, pinapod::PinaPodError> {
                <#patch_name #declaration_generics>::initialize(self, data)
            }
        }

        impl #enum_name {
            pub const HEADER_SIZE: usize = <Self as pinapod::PinaPodCompact>::HEADER_SIZE;
            pub const MIN_SIZE: usize = <Self as pinapod::PinaPodCompact>::MIN_SIZE;
            pub const MAX_SIZE: usize = <Self as pinapod::PinaPodCompact>::MAX_SIZE;
            pub const TAIL_ALIGNMENT: usize =
                <Self as pinapod::PinaPodCompact>::TAIL_ALIGNMENT;

            pub fn read_prefix(data: &[u8]) -> Result<#ref_name<'_>, pinapod::PinaPodError> {
                #ref_name::new(data)
            }

            pub fn updated_len(
                data: &[u8],
                patch: &#patch_ty,
            ) -> Result<usize, pinapod::PinaPodError> {
                patch.updated_len(data)
            }

            pub fn update(
                data: &mut [u8],
                patch: &#patch_ty,
            ) -> Result<usize, pinapod::PinaPodError> {
                patch.update(data)
            }

            pub fn initialize(
                data: &mut [u8],
                patch: &#patch_ty,
            ) -> Result<usize, pinapod::PinaPodError> {
                patch.initialize(data)
            }
        }
    }
}

fn parse_payload(variant: &Variant) -> Result<VariantPayload, TokenStream> {
    match &variant.fields {
        Fields::Unit => Ok(VariantPayload::Unit),
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            let ty = fields.unnamed[0].ty.clone();
            validate_dynamic_prefix_args(&ty)?;
            if has_compact_attr(&variant.attrs) {
                return Err(syn::Error::new_spanned(
                    &variant.fields,
                    "nested compact schemas are not supported in compact enum payloads; supported compact forms are: `String<N>`, `Vec<T, N>` for fixed `T`, `Option<T>` for fixed `T`, `Option<String<N>>`, `Option<Vec<T, N>>` for fixed `T`, and `Vec<String<M>, N>`",
                )
                .to_compile_error());
            }

            match classify_compact_field(&ty)? {
                FieldKind::Tail(TailField::Segment {
                    presence: TailPresence::Always,
                    payload: TailPayload::String { max, pfx },
                }) => Ok(VariantPayload::String { max, pfx }),
                FieldKind::Tail(TailField::Segment {
                    presence: TailPresence::Always,
                    payload: TailPayload::Vec { elem, max, pfx },
                }) => Ok(VariantPayload::Vec { elem, max, pfx }),
                FieldKind::Tail(TailField::Segment {
                    presence: TailPresence::OptionTag,
                    ..
                }) => Err(syn::Error::new_spanned(
                    &ty,
                    "compact PinaPod enum payloads do not support dynamic `Option`; wrap the payload in a compact struct",
                )
                .to_compile_error()),
                _ => Ok(VariantPayload::Fixed { ty }),
            }
        }
        _ => {
            let msg = format!(
                "compact PinaPod enum variant `{}` must be unit-like or contain exactly one unnamed payload field",
                variant.ident
            );
            Err(quote! { compile_error!(#msg); })
        }
    }
}

fn validate_payload_tokens(payload: &VariantPayload, tag_size: usize) -> TokenStream {
    match payload {
        VariantPayload::Unit => quote! { Ok(()) },
        VariantPayload::String { max, pfx } => {
            quote! {
                let _ = pinapod::pod::PodString::<#max, #pfx>::VALID;
                let __byte_len = __pinapod_read_prefix(data, #tag_size, #pfx)?;
                if __byte_len > #max {
                    return Err(pinapod::PinaPodError::InvalidLength);
                }
                let __payload_offset = __pinapod_checked_add(#tag_size, #pfx)?;
                let __payload_end = __pinapod_checked_add(__payload_offset, __byte_len)?;
                let __payload = data
                    .get(__payload_offset..__payload_end)
                    .ok_or(pinapod::PinaPodError::BufferTooSmall)?;
                if core::str::from_utf8(__payload).is_err() {
                    return Err(pinapod::PinaPodError::InvalidUtf8);
                }
                Ok(())
            }
        }
        VariantPayload::Vec { elem, max, pfx } => {
            let mapped_elem = map_to_pod_type(elem);
            quote! {
                let _ = pinapod::pod::PodVec::<u8, #max, #pfx>::VALID;
                let _ = const {
                    assert!(
                        core::mem::size_of::<#mapped_elem>() != 0,
                        "compact vector elements must not be zero-sized",
                    );
                };
                let __count = __pinapod_read_prefix(data, #tag_size, #pfx)?;
                if __count > #max {
                    return Err(pinapod::PinaPodError::InvalidLength);
                }
                let __payload_offset = __pinapod_checked_add(#tag_size, #pfx)?;
                let __elem_size = core::mem::size_of::<#mapped_elem>();
                let __byte_len = __pinapod_checked_mul(__count, __elem_size)?;
                let __payload_end = __pinapod_checked_add(__payload_offset, __byte_len)?;
                let __payload = data
                    .get(__payload_offset..__payload_end)
                    .ok_or(pinapod::PinaPodError::BufferTooSmall)?;
                for __i in 0..__count {
                    let __elem_offset = __pinapod_checked_mul(__i, __elem_size)?;
                    let __elem_ptr = unsafe {
                        &*(__payload.as_ptr().add(__elem_offset) as *const #mapped_elem)
                    };
                    <#mapped_elem as pinapod::ZcValidate>::validate_ref(__elem_ptr)?;
                }
                Ok(())
            }
        }
        VariantPayload::Fixed { ty } => quote! {
            let __payload = data
                .get(#tag_size..)
                .ok_or(pinapod::PinaPodError::BufferTooSmall)?;
            <#ty as pinapod::PinaPodFixed>::validate_prefix(__payload)?;
            Ok(())
        },
    }
}

fn construct_ref_tokens(
    payload: &VariantPayload,
    name: &syn::Ident,
    tag_size: usize,
) -> TokenStream {
    match payload {
        VariantPayload::Unit => quote! { Ok(Self::#name) },
        VariantPayload::String { pfx, .. } => {
            let read_len = read_len_at_expr(quote! { data }, quote! { #tag_size }, *pfx);
            quote! {
                let __byte_len = #read_len;
                let __payload_offset = #tag_size + #pfx;
                let __bytes = &data[__payload_offset..__payload_offset + __byte_len];
                Ok(Self::#name(unsafe { core::str::from_utf8_unchecked(__bytes) }))
            }
        }
        VariantPayload::Vec { elem, pfx, .. } => {
            let mapped_elem = map_to_pod_type(elem);
            let read_len = read_len_at_expr(quote! { data }, quote! { #tag_size }, *pfx);
            quote! {
                let __count = #read_len;
                let __payload_offset = #tag_size + #pfx;
                let __ptr = unsafe { data.as_ptr().add(__payload_offset) as *const #mapped_elem };
                Ok(Self::#name(unsafe { core::slice::from_raw_parts(__ptr, __count) }))
            }
        }
        VariantPayload::Fixed { ty } => quote! {
            Ok(Self::#name(<#ty as pinapod::PinaPodFixed>::read_prefix(&data[#tag_size..])?))
        },
    }
}

fn read_tag_expr(tag_size: usize, data: TokenStream) -> TokenStream {
    match tag_size {
        1 => quote! { #data[0] },
        2 => quote! { u16::from_le_bytes([#data[0], #data[1]]) },
        4 => quote! { u32::from_le_bytes([#data[0], #data[1], #data[2], #data[3]]) },
        8 => quote! {
            u64::from_le_bytes([
                #data[0], #data[1], #data[2], #data[3],
                #data[4], #data[5], #data[6], #data[7],
            ])
        },
        _ => unreachable!("invalid repr size"),
    }
}

fn read_len_at_expr(data: TokenStream, offset: TokenStream, pfx: usize) -> TokenStream {
    match pfx {
        1 => quote! { #data[#offset] as usize },
        2 => quote! { u16::from_le_bytes([#data[#offset], #data[#offset + 1]]) as usize },
        4 => quote! {
            u32::from_le_bytes([
                #data[#offset],
                #data[#offset + 1],
                #data[#offset + 2],
                #data[#offset + 3],
            ]) as usize
        },
        8 => quote! {
            u64::from_le_bytes([
                #data[#offset],
                #data[#offset + 1],
                #data[#offset + 2],
                #data[#offset + 3],
                #data[#offset + 4],
                #data[#offset + 5],
                #data[#offset + 6],
                #data[#offset + 7],
            ]) as usize
        },
        _ => unreachable!("invalid prefix size"),
    }
}

fn has_compact_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
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
    })
}

fn parse_enum_repr(input: &DeriveInput) -> Option<String> {
    for attr in &input.attrs {
        if attr.path().is_ident("repr") {
            let mut repr_name = None;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("u8") {
                    repr_name = Some("u8".to_string());
                } else if meta.path.is_ident("u16") {
                    repr_name = Some("u16".to_string());
                } else if meta.path.is_ident("u32") {
                    repr_name = Some("u32".to_string());
                } else if meta.path.is_ident("u64") {
                    repr_name = Some("u64".to_string());
                }
                Ok(())
            });
            if repr_name.is_some() {
                return repr_name;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_compact_enums_receive_a_focused_diagnostic() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(u8)]
            enum GenericEvent<T> {
                Value(T) = 0,
            }
        };

        let output = generate(&input).to_string();

        assert!(output.contains("compact PinaPod enums do not support generic parameters"));
    }

    #[test]
    fn scalar_compact_enums_omit_prefix_writes() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(u8)]
            enum ScalarEvent {
                Empty = 0,
                Value(ScalarPayload) = 1,
            }
        };

        let output = generate(&input).to_string();

        assert!(!output.contains("__pinapod_check_prefix (value . len () , 1usize) ?"));
    }

    #[test]
    fn dynamic_compact_enums_use_checked_prefix_writes() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(u8)]
            enum DynamicEvent {
                Empty = 0,
                Label(pinapod::String<8>) = 1,
            }
        };

        let output = generate(&input).to_string();

        assert!(output.contains("__pinapod_check_prefix (value . len () , 1usize) ?"));
    }

    #[test]
    fn compact_enum_prefixes_receive_a_focused_diagnostic() {
        let input: DeriveInput = syn::parse_quote! {
            #[repr(u8)]
            enum InvalidPrefixEvent {
                Label(pinapod::PodString<8, 3>) = 0,
            }
        };

        let output = generate(&input).to_string();

        assert!(output.contains("PodString length prefix must be"));
    }
}
