#![allow(
    clippy::manual_let_else,
    clippy::single_match_else,
    clippy::too_many_lines,
    tail_expr_drop_order,
    reason = "the upstream-compatible generator keeps parsing and token emission phases together"
)]

use {
    crate::schema::Schema,
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
    syn::Type,
};

pub fn generate(schema: &Schema) -> TokenStream {
    let struct_name = &schema.name;
    let generics = &schema.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let zc_name = format_ident!("{}Zc", struct_name);

    // Build ZC struct fields from the field's declared representation contract.
    // A projection is intentional here: spelling alone must never make a
    // caller-local lookalike such as `struct i8(bool)` use the primitive wire
    // representation.
    let zc_fields: Vec<TokenStream> = schema
        .fields
        .iter()
        .map(|f| {
            let name = &f.name;
            let vis = &f.vis;
            let pod_ty = pod_projection(&f.ty);
            quote! { #vis #name: #pod_ty }
        })
        .collect();

    // Build ZcValidate field delegation.
    let field_names: Vec<&syn::Ident> = schema.fields.iter().map(|f| &f.name).collect();
    let field_types: Vec<&Type> = schema.fields.iter().map(|field| &field.ty).collect();
    let pod_field_types: Vec<TokenStream> = schema
        .fields
        .iter()
        .map(|field| pod_projection(&field.ty))
        .collect();
    let capacity_checks: Vec<TokenStream> = schema
        .fields
        .iter()
        .flat_map(|field| fixed_capacity_checks(&field.ty))
        .collect();
    let where_clause_with_pod_bounds = {
        let representation_bounds: Vec<_> = field_types
            .iter()
            .map(|field_ty| {
                quote! {
                    #field_ty: pinapod::ZcField,
                    <#field_ty as pinapod::ZcField>::Pod: pinapod::ZcElem
                }
            })
            .collect();

        match (where_clause, representation_bounds.is_empty()) {
            (Some(existing), false) => {
                let predicates = existing.predicates.iter();
                quote! { where #(#predicates,)* #(#representation_bounds,)* }
            }
            (Some(existing), true) => quote! { #existing },
            (None, false) => quote! { where #(#representation_bounds,)* },
            (None, true) => quote! {},
        }
    };
    // Generate accessor methods on the Zc companion.
    let accessors = generate_accessors(schema);

    let align_assert = if schema.generics.params.is_empty() {
        quote! {
            const _: () = ::core::assert!(::core::mem::align_of::<#zc_name #ty_generics>() == 1);
        }
    } else {
        quote! {}
    };

    let inherent_helpers = if schema.no_inherent {
        quote! {}
    } else {
        quote! {
            impl #impl_generics #struct_name #ty_generics #where_clause_with_pod_bounds {
                /// The exact number of bytes in this fixed representation.
                pub const SIZE: ::core::primitive::usize = ::core::mem::size_of::<#zc_name #ty_generics>();

                /// Read and validate exactly one encoded value.
                #[inline(always)]
                pub fn read_exact(
                    data: &[::core::primitive::u8],
                ) -> ::core::result::Result<&#zc_name #ty_generics, pinapod::PinaPodError> {
                    <Self as pinapod::PinaPodFixed>::read_exact(data)
                }

                /// Mutably read and validate exactly one encoded value.
                #[inline(always)]
                pub fn read_exact_mut(
                    data: &mut [::core::primitive::u8],
                ) -> ::core::result::Result<&mut #zc_name #ty_generics, pinapod::PinaPodError> {
                    <Self as pinapod::PinaPodFixed>::read_exact_mut(data)
                }

                /// Read one encoded value from the start of a containing buffer.
                #[inline(always)]
                pub fn read_prefix(
                    data: &[::core::primitive::u8],
                ) -> ::core::result::Result<&#zc_name #ty_generics, pinapod::PinaPodError> {
                    <Self as pinapod::PinaPodFixed>::read_prefix(data)
                }

                /// Mutably read one value from the start of a containing buffer.
                #[inline(always)]
                pub fn read_prefix_mut(
                    data: &mut [::core::primitive::u8],
                ) -> ::core::result::Result<&mut #zc_name #ty_generics, pinapod::PinaPodError> {
                    <Self as pinapod::PinaPodFixed>::read_prefix_mut(data)
                }

                /// Validate exactly one encoded value without constructing a view.
                #[inline(always)]
                pub fn validate_exact(
                    data: &[::core::primitive::u8],
                ) -> ::core::result::Result<(), pinapod::PinaPodError> {
                    <Self as pinapod::PinaPodFixed>::validate_exact(data)
                }

                /// Validate one encoded value at the start of a containing buffer.
                #[inline(always)]
                pub fn validate_prefix(
                    data: &[::core::primitive::u8],
                ) -> ::core::result::Result<(), pinapod::PinaPodError> {
                    <Self as pinapod::PinaPodFixed>::validate_prefix(data)
                }

                /// Initialize exactly one value and validate it once complete.
                #[inline(always)]
                pub fn initialize(
                    data: &mut [::core::primitive::u8],
                    initialize: impl ::core::ops::FnOnce(
                        &mut #zc_name #ty_generics,
                    ) -> ::core::result::Result<(), pinapod::PinaPodError>,
                ) -> ::core::result::Result<&mut #zc_name #ty_generics, pinapod::PinaPodError> {
                    <Self as pinapod::PinaPodFixed>::initialize(data, initialize)
                }
            }
        }
    };

    quote! {
        #[repr(C)]
        pub struct #zc_name #generics #where_clause_with_pod_bounds {
            #( #zc_fields ),*
        }

        impl #impl_generics ::core::marker::Copy for #zc_name #ty_generics #where_clause_with_pod_bounds {}

        impl #impl_generics ::core::clone::Clone for #zc_name #ty_generics #where_clause_with_pod_bounds {
            fn clone(&self) -> Self {
                *self
            }
        }

        #align_assert

        #accessors

        #inherent_helpers

        impl #impl_generics pinapod::ZcValidate for #zc_name #ty_generics #where_clause_with_pod_bounds {
            fn validate_ref(
                value: &Self,
            ) -> ::core::result::Result<(), pinapod::PinaPodError> {
                #(#capacity_checks)*
                #(<#pod_field_types as pinapod::ZcValidate>::validate_ref(&value.#field_names)?;)*
                ::core::result::Result::Ok(())
            }
        }

        impl #impl_generics pinapod::PinaPod for #struct_name #ty_generics #where_clause_with_pod_bounds {}

        unsafe impl #impl_generics pinapod::PinaPodFixed for #struct_name #ty_generics #where_clause_with_pod_bounds {
            type Zc = #zc_name #ty_generics;
        }

        unsafe impl #impl_generics pinapod::ZcField for #struct_name #ty_generics #where_clause_with_pod_bounds {
            type Pod = #zc_name #ty_generics;
        }

        // SAFETY: Every generated field is required to implement ZcElem. The
        // companion is repr(C), and the non-generic case has an explicit
        // alignment assertion above.
        unsafe impl #impl_generics pinapod::ZcElem for #zc_name #ty_generics #where_clause_with_pod_bounds {}
    }
}

fn pod_projection(ty: &Type) -> TokenStream {
    quote! { <#ty as pinapod::ZcField>::Pod }
}

fn fixed_capacity_checks(ty: &Type) -> Vec<TokenStream> {
    let mut checks = Vec::new();
    collect_fixed_capacity_checks(ty, &mut checks);
    checks
}

fn collect_fixed_capacity_checks(ty: &Type, checks: &mut Vec<TokenStream>) {
    let Type::Path(type_path) = ty else {
        return;
    };
    if type_path.qself.is_some() {
        return;
    }
    let segments: Vec<_> = type_path.path.segments.iter().collect();
    let segment = match segments.as_slice() {
        [name] => name,
        [root, name] if root.ident == "pinapod" => name,
        [root, module, name] if root.ident == "pinapod" && module.ident == "pod" => name,
        [root, module, name]
            if (root.ident == "core" || root.ident == "std") && module.ident == "option" =>
        {
            name
        }
        _ => return,
    };
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return;
    };
    let arguments: Vec<_> = arguments.args.iter().collect();

    match segment.ident.to_string().as_str() {
        "String" | "PodString" => {
            let Some(capacity) = arguments.first() else {
                return;
            };
            let prefix = arguments
                .get(1)
                .map_or_else(|| quote! { 1 }, |prefix| quote! { #prefix });

            checks.push(quote! {
                let _ = pinapod::pod::PodString::<#capacity, #prefix>::VALID;
            });
        }
        "Vec" | "PodVec" => {
            let (Some(syn::GenericArgument::Type(element)), Some(capacity)) =
                (arguments.first(), arguments.get(1))
            else {
                return;
            };
            let prefix = arguments
                .get(2)
                .map_or_else(|| quote! { 2 }, |prefix| quote! { #prefix });

            checks.push(quote! {
                let _ = pinapod::pod::PodVec::<::core::primitive::u8, #capacity, #prefix>::VALID;
            });
            collect_fixed_capacity_checks(element, checks);
        }
        "Option" | "PodOption" => {
            if let Some(syn::GenericArgument::Type(inner)) = arguments.first() {
                collect_fixed_capacity_checks(inner, checks);
            }
        }
        _ => {}
    }
}

/// Classify a schema field type to determine what kind of accessor to generate.
enum AccessorKind {
    /// `u8`, `i8` — copy the field directly.
    CopyDirect,
    /// `u16`–`u128`, `bool` — pod type has `From` to native; return native via `.into()`.
    NativeViaFrom(TokenStream),
    /// `Address`, `[u8; N]` — borrow; return `&T`.
    Borrow,
    /// Bounded UTF-8 storage — return `&str`.
    String,
    /// A bounded fixed vector — return its active representation slice.
    Vec(TokenStream),
    /// An optional fixed value — return the semantic shape of its payload.
    Option(OptionAccessor),
    /// `#[pinapod(skip_accessor)]` — skip.
    Skip,
}

enum OptionAccessor {
    CopyDirect(TokenStream),
    NativeViaFrom(TokenStream),
    String,
    Vec(TokenStream),
    Borrow(TokenStream),
}

fn classify_accessor(ty: &Type, skip: bool) -> AccessorKind {
    if skip {
        return AccessorKind::Skip;
    }

    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            let name = seg.ident.to_string();
            match name.as_str() {
                "u8" | "i8" => return AccessorKind::CopyDirect,
                "u16" => {
                    return AccessorKind::NativeViaFrom(quote! { ::core::primitive::u16 });
                }
                "u32" => {
                    return AccessorKind::NativeViaFrom(quote! { ::core::primitive::u32 });
                }
                "u64" => {
                    return AccessorKind::NativeViaFrom(quote! { ::core::primitive::u64 });
                }
                "u128" => {
                    return AccessorKind::NativeViaFrom(quote! { ::core::primitive::u128 });
                }
                "i16" => {
                    return AccessorKind::NativeViaFrom(quote! { ::core::primitive::i16 });
                }
                "i32" => {
                    return AccessorKind::NativeViaFrom(quote! { ::core::primitive::i32 });
                }
                "i64" => {
                    return AccessorKind::NativeViaFrom(quote! { ::core::primitive::i64 });
                }
                "i128" => {
                    return AccessorKind::NativeViaFrom(quote! { ::core::primitive::i128 });
                }
                "bool" => {
                    return AccessorKind::NativeViaFrom(quote! { ::core::primitive::bool });
                }
                "String" | "PodString" => return AccessorKind::String,
                "Vec" | "PodVec" => {
                    return AccessorKind::Vec(extract_container_inner(ty));
                }
                "Option" | "PodOption" => {
                    return AccessorKind::Option(classify_option_accessor(ty));
                }
                _ => {}
            }
        }
    }

    // Array types [u8; N] and Address (which is [u8; 32] under the hood but appears
    // as a named type) — borrow.
    if matches!(ty, Type::Array(_)) {
        return AccessorKind::Borrow;
    }

    // Named types that we know are borrow-friendly (align 1, fixed).
    // For anything else (custom ZcField types), borrow by default.
    AccessorKind::Borrow
}

/// Extract and map the first type argument of a bounded container.
fn extract_container_inner(ty: &Type) -> TokenStream {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            if let syn::PathArguments::AngleBracketed(ab) = &seg.arguments {
                if let Some(syn::GenericArgument::Type(inner)) = ab.args.first() {
                    return pod_projection(inner);
                }
            }
        }
    }
    // Fallback — shouldn't happen since we only call this for PodOption fields.
    quote! { () }
}

fn classify_option_accessor(ty: &Type) -> OptionAccessor {
    let Some(inner) = extract_inner_type(ty) else {
        return OptionAccessor::Borrow(quote! { () });
    };
    let mapped_inner = pod_projection(inner);

    if let Type::Path(type_path) = inner {
        if let Some(segment) = type_path.path.segments.last() {
            let name = segment.ident.to_string();

            match name.as_str() {
                "u8" => {
                    return OptionAccessor::CopyDirect(quote! { ::core::primitive::u8 });
                }
                "i8" => {
                    return OptionAccessor::CopyDirect(quote! { ::core::primitive::i8 });
                }
                "u16" => {
                    return OptionAccessor::NativeViaFrom(quote! { ::core::primitive::u16 });
                }
                "u32" => {
                    return OptionAccessor::NativeViaFrom(quote! { ::core::primitive::u32 });
                }
                "u64" => {
                    return OptionAccessor::NativeViaFrom(quote! { ::core::primitive::u64 });
                }
                "u128" => {
                    return OptionAccessor::NativeViaFrom(quote! { ::core::primitive::u128 });
                }
                "i16" => {
                    return OptionAccessor::NativeViaFrom(quote! { ::core::primitive::i16 });
                }
                "i32" => {
                    return OptionAccessor::NativeViaFrom(quote! { ::core::primitive::i32 });
                }
                "i64" => {
                    return OptionAccessor::NativeViaFrom(quote! { ::core::primitive::i64 });
                }
                "i128" => {
                    return OptionAccessor::NativeViaFrom(quote! { ::core::primitive::i128 });
                }
                "bool" => {
                    return OptionAccessor::NativeViaFrom(quote! { ::core::primitive::bool });
                }
                "String" | "PodString" => return OptionAccessor::String,
                "Vec" | "PodVec" => {
                    return OptionAccessor::Vec(extract_container_inner(inner));
                }
                _ => {}
            }
        }
    }

    OptionAccessor::Borrow(mapped_inner)
}

fn extract_inner_type(ty: &Type) -> Option<&Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };

    arguments.args.iter().find_map(|argument| match argument {
        syn::GenericArgument::Type(inner) => Some(inner),
        _ => None,
    })
}

fn generate_accessors(schema: &Schema) -> TokenStream {
    // Only generate for non-generic fixed structs.
    if !schema.generics.params.is_empty() || schema.is_compact {
        return quote! {};
    }

    let zc_name = format_ident!("{}Zc", schema.name);

    let methods: Vec<TokenStream> = schema
        .fields
        .iter()
        .filter_map(|f| {
            let name = &f.name;
            let pod_ty = pod_projection(&f.ty);

            match classify_accessor(&f.ty, f.skip_accessor) {
                AccessorKind::CopyDirect => Some(quote! {
                    #[inline(always)]
                    pub fn #name(&self) -> #pod_ty {
                        self.#name
                    }
                }),
                AccessorKind::NativeViaFrom(native_ty) => Some(quote! {
                    #[inline(always)]
                    pub fn #name(&self) -> #native_ty {
                        ::core::convert::From::from(self.#name)
                    }
                }),
                AccessorKind::Borrow => Some(quote! {
                    #[inline(always)]
                    pub fn #name(&self) -> &#pod_ty {
                        &self.#name
                    }
                }),
                AccessorKind::String => Some(quote! {
                    #[inline(always)]
                    pub fn #name(&self) -> &::core::primitive::str {
                        self.#name.as_str()
                    }
                }),
                AccessorKind::Vec(element) => Some(quote! {
                    #[inline(always)]
                    pub fn #name(&self) -> &[#element] {
                        self.#name.as_slice()
                    }
                }),
                AccessorKind::Option(option) => Some(match option {
                    OptionAccessor::CopyDirect(inner) => quote! {
                        #[inline(always)]
                        pub fn #name(&self) -> ::core::option::Option<#inner> {
                            self.#name.get()
                        }
                    },
                    OptionAccessor::NativeViaFrom(inner) => quote! {
                        #[inline(always)]
                        pub fn #name(&self) -> ::core::option::Option<#inner> {
                            self.#name.get().map(::core::convert::From::from)
                        }
                    },
                    OptionAccessor::String => quote! {
                        #[inline(always)]
                        pub fn #name(&self) -> ::core::option::Option<&::core::primitive::str> {
                            self.#name.get_ref().map(|value| value.as_str())
                        }
                    },
                    OptionAccessor::Vec(element) => quote! {
                        #[inline(always)]
                        pub fn #name(&self) -> ::core::option::Option<&[#element]> {
                            self.#name.get_ref().map(|value| value.as_slice())
                        }
                    },
                    OptionAccessor::Borrow(inner) => quote! {
                        #[inline(always)]
                        pub fn #name(&self) -> ::core::option::Option<&#inner> {
                            self.#name.get_ref()
                        }
                    },
                }),
                AccessorKind::Skip => None,
            }
        })
        .collect();

    if methods.is_empty() {
        return quote! {};
    }

    quote! {
        impl #zc_name {
            #( #methods )*
        }
    }
}

pub fn generate_enum(input: &syn::DeriveInput) -> TokenStream {
    let enum_name = &input.ident;
    let zc_name = format_ident!("{}Zc", enum_name);

    // 1. Parse #[repr(uN)] attribute.
    let repr = match parse_enum_repr(input) {
        Some(r) => r,
        None => {
            return quote! {
                compile_error!("PinaPod enums require #[repr(u8)], #[repr(u16)], #[repr(u32)], or #[repr(u64)]");
            };
        }
    };

    // 2. Extract variants — all must be unit variants with explicit discriminants.
    let variants = match &input.data {
        syn::Data::Enum(data) => &data.variants,
        _ => unreachable!("generate_enum called on non-enum"),
    };

    let mut variant_names = Vec::new();
    for v in variants {
        if !v.fields.is_empty() {
            let msg = format!(
                "PinaPod enum variant `{}` must be a unit variant (no data fields)",
                v.ident
            );
            return quote! { compile_error!(#msg); };
        }
        match &v.discriminant {
            Some(_) => {}
            None => {
                let msg = format!(
                    "PinaPod enum variant `{}` must have an explicit discriminant (e.g. `= 0`)",
                    v.ident
                );
                return quote! { compile_error!(#msg); };
            }
        }
        variant_names.push(&v.ident);
    }

    // 3. Map repr to types and sizes.
    let (native_ty, pod_ty, repr_size): (TokenStream, TokenStream, usize) = match repr.as_str() {
        "u8" => (
            quote! { ::core::primitive::u8 },
            quote! { ::core::primitive::u8 },
            1,
        ),
        "u16" => (
            quote! { ::core::primitive::u16 },
            quote! { pinapod::pod::PodU16 },
            2,
        ),
        "u32" => (
            quote! { ::core::primitive::u32 },
            quote! { pinapod::pod::PodU32 },
            4,
        ),
        "u64" => (
            quote! { ::core::primitive::u64 },
            quote! { pinapod::pod::PodU64 },
            8,
        ),
        _ => unreachable!(),
    };

    // 4. Build the valid discriminant set for validation.
    let valid_arms: Vec<TokenStream> = variant_names
        .iter()
        .map(|name| quote! { value if value == (#enum_name::#name as #native_ty) })
        .collect();

    // 5. Build the From<Enum> -> PodType match arms.
    let from_arms: Vec<TokenStream> = variant_names
        .iter()
        .map(|name| {
            quote! {
                #enum_name::#name => ::core::convert::Into::into(
                    #enum_name::#name as #native_ty,
                )
            }
        })
        .collect();

    // For repr(u8), the Zc is a newtype wrapping u8 with .get().
    // For repr(u16+), the Zc is the Pod type which already has .get().
    // To unify the interface, we generate a Zc newtype for all cases.
    let read_value = match repr.as_str() {
        "u8" => quote! { self.0[0] },
        _ => quote! { <#native_ty>::from_le_bytes(self.0) },
    };

    quote! {
        #[repr(transparent)]
        #[derive(::core::clone::Clone, ::core::marker::Copy)]
        pub struct #zc_name([::core::primitive::u8; #repr_size]);

        impl #zc_name {
            #[inline(always)]
            pub fn get(&self) -> #native_ty {
                #read_value
            }
        }

        impl pinapod::ZcValidate for #zc_name {
            #[allow(clippy::manual_range_patterns)]
            fn validate_ref(
                value: &Self,
            ) -> ::core::result::Result<(), pinapod::PinaPodError> {
                let v = value.get();
                match v {
                    #( #valid_arms => ::core::result::Result::Ok(()), )*
                    _ => ::core::result::Result::Err(pinapod::PinaPodError::InvalidDiscriminant),
                }
            }
        }

        impl pinapod::PinaPod for #enum_name {}

        unsafe impl pinapod::PinaPodFixed for #enum_name {
            type Zc = #zc_name;
        }

        impl #enum_name {
            /// The exact number of bytes in this fixed representation.
            pub const SIZE: ::core::primitive::usize = ::core::mem::size_of::<#zc_name>();

            /// Read and validate exactly one encoded value.
            #[inline(always)]
            pub fn read_exact(
                data: &[::core::primitive::u8],
            ) -> ::core::result::Result<&#zc_name, pinapod::PinaPodError> {
                <Self as pinapod::PinaPodFixed>::read_exact(data)
            }

            /// Mutably read and validate exactly one encoded value.
            #[inline(always)]
            pub fn read_exact_mut(
                data: &mut [::core::primitive::u8],
            ) -> ::core::result::Result<&mut #zc_name, pinapod::PinaPodError> {
                <Self as pinapod::PinaPodFixed>::read_exact_mut(data)
            }

            /// Read one encoded value from the start of a containing buffer.
            #[inline(always)]
            pub fn read_prefix(
                data: &[::core::primitive::u8],
            ) -> ::core::result::Result<&#zc_name, pinapod::PinaPodError> {
                <Self as pinapod::PinaPodFixed>::read_prefix(data)
            }

            /// Mutably read one value from the start of a containing buffer.
            #[inline(always)]
            pub fn read_prefix_mut(
                data: &mut [::core::primitive::u8],
            ) -> ::core::result::Result<&mut #zc_name, pinapod::PinaPodError> {
                <Self as pinapod::PinaPodFixed>::read_prefix_mut(data)
            }

            /// Validate exactly one encoded value without constructing a view.
            #[inline(always)]
            pub fn validate_exact(
                data: &[::core::primitive::u8],
            ) -> ::core::result::Result<(), pinapod::PinaPodError> {
                <Self as pinapod::PinaPodFixed>::validate_exact(data)
            }

            /// Validate one encoded value at the start of a containing buffer.
            #[inline(always)]
            pub fn validate_prefix(
                data: &[::core::primitive::u8],
            ) -> ::core::result::Result<(), pinapod::PinaPodError> {
                <Self as pinapod::PinaPodFixed>::validate_prefix(data)
            }

            /// Initialize exactly one value and validate it once complete.
            #[inline(always)]
            pub fn initialize(
                data: &mut [::core::primitive::u8],
                initialize: impl ::core::ops::FnOnce(
                    &mut #zc_name,
                ) -> ::core::result::Result<(), pinapod::PinaPodError>,
            ) -> ::core::result::Result<&mut #zc_name, pinapod::PinaPodError> {
                <Self as pinapod::PinaPodFixed>::initialize(data, initialize)
            }
        }

        impl ::core::convert::From<#enum_name> for #pod_ty {
            fn from(v: #enum_name) -> Self {
                match v {
                    #( #from_arms ),*
                }
            }
        }

        unsafe impl pinapod::ZcField for #enum_name {
            type Pod = #zc_name;
        }

        // --- Enum ergonomics ---

        impl ::core::convert::From<#enum_name> for #zc_name {
            fn from(v: #enum_name) -> Self {
                let raw: #native_ty = match v {
                    #( #enum_name::#variant_names => #enum_name::#variant_names as #native_ty ),*
                };
                Self(raw.to_le_bytes())
            }
        }

        impl ::core::cmp::PartialEq<#enum_name> for #zc_name {
            fn eq(&self, other: &#enum_name) -> ::core::primitive::bool {
                let other_raw: #native_ty = match other {
                    #( #enum_name::#variant_names => #enum_name::#variant_names as #native_ty ),*
                };
                self.get() == other_raw
            }
        }

        impl #zc_name {
            /// Try to convert the raw ZC value back to the enum.
            #[allow(clippy::manual_range_patterns)]
            pub fn try_to_enum(
                &self,
            ) -> ::core::result::Result<#enum_name, pinapod::PinaPodError> {
                let val = self.get();
                match val {
                    #( #valid_arms => ::core::result::Result::Ok(#enum_name::#variant_names), )*
                    _ => ::core::result::Result::Err(pinapod::PinaPodError::InvalidDiscriminant),
                }
            }

            pub fn is(&self, variant: #enum_name) -> ::core::primitive::bool {
                let other: #zc_name = ::core::convert::Into::into(variant);
                self.get() == other.get()
            }
        }

        impl ::core::fmt::Display for #zc_name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                match self.get() {
                    #( #valid_arms => ::core::write!(f, ::core::stringify!(#variant_names)), )*
                    other => ::core::write!(
                        f,
                        "{}(invalid: {})",
                        ::core::stringify!(#enum_name),
                        other,
                    ),
                }
            }
        }

        impl ::core::fmt::Debug for #zc_name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                match self.get() {
                    #( #valid_arms => ::core::write!(
                        f,
                        "{}Zc({})",
                        ::core::stringify!(#enum_name),
                        ::core::stringify!(#variant_names),
                    ), )*
                    other => ::core::write!(
                        f,
                        "{}Zc(invalid: {})",
                        ::core::stringify!(#enum_name),
                        other,
                    ),
                }
            }
        }

        impl ::core::cmp::PartialEq for #zc_name {
            fn eq(&self, other: &Self) -> ::core::primitive::bool {
                self.0 == other.0
            }
        }

        impl ::core::cmp::PartialEq<#native_ty> for #zc_name {
            fn eq(&self, other: &#native_ty) -> ::core::primitive::bool {
                self.get() == *other
            }
        }

        // SAFETY: #zc_name is #[repr(transparent)] over [u8; #repr_size], alignment is 1.
        unsafe impl pinapod::ZcElem for #zc_name {}
    }
}

fn parse_enum_repr(input: &syn::DeriveInput) -> Option<String> {
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
    fn generated_struct_requires_the_original_field_mapping_and_zc_elem() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct ShadowedPrimitive {
                value: i8,
            }
        };
        let schema = Schema::parse(&input).unwrap();
        let generated = generate(&schema).to_string();

        assert!(generated.contains("value : < i8 as pinapod :: ZcField > :: Pod"));
        assert!(generated.contains("i8 : pinapod :: ZcField"));
        assert!(generated.contains("< i8 as pinapod :: ZcField > :: Pod : pinapod :: ZcElem"));
    }

    #[test]
    fn no_inherent_omits_schema_helpers_but_keeps_the_trait_contract() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[pinapod(no_inherent)]
            struct EmbeddedSchema {
                value: u64,
            }
        };
        let schema = Schema::parse(&input).unwrap();
        let generated = generate(&schema).to_string();

        assert!(!generated.contains("pub const SIZE"));
        assert!(!generated.contains("pub fn read_exact"));
        assert!(generated.contains("pinapod :: PinaPodFixed for EmbeddedSchema"));
        assert!(generated.contains("pub struct EmbeddedSchemaZc"));
    }

    #[test]
    fn fixed_containers_force_recursive_prefix_capacity_checks() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct NestedCapacity {
                value: Option<Vec<Option<String<256>>, 4>>,
            }
        };
        let schema = Schema::parse(&input).unwrap();
        let generated = generate(&schema).to_string();

        assert!(generated.contains("PodVec :: < :: core :: primitive :: u8 , 4 , 2 > :: VALID"));
        assert!(generated.contains("PodString :: < 256 , 1 > :: VALID"));
    }

    #[test]
    fn generates_u32_and_u64_enum_storage() {
        let input_u32: syn::DeriveInput = syn::parse_quote! {
            #[repr(u32)]
            enum Wide32 {
                Value = 7,
            }
        };
        let input_u64: syn::DeriveInput = syn::parse_quote! {
            #[repr(u64)]
            enum Wide64 {
                Value = 9,
            }
        };

        let generated_u32 = generate_enum(&input_u32).to_string();
        let generated_u64 = generate_enum(&input_u64).to_string();

        assert!(generated_u32.contains("PodU32"));
        assert!(generated_u64.contains("PodU64"));
    }
}
