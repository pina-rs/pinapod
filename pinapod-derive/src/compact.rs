#![allow(
    clippy::match_wildcard_for_single_variants,
    clippy::needless_pass_by_value,
    clippy::similar_names,
    clippy::too_many_lines,
    reason = "the upstream-compatible generator is clearer when its phases mirror the wire layout"
)]

use {
    crate::{
        schema::Schema,
        type_map::{
            map_to_pod_type, option_inner_type, FieldKind, TailField, TailPayload, TailPresence,
        },
    },
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
};

pub fn generate(schema: &Schema) -> TokenStream {
    let struct_name = &schema.name;
    let vis = &schema.vis;
    let module_name = format_ident!("__pinapod_compact_{}", struct_name);
    let header_name = format_ident!("{}Header", struct_name);
    let ref_name = format_ident!("{}Ref", struct_name);
    let mut_name = format_ident!("{}Mut", struct_name);
    let patch_name = format_ident!("{}Patch", struct_name);
    let (_, ty_generics, _) = schema.generics.split_for_impl();
    let header_ty = quote! { #header_name #ty_generics };

    let header_ts = generate_header(schema, &header_name);
    let trait_impl_ts = generate_trait_impl(schema, &header_ty);
    let ref_ts = generate_ref(schema, &header_ty, &ref_name);
    let mut_ts = generate_mut(schema, &header_ty, &mut_name);
    let patch_ts = generate_patch(schema, &header_ty, &ref_name, &mut_name, &patch_name);
    let support_ts = generate_support();

    quote! {
        #[doc(hidden)]
        #[allow(dead_code, non_snake_case, unused_imports)]
        mod #module_name {
            use super::*;

            #support_ts
            #header_ts
            #trait_impl_ts
            #ref_ts
            #mut_ts
            #patch_ts
        }

        #[allow(unused_imports)]
        #vis use #module_name::{#header_name, #patch_name, #ref_name};
    }
}

fn generate_support() -> TokenStream {
    quote! {
        #[inline(always)]
        fn __pinapod_checked_add(
            left: usize,
            right: usize,
        ) -> Result<usize, pinapod::PinaPodError> {
            left.checked_add(right).ok_or(pinapod::PinaPodError::Overflow)
        }

        #[inline(always)]
        fn __pinapod_checked_mul(
            left: usize,
            right: usize,
        ) -> Result<usize, pinapod::PinaPodError> {
            left.checked_mul(right).ok_or(pinapod::PinaPodError::Overflow)
        }

        const fn __pinapod_gcd(mut left: usize, mut right: usize) -> usize {
            while right != 0 {
                let remainder = left % right;
                left = right;
                right = remainder;
            }
            left
        }

        #[inline(always)]
        fn __pinapod_prefix_max(width: usize) -> Option<usize> {
            match width {
                1 => Some(u8::MAX as usize),
                2 => Some(u16::MAX as usize),
                4 => usize::try_from(u32::MAX).ok(),
                8 => Some(usize::MAX),
                _ => None,
            }
        }

        #[inline(always)]
        fn __pinapod_check_prefix(
            value: usize,
            width: usize,
        ) -> Result<(), pinapod::PinaPodError> {
            match __pinapod_prefix_max(width) {
                Some(max) if value <= max => Ok(()),
                _ => Err(pinapod::PinaPodError::Overflow),
            }
        }

        #[inline(always)]
        fn __pinapod_decode_prefix(bytes: &[u8]) -> Result<usize, pinapod::PinaPodError> {
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

        #[inline(always)]
        fn __pinapod_read_prefix(
            data: &[u8],
            offset: usize,
            width: usize,
        ) -> Result<usize, pinapod::PinaPodError> {
            let end = __pinapod_checked_add(offset, width)?;
            let bytes = data
                .get(offset..end)
                .ok_or(pinapod::PinaPodError::BufferTooSmall)?;
            __pinapod_decode_prefix(bytes)
        }

        #[inline(always)]
        fn __pinapod_write_prefix(
            data: &mut [u8],
            offset: usize,
            width: usize,
            value: usize,
        ) -> Result<(), pinapod::PinaPodError> {
            __pinapod_check_prefix(value, width)?;
            let end = __pinapod_checked_add(offset, width)?;
            let destination = data
                .get_mut(offset..end)
                .ok_or(pinapod::PinaPodError::BufferTooSmall)?;
            let bytes = (value as u64).to_le_bytes();
            destination.copy_from_slice(&bytes[..width]);
            Ok(())
        }

    }
}

// ---------------------------------------------------------------------------
// Header generation
// ---------------------------------------------------------------------------

fn generate_header(schema: &Schema, header_name: &syn::Ident) -> TokenStream {
    let generics = &schema.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut fields = Vec::new();

    let inline_field_names: Vec<&syn::Ident> = schema.inline_fields().map(|f| &f.name).collect();
    let inline_pod_types: Vec<TokenStream> = schema
        .inline_fields()
        .map(|f| map_to_pod_type(&f.ty))
        .collect();

    for f in schema.inline_fields() {
        let name = &f.name;
        let vis = &f.vis;
        let pod_ty = map_to_pod_type(&f.ty);
        fields.push(quote! { #vis #name: #pod_ty });
    }

    for f in schema.tail_fields() {
        match &f.kind {
            FieldKind::Tail(tail) => match tail.presence() {
                TailPresence::Always => {
                    let len_name = format_ident!("__{}_len", f.name);
                    let pfx = tail.payload().pfx();
                    fields.push(quote! { #len_name: [u8; #pfx] });
                }
                TailPresence::OptionTag => {
                    let tag_name = format_ident!("__{}_tag", f.name);
                    fields.push(quote! { #tag_name: [u8; 1] });
                }
            },
            _ => unreachable!(),
        }
    }

    let pod_bounds: Vec<_> = inline_pod_types
        .iter()
        .map(|pod_ty| quote! { #pod_ty: pinapod::ZcElem })
        .collect();
    let where_clause_with_bounds = where_clause_with_bounds(where_clause, pod_bounds.iter());
    let (marker_field, _) = generic_marker_tokens(&schema.generics);

    let align_assert = if schema.generics.params.is_empty() {
        quote! {
            const _: () = assert!(core::mem::align_of::<#header_name>() == 1);
        }
    } else {
        quote! {}
    };

    quote! {
        #[repr(C)]
        pub struct #header_name #generics #where_clause_with_bounds {
            #( #fields, )*
            #marker_field
        }

        #align_assert

        impl #impl_generics Copy for #header_name #ty_generics #where_clause_with_bounds {}

        impl #impl_generics Clone for #header_name #ty_generics #where_clause_with_bounds {
            fn clone(&self) -> Self {
                *self
            }
        }

        impl #impl_generics pinapod::ZcValidate for #header_name #ty_generics #where_clause_with_bounds {
            fn validate_ref(value: &Self) -> Result<(), pinapod::PinaPodError> {
                #(<#inline_pod_types as pinapod::ZcValidate>::validate_ref(&value.#inline_field_names)?;)*
                Ok(())
            }
        }

        // SAFETY: the header is `#[repr(C)]` and every field is an
        // alignment-one `ZcElem`; length and tag fields are byte arrays.
        unsafe impl #impl_generics pinapod::ZcElem for #header_name #ty_generics #where_clause_with_bounds {}
    }
}

// ---------------------------------------------------------------------------
// PinaPodCompact trait impl
// ---------------------------------------------------------------------------

fn generate_trait_impl(schema: &Schema, header_ty: &TokenStream) -> TokenStream {
    let struct_name = &schema.name;
    let (impl_generics, ty_generics, where_clause) = schema.generics.split_for_impl();
    let bounds = compact_bounds(schema);
    let where_clause_with_bounds = where_clause_with_bounds(where_clause, bounds.iter());
    let max_size = compact_max_size(schema, header_ty);
    let tail_alignment = compact_tail_alignment(schema);
    let schema_proofs: Vec<_> = schema
        .tail_fields()
        .map(|field| {
            let source = &field.ty;
            let marker = compact_marker_type(field);
            let capacity_checks = compact_capacity_checks(field);
            quote! {
                let __pinapod_type_check: fn(#source) -> #marker = |value| value;
                let _ = __pinapod_type_check;
                #capacity_checks
            }
        })
        .collect();
    let mut tail_validations = Vec::new();
    for f in schema.tail_fields() {
        match &f.kind {
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::String { max, pfx: _ },
            }) => {
                let len_name = format_ident!("__{}_len", f.name);
                tail_validations.push(quote! {
                    let #len_name = __pinapod_decode_prefix(&__hdr.#len_name)?;
                    if #len_name > #max {
                        return Err(pinapod::PinaPodError::InvalidLength);
                    }
                    let __tail_end = __pinapod_checked_add(__tail_offset, #len_name)?;
                    let __tail = data
                        .get(__tail_offset..__tail_end)
                        .ok_or(pinapod::PinaPodError::BufferTooSmall)?;
                    if core::str::from_utf8(__tail).is_err() {
                        return Err(pinapod::PinaPodError::InvalidUtf8);
                    }
                    __tail_offset = __tail_end;
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::Vec { elem, max, pfx: _ },
            }) => {
                let len_name = format_ident!("__{}_len", f.name);
                let mapped_elem = map_to_pod_type(elem);
                tail_validations.push(quote! {
					let #len_name = __pinapod_decode_prefix(&__hdr.#len_name)?;
					if #len_name > #max {
						return Err(pinapod::PinaPodError::InvalidLength);
					}
					let __elem_size = core::mem::size_of::<#mapped_elem>();
					if __elem_size == 0 {
						return Err(pinapod::PinaPodError::InvalidLength);
					}
					let __byte_len = __pinapod_checked_mul(#len_name, __elem_size)?;
					let __tail_end = __pinapod_checked_add(__tail_offset, __byte_len)?;
					let __tail = data
						.get(__tail_offset..__tail_end)
						.ok_or(pinapod::PinaPodError::BufferTooSmall)?;
					for __i in 0..#len_name {
						let __elem_offset = __pinapod_checked_mul(__i, __elem_size)?;
						let __elem_ptr = unsafe { &*(__tail.as_ptr().add(__elem_offset) as *const #mapped_elem) };
						<#mapped_elem as pinapod::ZcValidate>::validate_ref(__elem_ptr)?;
					}
					__tail_offset = __tail_end;
				});
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::String { max, pfx },
            }) => {
                let tag_name = format_ident!("__{}_tag", f.name);
                tail_validations.push(quote! {
					match __hdr.#tag_name[0] {
						0 => {}
						1 => {
							let __byte_len = __pinapod_read_prefix(data, __tail_offset, #pfx)?;
							if __byte_len > #max {
								return Err(pinapod::PinaPodError::InvalidLength);
							}
							let __payload_offset = __pinapod_checked_add(__tail_offset, #pfx)?;
							let __payload_end = __pinapod_checked_add(__payload_offset, __byte_len)?;
							let __payload = data
								.get(__payload_offset..__payload_end)
								.ok_or(pinapod::PinaPodError::BufferTooSmall)?;
							if core::str::from_utf8(__payload).is_err() {
								return Err(pinapod::PinaPodError::InvalidUtf8);
							}
							__tail_offset = __payload_end;
						}
						_ => return Err(pinapod::PinaPodError::InvalidTag),
					}
				});
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::Vec { elem, max, pfx },
            }) => {
                let tag_name = format_ident!("__{}_tag", f.name);
                let mapped_elem = map_to_pod_type(elem);
                tail_validations.push(quote! {
					match __hdr.#tag_name[0] {
						0 => {}
						1 => {
							let __count = __pinapod_read_prefix(data, __tail_offset, #pfx)?;
							if __count > #max {
								return Err(pinapod::PinaPodError::InvalidLength);
							}
							let __payload_offset = __pinapod_checked_add(__tail_offset, #pfx)?;
							let __elem_size = core::mem::size_of::<#mapped_elem>();
							if __elem_size == 0 {
								return Err(pinapod::PinaPodError::InvalidLength);
							}
							let __byte_len = __pinapod_checked_mul(__count, __elem_size)?;
							let __payload_end = __pinapod_checked_add(__payload_offset, __byte_len)?;
							let __payload = data
								.get(__payload_offset..__payload_end)
								.ok_or(pinapod::PinaPodError::BufferTooSmall)?;
							for __i in 0..__count {
								let __elem_offset = __pinapod_checked_mul(__i, __elem_size)?;
								let __elem_ptr = unsafe { &*(__payload.as_ptr().add(__elem_offset) as *const #mapped_elem) };
								<#mapped_elem as pinapod::ZcValidate>::validate_ref(__elem_ptr)?;
							}
							__tail_offset = __payload_end;
						}
						_ => return Err(pinapod::PinaPodError::InvalidTag),
					}
				});
            }
            _ => unreachable!(),
        }
    }

    quote! {
        impl #impl_generics pinapod::PinaPod for #struct_name #ty_generics #where_clause_with_bounds {}

        unsafe impl #impl_generics pinapod::PinaPodCompact for #struct_name #ty_generics #where_clause_with_bounds {
            type Header = #header_ty;
            const MIN_SIZE: usize = core::mem::size_of::<#header_ty>();
            const MAX_SIZE: usize = #max_size;
            const TAIL_ALIGNMENT: usize = #tail_alignment;
            const HEADER_SIZE: usize = core::mem::size_of::<#header_ty>();

            fn validate(data: &[u8]) -> Result<(), pinapod::PinaPodError> {
                #( #schema_proofs )*
                Self::validate_storage_len(data.len())?;
                if data.len() < core::mem::size_of::<#header_ty>() {
                    return Err(pinapod::PinaPodError::BufferTooSmall);
                }
                let __hdr = unsafe { &*(data.as_ptr() as *const #header_ty) };
                <#header_ty as pinapod::ZcValidate>::validate_ref(__hdr)?;
                let mut __tail_offset = core::mem::size_of::<#header_ty>();
                #( #tail_validations )*
                Ok(())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Ref generation
// ---------------------------------------------------------------------------

fn generate_ref(schema: &Schema, header_ty: &TokenStream, ref_name: &syn::Ident) -> TokenStream {
    let struct_name = &schema.name;
    let (_, struct_ty_generics, where_clause) = schema.generics.split_for_impl();
    let data_lifetime = fresh_lifetime(&schema.generics, "__pinapod_data");
    let ref_generics = generics_with_lifetime(&schema.generics, &data_lifetime);
    let (ref_impl_generics, ref_ty_generics, _) = ref_generics.split_for_impl();
    let bounds = compact_bounds(schema);
    let where_clause_with_bounds = where_clause_with_bounds(where_clause, bounds.iter());
    let (marker_field, marker_init) = generic_marker_tokens(&schema.generics);
    let tail_fields: Vec<_> = schema.tail_fields().collect();
    let current_encoded_len = compute_offset_tokens(header_ty, &tail_fields, tail_fields.len());
    let mut accessors = Vec::new();

    for (i, f) in tail_fields.iter().enumerate() {
        let fname = &f.name;
        let offset_computation = compute_offset_tokens(header_ty, &tail_fields, i);

        match &f.kind {
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::String { pfx, .. },
            }) => {
                let len_name = format_ident!("__{}_len", fname);
                let read_len = read_len_expr(&len_name, *pfx);
                accessors.push(quote! {
                    pub fn #fname(&self) -> &#data_lifetime str {
                        let __hdr = self.header();
                        let __byte_len = #read_len;
                        #offset_computation
                        unsafe {
                            let __ptr = self.data.as_ptr().add(__offset);
                            let __slice = core::slice::from_raw_parts(__ptr, __byte_len);
                            core::str::from_utf8_unchecked(__slice)
                        }
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::Vec { elem, pfx, .. },
            }) => {
                let len_name = format_ident!("__{}_len", fname);
                let read_len = read_len_expr(&len_name, *pfx);
                let mapped_elem = map_to_pod_type(elem);
                accessors.push(quote! {
                    pub fn #fname(&self) -> &#data_lifetime [#mapped_elem] {
                        let __hdr = self.header();
                        let __count = #read_len;
                        #offset_computation
                        unsafe {
                            let __ptr = self.data.as_ptr().add(__offset) as *const #mapped_elem;
                            core::slice::from_raw_parts(__ptr, __count)
                        }
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::String { pfx, .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", fname);
                let read_len = read_self_data_len_expr(quote! { __offset }, *pfx);
                accessors.push(quote! {
                    pub fn #fname(&self) -> Option<&#data_lifetime str> {
                        let __hdr = self.header();
                        if __hdr.#tag_name[0] == 0 {
                            return None;
                        }
                        #offset_computation
                        let __byte_len = #read_len;
                        let __payload_offset = __offset + #pfx;
                        unsafe {
                            let __ptr = self.data.as_ptr().add(__payload_offset);
                            let __slice = core::slice::from_raw_parts(__ptr, __byte_len);
                            Some(core::str::from_utf8_unchecked(__slice))
                        }
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::Vec { elem, pfx, .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", fname);
                let read_len = read_self_data_len_expr(quote! { __offset }, *pfx);
                let mapped_elem = map_to_pod_type(elem);
                accessors.push(quote! {
					pub fn #fname(&self) -> Option<&#data_lifetime [#mapped_elem]> {
						let __hdr = self.header();
						if __hdr.#tag_name[0] == 0 {
							return None;
						}
						#offset_computation
						let __count = #read_len;
						let __payload_offset = __offset + #pfx;
						unsafe {
							let __ptr = self.data.as_ptr().add(__payload_offset) as *const #mapped_elem;
							Some(core::slice::from_raw_parts(__ptr, __count))
						}
					}
				});
            }
            _ => unreachable!(),
        }
    }

    quote! {
        pub struct #ref_name #ref_generics #where_clause_with_bounds {
            data: &#data_lifetime [u8],
            encoded_len: usize,
            #marker_field
        }

        impl #ref_impl_generics core::ops::Deref for #ref_name #ref_ty_generics #where_clause_with_bounds {
            type Target = #header_ty;
            fn deref(&self) -> &#header_ty {
                self.header()
            }
        }

        impl #ref_impl_generics #ref_name #ref_ty_generics #where_clause_with_bounds {
            pub fn new(data: &#data_lifetime [u8]) -> Result<Self, pinapod::PinaPodError> {
                <#struct_name #struct_ty_generics as pinapod::PinaPodCompact>::validate(data)?;
                let mut value = Self {
                    data,
                    encoded_len: 0,
                    #marker_init
                };
                value.encoded_len = value.current_encoded_len();
                Ok(value)
            }

            fn header(&self) -> &#data_lifetime #header_ty {
                unsafe { &*(self.data.as_ptr() as *const #header_ty) }
            }

            fn current_encoded_len(&self) -> usize {
                let __hdr = self.header();
                #current_encoded_len
                __offset
            }

            pub fn encoded_len(&self) -> usize {
                self.encoded_len
            }

            pub fn storage_len(&self) -> usize {
                self.data.len()
            }

            pub fn spare_capacity(&self) -> usize {
                self.data.len() - self.encoded_len
            }

            #( #accessors )*
        }
    }
}

// ---------------------------------------------------------------------------
// Mut generation
// ---------------------------------------------------------------------------

fn generate_mut(schema: &Schema, header_ty: &TokenStream, mut_name: &syn::Ident) -> TokenStream {
    let struct_name = &schema.name;
    let (_, struct_ty_generics, where_clause) = schema.generics.split_for_impl();
    let data_lifetime = fresh_lifetime(&schema.generics, "__pinapod_data");
    let mut_generics = generics_with_lifetime(&schema.generics, &data_lifetime);
    let (mut_impl_generics, mut_ty_generics, _) = mut_generics.split_for_impl();
    let bounds = compact_bounds(schema);
    let where_clause_with_bounds = where_clause_with_bounds(where_clause, bounds.iter());
    let (marker_field, marker_init) = generic_marker_tokens(&schema.generics);
    let tail_fields: Vec<_> = schema.tail_fields().collect();
    let inline_mut_accessors: Vec<_> = schema
        .inline_fields()
        .map(|field| {
            let name = &field.name;
            let method = format_ident!("{}_mut", name);
            let pod_ty = map_to_pod_type(&field.ty);
            quote! {
                pub fn #method(&mut self) -> &mut #pod_ty {
                    &mut self.header_mut().#name
                }
            }
        })
        .collect();

    // Edit descriptor fields.
    let mut edit_fields = Vec::new();
    for f in &tail_fields {
        let edit_name = format_ident!("__{}_edit", f.name);
        match &f.kind {
            FieldKind::Tail(TailField::Segment {
                payload: TailPayload::String { .. },
                ..
            }) => {
                edit_fields.push(quote! { #edit_name: Option<(*const u8, usize)> });
            }
            FieldKind::Tail(TailField::Segment {
                payload: TailPayload::Vec { .. },
                ..
            }) => {
                edit_fields.push(quote! { #edit_name: Option<(*const u8, usize, usize)> });
            }
            _ => unreachable!(),
        }
    }

    let edit_inits: Vec<_> = tail_fields
        .iter()
        .map(|f| {
            let edit_name = format_ident!("__{}_edit", f.name);
            quote! { #edit_name: None }
        })
        .collect();

    // Setter methods.
    let mut setters = Vec::new();
    for f in &tail_fields {
        let fname = &f.name;
        let setter_name = format_ident!("set_{}", fname);
        let edit_name = format_ident!("__{}_edit", fname);

        match &f.kind {
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::String { max, pfx },
            }) => {
                setters.push(quote! {
					pub fn #setter_name(&mut self, value: &#data_lifetime str) -> Result<(), pinapod::PinaPodError> {
						if value.len() > #max || __pinapod_check_prefix(value.len(), #pfx).is_err() {
							return Err(pinapod::PinaPodError::Overflow);
						}
						self.#edit_name = Some((value.as_ptr(), value.len()));
						Ok(())
					}
				});
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::Vec { elem, max, pfx },
            }) => {
                let mapped_elem = map_to_pod_type(elem);
                setters.push(quote! {
                    pub fn #setter_name(&mut self, value: &#data_lifetime [#mapped_elem]) -> Result<(), pinapod::PinaPodError> {
                        if value.len() > #max || __pinapod_check_prefix(value.len(), #pfx).is_err() {
                            return Err(pinapod::PinaPodError::Overflow);
                        }
                        for __item in value {
                            <#mapped_elem as pinapod::ZcValidate>::validate_ref(__item)?;
                        }
                        self.#edit_name = Some((
                            value.as_ptr() as *const u8,
                            value.len(),
                            core::mem::size_of::<#mapped_elem>(),
                        ));
                        Ok(())
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::String { max, pfx },
            }) => {
                setters.push(quote! {
					pub fn #setter_name(&mut self, value: Option<&#data_lifetime str>) -> Result<(), pinapod::PinaPodError> {
						if let Some(value) = value {
							if value.len() > #max || __pinapod_check_prefix(value.len(), #pfx).is_err() {
								return Err(pinapod::PinaPodError::Overflow);
							}
							self.#edit_name = Some((value.as_ptr(), value.len()));
						} else {
							self.#edit_name = Some((core::ptr::null(), 0));
						}
						Ok(())
					}
				});
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::Vec { elem, max, pfx },
            }) => {
                let mapped_elem = map_to_pod_type(elem);
                setters.push(quote! {
                    pub fn #setter_name(&mut self, value: Option<&#data_lifetime [#mapped_elem]>) -> Result<(), pinapod::PinaPodError> {
                        if let Some(value) = value {
                            if value.len() > #max || __pinapod_check_prefix(value.len(), #pfx).is_err() {
                                return Err(pinapod::PinaPodError::Overflow);
                            }
                            for __item in value {
                                <#mapped_elem as pinapod::ZcValidate>::validate_ref(__item)?;
                            }
                            self.#edit_name = Some((
                                value.as_ptr() as *const u8,
                                value.len(),
                                core::mem::size_of::<#mapped_elem>(),
                            ));
                        } else {
                            self.#edit_name = Some((core::ptr::null(), 0, core::mem::size_of::<#mapped_elem>()));
                        }
                        Ok(())
                    }
                });
            }
            _ => unreachable!(),
        }
    }

    // `try_projected_size` is the read-only half of commit preflight. Keeping
    // it private lets the staged writer disappear when the atomic Patch API
    // replaces it without making layout-planning internals public.
    let mut projected_steps = Vec::new();
    for (i, f) in tail_fields.iter().enumerate() {
        let edit_name = format_ident!("__{}_edit", f.name);
        let len_name = format_ident!("__{}_len", f.name);
        let offset_computation = compute_offset_tokens(header_ty, &tail_fields, i);

        match &f.kind {
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::String { pfx, .. },
            }) => {
                let read_len = read_len_expr(&len_name, *pfx);
                projected_steps.push(quote! {
                    if let Some((_, __new_len)) = self.#edit_name {
                        let __hdr = self.header();
                        let __old_len = #read_len;
                        __total = __total
                            .checked_sub(__old_len)
                            .ok_or(pinapod::PinaPodError::Overflow)?;
                        __total = __pinapod_checked_add(__total, __new_len)?;
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::Vec { elem, pfx, .. },
            }) => {
                let read_len = read_len_expr(&len_name, *pfx);
                let mapped_elem = map_to_pod_type(elem);
                projected_steps.push(quote! {
                    if let Some((_, __new_count, __new_elem_size)) = self.#edit_name {
                        let __hdr = self.header();
                        let __old_count = #read_len;
                        let __old_len = __pinapod_checked_mul(
                            __old_count,
                            core::mem::size_of::<#mapped_elem>(),
                        )?;
                        let __new_len = __pinapod_checked_mul(
                            __new_count,
                            __new_elem_size,
                        )?;
                        __total = __total
                            .checked_sub(__old_len)
                            .ok_or(pinapod::PinaPodError::Overflow)?;
                        __total = __pinapod_checked_add(__total, __new_len)?;
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::String { pfx, .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", f.name);
                let old_size = old_option_string_size_expr(&tag_name, *pfx, quote! { __offset });
                projected_steps.push(quote! {
                    if let Some((__ptr, __byte_len)) = self.#edit_name {
                        let __hdr = self.header();
                        #offset_computation
                        let __old_len = #old_size;
                        let __new_len = if __ptr.is_null() {
                            0
                        } else {
                            __pinapod_checked_add(#pfx, __byte_len)?
                        };
                        __total = __total
                            .checked_sub(__old_len)
                            .ok_or(pinapod::PinaPodError::Overflow)?;
                        __total = __pinapod_checked_add(__total, __new_len)?;
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::Vec { elem, pfx, .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", f.name);
                let mapped_elem = map_to_pod_type(elem);
                let old_size =
                    old_option_vec_size_expr(&tag_name, *pfx, quote! { __offset }, &mapped_elem);
                projected_steps.push(quote! {
                    if let Some((__ptr, __count, __elem_size)) = self.#edit_name {
                        let __hdr = self.header();
                        #offset_computation
                        let __old_len = #old_size;
                        let __new_len = if __ptr.is_null() {
                            0
                        } else {
                            __pinapod_checked_add(
                                #pfx,
                                __pinapod_checked_mul(__count, __elem_size)?,
                            )?
                        };
                        __total = __total
                            .checked_sub(__old_len)
                            .ok_or(pinapod::PinaPodError::Overflow)?;
                        __total = __pinapod_checked_add(__total, __new_len)?;
                    }
                });
            }
            _ => unreachable!(),
        }
    }

    // commit()
    let commit_body = generate_commit_body(header_ty, &tail_fields);
    let current_encoded_len = compute_offset_tokens(header_ty, &tail_fields, tail_fields.len());

    quote! {
        struct #mut_name #mut_generics #where_clause_with_bounds {
            data: &#data_lifetime mut [u8],
            total_len: usize,
            #( #edit_fields, )*
            #marker_field
        }

        impl #mut_impl_generics core::ops::Deref for #mut_name #mut_ty_generics #where_clause_with_bounds {
            type Target = #header_ty;
            fn deref(&self) -> &#header_ty {
                self.header()
            }
        }

        impl #mut_impl_generics #mut_name #mut_ty_generics #where_clause_with_bounds {
            pub fn new(data: &#data_lifetime mut [u8]) -> Result<Self, pinapod::PinaPodError> {
                <#struct_name #struct_ty_generics as pinapod::PinaPodCompact>::validate(data)?;
                let mut value = Self {
                    data,
                    total_len: 0,
                    #( #edit_inits, )*
                    #marker_init
                };
                value.total_len = value.current_encoded_len();
                Ok(value)
            }

            /// # Safety
            /// Caller must ensure `data` is at least `HEADER_SIZE` bytes and
            /// contains a valid compact header. The tail region must be
            /// consistent with the header length prefixes.
            unsafe fn new_unchecked(data: &#data_lifetime mut [u8]) -> Self {
                let mut value = Self {
                    data,
                    total_len: 0,
                    #( #edit_inits, )*
                    #marker_init
                };
                value.total_len = value.current_encoded_len();
                value
            }

            fn header(&self) -> &#header_ty {
                unsafe { &*(self.data.as_ptr() as *const #header_ty) }
            }

            fn header_mut(&mut self) -> &mut #header_ty {
                unsafe { &mut *(self.data.as_mut_ptr() as *mut #header_ty) }
            }

            #( #inline_mut_accessors )*

            #( #setters )*

            fn current_encoded_len(&self) -> usize {
                let __hdr = self.header();
                #current_encoded_len
                __offset
            }

            fn try_projected_size(&self) -> Result<usize, pinapod::PinaPodError> {
                let mut __total = self.total_len;
                #( #projected_steps )*
                Ok(__total)
            }

            pub fn projected_size(&self) -> usize {
                self.try_projected_size().unwrap_or(usize::MAX)
            }

            #commit_body
        }
    }
}

fn generate_patch(
    schema: &Schema,
    header_ty: &TokenStream,
    ref_name: &syn::Ident,
    mut_name: &syn::Ident,
    patch_name: &syn::Ident,
) -> TokenStream {
    let struct_name = &schema.name;
    let patch_lifetime = fresh_lifetime(&schema.generics, "__pinapod_patch");
    let call_lifetime = fresh_lifetime(&schema.generics, "__pinapod_call");
    let patch_generics = generics_with_lifetime(&schema.generics, &patch_lifetime);
    let (patch_impl_generics, patch_ty_generics, patch_where_clause) =
        patch_generics.split_for_impl();
    let (impl_generics, ty_generics, where_clause) = schema.generics.split_for_impl();
    let bounds = compact_bounds(schema);
    let patch_where_clause_with_bounds =
        where_clause_with_bounds(patch_where_clause, bounds.iter());
    let where_clause_with_bounds = where_clause_with_bounds(where_clause, bounds.iter());
    let patch_call_ty = type_with_leading_lifetime(patch_name, &schema.generics, &call_lifetime);
    let ref_call_ty = type_with_leading_lifetime(ref_name, &schema.generics, &call_lifetime);
    let elided_lifetime: syn::Lifetime = syn::parse_quote!('_);
    let patch_elided_ty =
        type_with_leading_lifetime(patch_name, &schema.generics, &elided_lifetime);
    let ref_elided_ty = type_with_leading_lifetime(ref_name, &schema.generics, &elided_lifetime);
    let mut_elided_ty = type_with_leading_lifetime(mut_name, &schema.generics, &elided_lifetime);
    let (marker_field, marker_init) = generic_marker_tokens(&schema.generics);

    let mut fields = Vec::new();
    let mut field_inits = Vec::new();
    let mut builders = Vec::new();
    let mut input_validations = Vec::new();
    let mut updated_steps = Vec::new();
    let mut initial_steps = Vec::new();
    let mut stage_steps = Vec::new();
    let mut inline_writes = Vec::new();

    for field in &schema.fields {
        if field.skip_patch {
            continue;
        }
        let name = &field.name;
        field_inits.push(quote! { #name: None });
        match &field.kind {
            FieldKind::Inline => {
                let pod_ty = map_to_pod_type(&field.ty);
                fields.push(quote! { #name: Option<#pod_ty> });
                if let Some(inner) = option_inner_type(&field.ty) {
                    builders.push(quote! {
                        pub fn #name(
                            mut self,
                            value: impl pinapod::traits::IntoPodOption<#inner>,
                        ) -> Self {
                            self.#name = Some(
                                pinapod::traits::IntoPodOption::into_pod_option(value),
                            );
                            self
                        }
                    });
                } else {
                    builders.push(quote! {
                        pub fn #name(mut self, value: impl Into<#pod_ty>) -> Self {
                            self.#name = Some(value.into());
                            self
                        }
                    });
                }
                input_validations.push(quote! {
                    if let Some(value) = &self.#name {
                        <#pod_ty as pinapod::ZcValidate>::validate_ref(value)?;
                    }
                });
                let mut_method = format_ident!("{}_mut", name);
                inline_writes.push(quote! {
                    if let Some(value) = self.#name {
                        *writer.#mut_method() = value;
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::String { max, pfx },
            }) => {
                fields.push(quote! { #name: Option<&#patch_lifetime str> });
                builders.push(quote! {
                    pub fn #name(mut self, value: &#patch_lifetime str) -> Self {
                        self.#name = Some(value);
                        self
                    }
                });
                input_validations.push(quote! {
                    if let Some(value) = self.#name {
                        if value.len() > #max {
                            return Err(pinapod::PinaPodError::Overflow);
                        }
                        __pinapod_check_prefix(value.len(), #pfx)?;
                    }
                });
                updated_steps.push(quote! {
                    if let Some(value) = self.#name {
                        let old_len = view.#name().len();
                        updated_len = updated_len
                            .checked_sub(old_len)
                            .ok_or(pinapod::PinaPodError::Overflow)?;
                        updated_len = __pinapod_checked_add(updated_len, value.len())?;
                    }
                });
                initial_steps.push(quote! {
                    if let Some(value) = self.#name {
                        initialized_len = __pinapod_checked_add(initialized_len, value.len())?;
                    }
                });
                let edit = format_ident!("__{}_edit", name);
                stage_steps.push(quote! {
                    if let Some(value) = self.#name {
                        writer.#edit = Some((value.as_ptr(), value.len()));
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::Vec { elem, max, pfx },
            }) => {
                let mapped_elem = map_to_pod_type(elem);
                let replace = format_ident!("replace_{}", name);
                fields.push(quote! { #name: Option<&#patch_lifetime [#mapped_elem]> });
                builders.push(quote! {
                    pub fn #replace(mut self, value: &#patch_lifetime [#mapped_elem]) -> Self {
                        self.#name = Some(value);
                        self
                    }
                });
                input_validations.push(quote! {
                    if let Some(value) = self.#name {
                        if value.len() > #max {
                            return Err(pinapod::PinaPodError::Overflow);
                        }
                        __pinapod_check_prefix(value.len(), #pfx)?;
                        if core::mem::size_of::<#mapped_elem>() == 0 {
                            return Err(pinapod::PinaPodError::InvalidLength);
                        }
                        for item in value {
                            <#mapped_elem as pinapod::ZcValidate>::validate_ref(item)?;
                        }
                    }
                });
                updated_steps.push(quote! {
                    if let Some(value) = self.#name {
                        let old_len = __pinapod_checked_mul(
                            view.#name().len(),
                            core::mem::size_of::<#mapped_elem>(),
                        )?;
                        let new_len = __pinapod_checked_mul(
                            value.len(),
                            core::mem::size_of::<#mapped_elem>(),
                        )?;
                        updated_len = updated_len
                            .checked_sub(old_len)
                            .ok_or(pinapod::PinaPodError::Overflow)?;
                        updated_len = __pinapod_checked_add(updated_len, new_len)?;
                    }
                });
                initial_steps.push(quote! {
                    if let Some(value) = self.#name {
                        let new_len = __pinapod_checked_mul(
                            value.len(),
                            core::mem::size_of::<#mapped_elem>(),
                        )?;
                        initialized_len = __pinapod_checked_add(initialized_len, new_len)?;
                    }
                });
                let edit = format_ident!("__{}_edit", name);
                stage_steps.push(quote! {
                    if let Some(value) = self.#name {
                        writer.#edit = Some((
                            value.as_ptr() as *const u8,
                            value.len(),
                            core::mem::size_of::<#mapped_elem>(),
                        ));
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::String { max, pfx },
            }) => {
                fields.push(quote! { #name: Option<Option<&#patch_lifetime str>> });
                builders.push(quote! {
                    pub fn #name(mut self, value: Option<&#patch_lifetime str>) -> Self {
                        self.#name = Some(value);
                        self
                    }
                });
                input_validations.push(quote! {
                    if let Some(Some(value)) = self.#name {
                        if value.len() > #max {
                            return Err(pinapod::PinaPodError::Overflow);
                        }
                        __pinapod_check_prefix(value.len(), #pfx)?;
                    }
                });
                updated_steps.push(quote! {
                    if let Some(value) = self.#name {
                        let old_len = match view.#name() {
                            Some(old) => __pinapod_checked_add(#pfx, old.len())?,
                            None => 0,
                        };
                        let new_len = match value {
                            Some(new) => __pinapod_checked_add(#pfx, new.len())?,
                            None => 0,
                        };
                        updated_len = updated_len
                            .checked_sub(old_len)
                            .ok_or(pinapod::PinaPodError::Overflow)?;
                        updated_len = __pinapod_checked_add(updated_len, new_len)?;
                    }
                });
                initial_steps.push(quote! {
                    if let Some(Some(value)) = self.#name {
                        initialized_len = __pinapod_checked_add(
                            initialized_len,
                            __pinapod_checked_add(#pfx, value.len())?,
                        )?;
                    }
                });
                let edit = format_ident!("__{}_edit", name);
                stage_steps.push(quote! {
                    if let Some(value) = self.#name {
                        writer.#edit = Some(match value {
                            Some(value) => (value.as_ptr(), value.len()),
                            None => (core::ptr::null(), 0),
                        });
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::Vec { elem, max, pfx },
            }) => {
                let mapped_elem = map_to_pod_type(elem);
                let replace = format_ident!("replace_{}", name);
                fields.push(quote! { #name: Option<Option<&#patch_lifetime [#mapped_elem]>> });
                builders.push(quote! {
                    pub fn #replace(
                        mut self,
                        value: Option<&#patch_lifetime [#mapped_elem]>,
                    ) -> Self {
                        self.#name = Some(value);
                        self
                    }
                });
                input_validations.push(quote! {
                    if let Some(Some(value)) = self.#name {
                        if value.len() > #max {
                            return Err(pinapod::PinaPodError::Overflow);
                        }
                        __pinapod_check_prefix(value.len(), #pfx)?;
                        if core::mem::size_of::<#mapped_elem>() == 0 {
                            return Err(pinapod::PinaPodError::InvalidLength);
                        }
                        for item in value {
                            <#mapped_elem as pinapod::ZcValidate>::validate_ref(item)?;
                        }
                    }
                });
                updated_steps.push(quote! {
                    if let Some(value) = self.#name {
                        let old_len = match view.#name() {
                            Some(old) => __pinapod_checked_add(
                                #pfx,
                                __pinapod_checked_mul(
                                    old.len(),
                                    core::mem::size_of::<#mapped_elem>(),
                                )?,
                            )?,
                            None => 0,
                        };
                        let new_len = match value {
                            Some(new) => __pinapod_checked_add(
                                #pfx,
                                __pinapod_checked_mul(
                                    new.len(),
                                    core::mem::size_of::<#mapped_elem>(),
                                )?,
                            )?,
                            None => 0,
                        };
                        updated_len = updated_len
                            .checked_sub(old_len)
                            .ok_or(pinapod::PinaPodError::Overflow)?;
                        updated_len = __pinapod_checked_add(updated_len, new_len)?;
                    }
                });
                initial_steps.push(quote! {
                    if let Some(Some(value)) = self.#name {
                        initialized_len = __pinapod_checked_add(
                            initialized_len,
                            __pinapod_checked_add(
                                #pfx,
                                __pinapod_checked_mul(
                                    value.len(),
                                    core::mem::size_of::<#mapped_elem>(),
                                )?,
                            )?,
                        )?;
                    }
                });
                let edit = format_ident!("__{}_edit", name);
                stage_steps.push(quote! {
                    if let Some(value) = self.#name {
                        writer.#edit = Some(match value {
                            Some(value) => (
                                value.as_ptr() as *const u8,
                                value.len(),
                                core::mem::size_of::<#mapped_elem>(),
                            ),
                            None => (
                                core::ptr::null(),
                                0,
                                core::mem::size_of::<#mapped_elem>(),
                            ),
                        });
                    }
                });
            }
        }
    }

    let inherent = if schema.no_inherent {
        TokenStream::new()
    } else {
        quote! {
            impl #impl_generics #struct_name #ty_generics #where_clause_with_bounds {
                pub const HEADER_SIZE: usize =
                    <Self as pinapod::PinaPodCompact>::HEADER_SIZE;
                pub const MIN_SIZE: usize =
                    <Self as pinapod::PinaPodCompact>::MIN_SIZE;
                pub const MAX_SIZE: usize =
                    <Self as pinapod::PinaPodCompact>::MAX_SIZE;
                pub const TAIL_ALIGNMENT: usize =
                    <Self as pinapod::PinaPodCompact>::TAIL_ALIGNMENT;

                pub fn read_prefix<#call_lifetime>(
                    data: &#call_lifetime [u8],
                ) -> Result<#ref_call_ty, pinapod::PinaPodError> {
                    <#ref_call_ty>::new(data)
                }

                pub fn updated_len(
                    data: &[u8],
                    patch: &#patch_elided_ty,
                ) -> Result<usize, pinapod::PinaPodError> {
                    patch.updated_len(data)
                }

                pub fn update<#call_lifetime>(
                    data: &#call_lifetime mut [u8],
                    patch: &#call_lifetime #patch_call_ty,
                ) -> Result<usize, pinapod::PinaPodError> {
                    patch.update(data)
                }

                pub fn initialize<#call_lifetime>(
                    data: &#call_lifetime mut [u8],
                    patch: &#call_lifetime #patch_call_ty,
                ) -> Result<usize, pinapod::PinaPodError> {
                    patch.initialize(data)
                }
            }
        }
    };

    quote! {
        pub struct #patch_name #patch_generics #patch_where_clause_with_bounds {
            #( #fields, )*
            __pinapod_lifetime: core::marker::PhantomData<&#patch_lifetime ()>,
            #marker_field
        }

        impl #patch_impl_generics #patch_name #patch_ty_generics #patch_where_clause_with_bounds {
            pub fn new() -> Self {
                Self {
                    #( #field_inits, )*
                    __pinapod_lifetime: core::marker::PhantomData,
                    #marker_init
                }
            }

            #( #builders )*

            fn validate_inputs(&self) -> Result<(), pinapod::PinaPodError> {
                #( #input_validations )*
                Ok(())
            }

            pub fn updated_len(&self, data: &[u8]) -> Result<usize, pinapod::PinaPodError> {
                self.validate_inputs()?;
                let view = <#ref_elided_ty>::new(data)?;
                let mut updated_len = view.encoded_len();
                #( #updated_steps )*
                Ok(updated_len)
            }

            fn initialized_len(&self) -> Result<usize, pinapod::PinaPodError> {
                self.validate_inputs()?;
                let mut initialized_len = core::mem::size_of::<#header_ty>();
                #( #initial_steps )*
                Ok(initialized_len)
            }

            pub fn update(&self, data: &mut [u8]) -> Result<usize, pinapod::PinaPodError> {
                let expected_len = self.updated_len(data)?;
                if expected_len > data.len() {
                    return Err(pinapod::PinaPodError::BufferTooSmall);
                }
                let mut writer = unsafe {
                    // SAFETY: `updated_len` validated this exact buffer, and no
                    // bytes were mutated between that validation and this
                    // private writer construction.
                    <#mut_elided_ty>::new_unchecked(data)
                };
                #( #stage_steps )*
                let encoded_len = writer.commit()?;
                #( #inline_writes )*
                debug_assert_eq!(encoded_len, expected_len);
                Ok(encoded_len)
            }

            fn try_initialize(&self, data: &mut [u8]) -> Result<usize, pinapod::PinaPodError> {
                <#struct_name #ty_generics as pinapod::PinaPodCompact>::validate_storage_len(
                    data.len(),
                )?;
                let expected_len = self.initialized_len()?;
                if expected_len > data.len() {
                    return Err(pinapod::PinaPodError::BufferTooSmall);
                }
                let encoded_len = {
                    let mut writer = unsafe { <#mut_elided_ty>::new_unchecked(data) };
                    #( #stage_steps )*
                    let encoded_len = writer.commit()?;
                    #( #inline_writes )*
                    encoded_len
                };
                <#struct_name #ty_generics as pinapod::PinaPodCompact>::validate(
                    &data[..encoded_len],
                )?;
                debug_assert_eq!(encoded_len, expected_len);
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

        impl #patch_impl_generics
            pinapod::PinaPodPatch<#struct_name #ty_generics>
            for #patch_name #patch_ty_generics
            #patch_where_clause_with_bounds
        {
            fn updated_len(&self, data: &[u8]) -> Result<usize, pinapod::PinaPodError> {
                <#patch_name #patch_ty_generics>::updated_len(self, data)
            }

            fn update(&self, data: &mut [u8]) -> Result<usize, pinapod::PinaPodError> {
                <#patch_name #patch_ty_generics>::update(self, data)
            }

            fn initialize(&self, data: &mut [u8]) -> Result<usize, pinapod::PinaPodError> {
                <#patch_name #patch_ty_generics>::initialize(self, data)
            }
        }

        #inherent
    }
}

fn type_with_leading_lifetime(
    name: &syn::Ident,
    generics: &syn::Generics,
    lifetime: &syn::Lifetime,
) -> TokenStream {
    let arguments: Vec<_> = generics
        .params
        .iter()
        .map(|parameter| match parameter {
            syn::GenericParam::Lifetime(parameter) => {
                let lifetime = &parameter.lifetime;
                quote! { #lifetime }
            }
            syn::GenericParam::Type(parameter) => {
                let ident = &parameter.ident;
                quote! { #ident }
            }
            syn::GenericParam::Const(parameter) => {
                let ident = &parameter.ident;
                quote! { #ident }
            }
        })
        .collect();
    quote! { #name<#lifetime #(, #arguments)*> }
}

fn generate_commit_body(
    header_ty: &TokenStream,
    tail_fields: &[&crate::schema::SchemaField],
) -> TokenStream {
    if tail_fields.is_empty() {
        return quote! {
            pub fn commit(&mut self) -> Result<usize, pinapod::PinaPodError> {
                Ok(core::mem::size_of::<#header_ty>())
            }
        };
    }

    let header_size = quote! { core::mem::size_of::<#header_ty>() };

    // Step 1: compute per-field old/new offsets and lengths in field order.
    let mut setup_positions = Vec::new();
    for (i, f) in tail_fields.iter().enumerate() {
        let fname = &f.name;
        let edit_name = format_ident!("__{}_edit", fname);
        let len_name = format_ident!("__{}_len", fname);
        let old_off_var = format_ident!("__old_off_{}", fname);
        let new_off_var = format_ident!("__new_off_{}", fname);
        let old_len_var = format_ident!("__old_len_{}", fname);
        let new_len_var = format_ident!("__new_len_{}", fname);

        let offsets = if i == 0 {
            quote! {
                let #old_off_var: usize = #header_size;
                let #new_off_var: usize = #header_size;
            }
        } else {
            let prev_f = &tail_fields[i - 1];
            let prev_old_off = format_ident!("__old_off_{}", prev_f.name);
            let prev_new_off = format_ident!("__new_off_{}", prev_f.name);
            let prev_old_len = format_ident!("__old_len_{}", prev_f.name);
            let prev_new_len = format_ident!("__new_len_{}", prev_f.name);
            quote! {
                let #old_off_var: usize = __pinapod_checked_add(#prev_old_off, #prev_old_len)?;
                let #new_off_var: usize = __pinapod_checked_add(#prev_new_off, #prev_new_len)?;
            }
        };

        match &f.kind {
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::String { pfx, .. },
            }) => {
                let read_len = read_len_expr(&len_name, *pfx);
                setup_positions.push(quote! {
                    #offsets
                    let #old_len_var: usize = {
                        let __hdr = self.header();
                        #read_len
                    };
                    let #new_len_var: usize = match self.#edit_name {
                        Some((_, __bl)) => __bl,
                        None => #old_len_var,
                    };
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::Vec { elem, pfx, .. },
            }) => {
                let read_len = read_len_expr(&len_name, *pfx);
                let mapped_elem = map_to_pod_type(elem);
                setup_positions.push(quote! {
                    #offsets
                    let #old_len_var: usize = {
                        let __hdr = self.header();
                        let __count = #read_len;
                        __pinapod_checked_mul(
                            __count,
                            core::mem::size_of::<#mapped_elem>(),
                        )?
                    };
                    let #new_len_var: usize = match self.#edit_name {
                        Some((_, __count, __sz)) => __pinapod_checked_mul(__count, __sz)?,
                        None => #old_len_var,
                    };
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::String { pfx, .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", fname);
                let old_size =
                    old_option_string_size_expr(&tag_name, *pfx, quote! { #old_off_var });
                setup_positions.push(quote! {
                    #offsets
                    let #old_len_var: usize = {
                        let __hdr = self.header();
                        #old_size
                    };
                    let #new_len_var: usize = match self.#edit_name {
                        Some((ptr, __bl)) => {
                            if ptr.is_null() {
                                0
                            } else {
                                __pinapod_checked_add(#pfx, __bl)?
                            }
                        }
                        None => #old_len_var,
                    };
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::Vec { elem, pfx, .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", fname);
                let mapped_elem = map_to_pod_type(elem);
                let old_size = old_option_vec_size_expr(
                    &tag_name,
                    *pfx,
                    quote! { #old_off_var },
                    &mapped_elem,
                );
                setup_positions.push(quote! {
                    #offsets
                    let #old_len_var: usize = {
                        let __hdr = self.header();
                        #old_size
                    };
                    let #new_len_var: usize = match self.#edit_name {
                        Some((ptr, __count, __sz)) => {
                            if ptr.is_null() {
                                0
                            } else {
                                __pinapod_checked_add(
                                    #pfx,
                                    __pinapod_checked_mul(__count, __sz)?,
                                )?
                            }
                        }
                        None => #old_len_var,
                    };
                });
            }
            _ => unreachable!(),
        }
    }

    let last_f = tail_fields.last().unwrap();
    let last_new_off = format_ident!("__new_off_{}", last_f.name);
    let last_new_len = format_ident!("__new_len_{}", last_f.name);
    let range_checks: Vec<_> = tail_fields
        .iter()
        .map(|field| {
            let old_off = format_ident!("__old_off_{}", field.name);
            let new_off = format_ident!("__new_off_{}", field.name);
            let old_len = format_ident!("__old_len_{}", field.name);
            let new_len = format_ident!("__new_len_{}", field.name);
            quote! {
                let __old_end = __pinapod_checked_add(#old_off, #old_len)?;
                if __old_end > self.total_len {
                    return Err(pinapod::PinaPodError::BufferTooSmall);
                }
                let __new_end = __pinapod_checked_add(#new_off, #new_len)?;
                if __new_end > self.data.len() {
                    return Err(pinapod::PinaPodError::BufferTooSmall);
                }
            }
        })
        .collect();

    // Phase 1a: unedited fields that shift backward, in forward iteration order.
    // Phase 1b: unedited fields that shift forward, in reverse iteration order.
    // This two-pass ordering ensures source bytes are never read after being
    // overwritten by an earlier step, regardless of mixed grow/shrink edits.
    let mut phase_1a = Vec::new();
    for f in tail_fields {
        let fname = &f.name;
        let edit_name = format_ident!("__{}_edit", fname);
        let old_off_var = format_ident!("__old_off_{}", fname);
        let new_off_var = format_ident!("__new_off_{}", fname);
        let old_len_var = format_ident!("__old_len_{}", fname);
        phase_1a.push(quote! {
            if self.#edit_name.is_none()
                && #new_off_var < #old_off_var
                && #old_len_var > 0
            {
                unsafe {
                    core::ptr::copy(
                        __buf_ptr.add(#old_off_var) as *const u8,
                        __buf_ptr.add(#new_off_var),
                        #old_len_var,
                    );
                }
            }
        });
    }

    let mut phase_1b = Vec::new();
    for f in tail_fields.iter().rev() {
        let fname = &f.name;
        let edit_name = format_ident!("__{}_edit", fname);
        let old_off_var = format_ident!("__old_off_{}", fname);
        let new_off_var = format_ident!("__new_off_{}", fname);
        let old_len_var = format_ident!("__old_len_{}", fname);
        phase_1b.push(quote! {
            if self.#edit_name.is_none()
                && #new_off_var > #old_off_var
                && #old_len_var > 0
            {
                unsafe {
                    core::ptr::copy(
                        __buf_ptr.add(#old_off_var) as *const u8,
                        __buf_ptr.add(#new_off_var),
                        #old_len_var,
                    );
                }
            }
        });
    }

    // Phase 2: write edited fields to their final positions.
    let mut phase_2 = Vec::new();
    for f in tail_fields {
        let fname = &f.name;
        let edit_name = format_ident!("__{}_edit", fname);
        let new_off_var = format_ident!("__new_off_{}", fname);
        match &f.kind {
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::String { .. },
            }) => {
                phase_2.push(quote! {
                    if let Some((__src_ptr, __new_byte_len)) = self.#edit_name {
                        if __new_byte_len > 0 {
                            let __source = unsafe {
                                core::slice::from_raw_parts(__src_ptr, __new_byte_len)
                            };
                            let __end = __pinapod_checked_add(
                                #new_off_var,
                                __new_byte_len,
                            )?;
                            self.data[#new_off_var..__end].copy_from_slice(__source);
                        }
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::Vec { .. },
            }) => {
                let new_len_var = format_ident!("__new_len_{}", fname);
                phase_2.push(quote! {
                    if let Some((__src_ptr, _, _)) = self.#edit_name {
                        if #new_len_var > 0 {
                            let __source = unsafe {
                                core::slice::from_raw_parts(__src_ptr, #new_len_var)
                            };
                            let __end = __pinapod_checked_add(
                                #new_off_var,
                                #new_len_var,
                            )?;
                            self.data[#new_off_var..__end].copy_from_slice(__source);
                        }
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::String { pfx, .. },
            }) => {
                phase_2.push(quote! {
                    if let Some((__src_ptr, __new_byte_len)) = self.#edit_name {
                        if !__src_ptr.is_null() {
                            __pinapod_write_prefix(
                                self.data,
                                #new_off_var,
                                #pfx,
                                __new_byte_len,
                            )?;
                            let __payload_offset = __pinapod_checked_add(#new_off_var, #pfx)?;
                            if __new_byte_len > 0 {
                                let __source = unsafe {
                                    core::slice::from_raw_parts(__src_ptr, __new_byte_len)
                                };
                                let __payload_end = __pinapod_checked_add(
                                    __payload_offset,
                                    __new_byte_len,
                                )?;
                                self.data[__payload_offset..__payload_end]
                                    .copy_from_slice(__source);
                            }
                        }
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::Vec { pfx, .. },
            }) => {
                phase_2.push(quote! {
                    if let Some((__src_ptr, __count, __elem_size)) = self.#edit_name {
                        if !__src_ptr.is_null() {
                            __pinapod_write_prefix(
                                self.data,
                                #new_off_var,
                                #pfx,
                                __count,
                            )?;
                            let __new_byte_len = __pinapod_checked_mul(__count, __elem_size)?;
                            let __payload_offset = __pinapod_checked_add(#new_off_var, #pfx)?;
                            if __new_byte_len > 0 {
                                let __source = unsafe {
                                    core::slice::from_raw_parts(__src_ptr, __new_byte_len)
                                };
                                let __payload_end = __pinapod_checked_add(
                                    __payload_offset,
                                    __new_byte_len,
                                )?;
                                self.data[__payload_offset..__payload_end]
                                    .copy_from_slice(__source);
                            }
                        }
                    }
                });
            }
            _ => unreachable!(),
        }
    }

    // Update header length prefixes for edited fields.
    let mut update_lens = Vec::new();
    for f in tail_fields {
        let edit_name = format_ident!("__{}_edit", f.name);
        let len_name = format_ident!("__{}_len", f.name);
        let pfx_lit = tail_pfx(&f.kind);

        match &f.kind {
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::String { .. },
            }) => {
                update_lens.push(quote! {
                    if let Some((_, __new_byte_len)) = self.#edit_name {
                        __pinapod_write_prefix(
                            &mut self.header_mut().#len_name,
                            0,
                            #pfx_lit,
                            __new_byte_len,
                        )?;
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::Vec { .. },
            }) => {
                update_lens.push(quote! {
                    if let Some((_, __count, _)) = self.#edit_name {
                        __pinapod_write_prefix(
                            &mut self.header_mut().#len_name,
                            0,
                            #pfx_lit,
                            __count,
                        )?;
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::String { .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", f.name);
                update_lens.push(quote! {
                    if let Some((ptr, _)) = self.#edit_name {
                        self.header_mut().#tag_name[0] = if ptr.is_null() { 0 } else { 1 };
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::Vec { .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", f.name);
                update_lens.push(quote! {
                    if let Some((ptr, _, _)) = self.#edit_name {
                        self.header_mut().#tag_name[0] = if ptr.is_null() { 0 } else { 1 };
                    }
                });
            }
            _ => unreachable!(),
        }
    }

    let clear_edits: Vec<_> = tail_fields
        .iter()
        .map(|f| {
            let edit_name = format_ident!("__{}_edit", f.name);
            quote! { self.#edit_name = None; }
        })
        .collect();

    quote! {
        pub fn commit(&mut self) -> Result<usize, pinapod::PinaPodError> {
            #( #setup_positions )*
            #( #range_checks )*

            let __old_total = self.total_len;
            let __final_total: usize = __pinapod_checked_add(#last_new_off, #last_new_len)?;
            if __final_total > self.data.len() {
                return Err(pinapod::PinaPodError::BufferTooSmall);
            }

            let __buf_ptr = self.data.as_mut_ptr();

            // Move unedited fields that shift to a lower offset. Forward
            // iteration is safe here because writing to a lower address never
            // clobbers the source bytes of a later unedited field.
            #( #phase_1a )*

            // Move unedited fields that shift to a higher offset. Reverse
            // iteration is required so earlier fields still read untouched
            // source bytes even after later fields have been moved forward.
            #( #phase_1b )*

            // All unedited tail bytes are now at their final positions, so we
            // can safely copy caller-provided data into the edited slots
            // without risking aliasing with remaining unedited data.
            #( #phase_2 )*

            #( #update_lens )*

            if __final_total < __old_total {
                self.data[__final_total..__old_total].fill(0);
            }

            self.total_len = __final_total;
            #( #clear_edits )*

            Ok(__final_total)
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn compact_max_size(schema: &Schema, header_ty: &TokenStream) -> TokenStream {
    let steps = schema.tail_fields().map(|field| {
        let FieldKind::Tail(TailField::Segment { presence, payload }) = &field.kind else {
            unreachable!("compact maximum requested for an inline field")
        };
        let payload_max = match payload {
            TailPayload::String { max, .. } => quote! { (#max as usize) },
            TailPayload::Vec { elem, max, .. } => {
                let mapped_elem = map_to_pod_type(elem);
                quote! {
                    match (#max as usize).checked_mul(core::mem::size_of::<#mapped_elem>()) {
                        Some(value) => value,
                        None => panic!("compact schema maximum size overflows usize"),
                    }
                }
            }
        };
        let contribution = match presence {
            TailPresence::Always => payload_max,
            TailPresence::OptionTag => {
                let prefix = payload.pfx();
                quote! {
                    match (#payload_max).checked_add(#prefix) {
                        Some(value) => value,
                        None => panic!("compact schema maximum size overflows usize"),
                    }
                }
            }
        };
        quote! {
            __size = match __size.checked_add(#contribution) {
                Some(value) => value,
                None => panic!("compact schema maximum size overflows usize"),
            };
        }
    });

    quote! {{
        let mut __size = core::mem::size_of::<#header_ty>();
        #( #steps )*
        __size
    }}
}

fn compact_tail_alignment(schema: &Schema) -> TokenStream {
    let steps = schema.tail_fields().flat_map(|field| {
        let FieldKind::Tail(TailField::Segment { presence, payload }) = &field.kind else {
            unreachable!("compact alignment requested for an inline field")
        };
        let mut atoms = Vec::new();
        match payload {
            TailPayload::String { .. } => atoms.push(quote! { 1usize }),
            TailPayload::Vec { elem, .. } => {
                let mapped_elem = map_to_pod_type(elem);
                atoms.push(quote! { core::mem::size_of::<#mapped_elem>() });
            }
        }
        if matches!(presence, TailPresence::OptionTag) {
            let prefix = payload.pfx();
            atoms.push(quote! { #prefix });
        }
        atoms
    });

    quote! {{
        let mut __alignment = 0usize;
        #(
            __alignment = __pinapod_gcd(__alignment, #steps);
        )*
        if __alignment == 0 { 1 } else { __alignment }
    }}
}

fn compact_marker_type(field: &crate::schema::SchemaField) -> TokenStream {
    let FieldKind::Tail(TailField::Segment { presence, payload }) = &field.kind else {
        unreachable!("compact marker requested for an inline field")
    };
    let payload = match payload {
        TailPayload::String { max, pfx } => {
            quote! { pinapod::pod::PodString<#max, #pfx> }
        }
        TailPayload::Vec { elem, max, pfx } => {
            let mapped_elem = map_to_pod_type(elem);
            quote! { pinapod::pod::PodVecRepr<#mapped_elem, #max, #pfx> }
        }
    };

    match presence {
        TailPresence::Always => payload,
        TailPresence::OptionTag => quote! { core::option::Option<#payload> },
    }
}

fn compact_capacity_checks(field: &crate::schema::SchemaField) -> TokenStream {
    let FieldKind::Tail(TailField::Segment { payload, .. }) = &field.kind else {
        unreachable!("compact capacity check requested for an inline field")
    };
    match payload {
        TailPayload::String { max, pfx } => quote! {
            let _ = pinapod::pod::PodString::<#max, #pfx>::VALID;
        },
        TailPayload::Vec { elem, max, pfx } => {
            let mapped_elem = map_to_pod_type(elem);
            quote! {
                let _ = pinapod::pod::PodVec::<u8, #max, #pfx>::VALID;
                let _ = const {
                    assert!(
                        core::mem::size_of::<#mapped_elem>() != 0,
                        "compact vector elements must not be zero-sized",
                    );
                };
            }
        }
    }
}

fn compact_bounds(schema: &Schema) -> Vec<TokenStream> {
    let mut bounds: Vec<TokenStream> = schema
        .inline_fields()
        .map(|f| {
            let pod_ty = map_to_pod_type(&f.ty);
            quote! { #pod_ty: pinapod::ZcElem }
        })
        .collect();

    bounds.extend(schema.tail_fields().filter_map(|f| match &f.kind {
        FieldKind::Tail(TailField::Segment {
            payload: TailPayload::Vec { elem, .. },
            ..
        }) => {
            let mapped_elem = map_to_pod_type(elem);
            Some(quote! { #mapped_elem: pinapod::ZcElem })
        }
        _ => None,
    }));

    bounds
}

fn where_clause_with_bounds<'a>(
    where_clause: Option<&syn::WhereClause>,
    bounds: impl IntoIterator<Item = &'a TokenStream>,
) -> TokenStream {
    let bounds: Vec<&TokenStream> = bounds.into_iter().collect();
    match (where_clause, bounds.is_empty()) {
        (Some(existing), false) => {
            let predicates = existing.predicates.iter();
            quote! { where #(#predicates,)* #(#bounds,)* }
        }
        (Some(existing), true) => quote! { #existing },
        (None, false) => quote! { where #(#bounds,)* },
        (None, true) => quote! {},
    }
}

fn generics_with_lifetime(generics: &syn::Generics, lifetime: &syn::Lifetime) -> syn::Generics {
    let mut generics = generics.clone();
    generics.params.insert(0, syn::parse_quote!(#lifetime));
    generics
}

fn fresh_lifetime(generics: &syn::Generics, base: &str) -> syn::Lifetime {
    let mut candidate = base.to_owned();
    let mut suffix = 0_u32;

    while generics
        .lifetimes()
        .any(|parameter| parameter.lifetime.ident == candidate)
    {
        suffix += 1;
        candidate = format!("{base}_{suffix}");
    }

    syn::Lifetime::new(&format!("'{candidate}"), proc_macro2::Span::call_site())
}

fn generic_marker_tokens(generics: &syn::Generics) -> (TokenStream, TokenStream) {
    let marker_types: Vec<TokenStream> = generics
        .params
        .iter()
        .filter_map(|parameter| match parameter {
            syn::GenericParam::Lifetime(parameter) => {
                let lifetime = &parameter.lifetime;
                Some(quote! { &#lifetime () })
            }
            syn::GenericParam::Type(parameter) => {
                let ident = &parameter.ident;
                Some(quote! { *const #ident })
            }
            syn::GenericParam::Const(_) => None,
        })
        .collect();

    if marker_types.is_empty() {
        return (TokenStream::new(), TokenStream::new());
    }

    (
        quote! {
            __pinapod_type_marker: core::marker::PhantomData<fn(#(#marker_types),*)>,
        },
        quote! {
            __pinapod_type_marker: core::marker::PhantomData,
        },
    )
}

fn read_len_expr(len_name: &syn::Ident, pfx: usize) -> TokenStream {
    match pfx {
        1 => quote! { __hdr.#len_name[0] as usize },
        2 => quote! { u16::from_le_bytes(__hdr.#len_name) as usize },
        4 => quote! { u32::from_le_bytes(__hdr.#len_name) as usize },
        8 => quote! { u64::from_le_bytes(__hdr.#len_name) as usize },
        _ => unreachable!("invalid PFX: {}", pfx),
    }
}

fn read_self_data_len_expr(offset: TokenStream, pfx: usize) -> TokenStream {
    match pfx {
        1 => quote! { self.data[#offset] as usize },
        2 => quote! { u16::from_le_bytes([self.data[#offset], self.data[#offset + 1]]) as usize },
        4 => quote! {
            u32::from_le_bytes([
                self.data[#offset],
                self.data[#offset + 1],
                self.data[#offset + 2],
                self.data[#offset + 3],
            ]) as usize
        },
        8 => quote! {
            u64::from_le_bytes([
                self.data[#offset],
                self.data[#offset + 1],
                self.data[#offset + 2],
                self.data[#offset + 3],
                self.data[#offset + 4],
                self.data[#offset + 5],
                self.data[#offset + 6],
                self.data[#offset + 7],
            ]) as usize
        },
        _ => unreachable!("invalid PFX: {}", pfx),
    }
}

fn old_option_string_size_expr(
    tag_name: &syn::Ident,
    pfx: usize,
    offset: TokenStream,
) -> TokenStream {
    quote! {
        match __hdr.#tag_name[0] {
            0 => 0,
            1 => {
                let __byte_len = __pinapod_read_prefix(self.data, #offset, #pfx)?;
                let __encoded_len = __pinapod_checked_add(#pfx, __byte_len)?;
                let __end = __pinapod_checked_add(#offset, __encoded_len)?;
                if __end > self.total_len {
                    return Err(pinapod::PinaPodError::BufferTooSmall);
                }
                __encoded_len
            }
            _ => return Err(pinapod::PinaPodError::InvalidTag),
        }
    }
}

fn old_option_vec_size_expr(
    tag_name: &syn::Ident,
    pfx: usize,
    offset: TokenStream,
    mapped_elem: &TokenStream,
) -> TokenStream {
    quote! {
        match __hdr.#tag_name[0] {
            0 => 0,
            1 => {
                let __count = __pinapod_read_prefix(self.data, #offset, #pfx)?;
                let __byte_len = __pinapod_checked_mul(
                    __count,
                    core::mem::size_of::<#mapped_elem>(),
                )?;
                let __encoded_len = __pinapod_checked_add(#pfx, __byte_len)?;
                let __end = __pinapod_checked_add(#offset, __encoded_len)?;
                if __end > self.total_len {
                    return Err(pinapod::PinaPodError::BufferTooSmall);
                }
                __encoded_len
            }
            _ => return Err(pinapod::PinaPodError::InvalidTag),
        }
    }
}

fn tail_pfx(kind: &FieldKind) -> usize {
    match kind {
        FieldKind::Tail(tail) => tail.payload().pfx(),
        _ => unreachable!(),
    }
}

fn compute_offset_tokens(
    header_ty: &TokenStream,
    tail_fields: &[&crate::schema::SchemaField],
    target_index: usize,
) -> TokenStream {
    let header_size = quote! { core::mem::size_of::<#header_ty>() };
    let mut steps = Vec::new();
    for f in &tail_fields[..target_index] {
        let len_name = format_ident!("__{}_len", f.name);
        let pfx = tail_pfx(&f.kind);
        let read_len = read_len_expr(&len_name, pfx);

        match &f.kind {
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::String { .. },
            }) => {
                steps.push(quote! {
                    __offset += #read_len;
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::Vec { elem, .. },
            }) => {
                let count_name = format_ident!("__{}_offset_count", f.name);
                let mapped_elem = map_to_pod_type(elem);
                steps.push(quote! {
                    let #count_name = #read_len;
                    __offset += #count_name * core::mem::size_of::<#mapped_elem>();
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::String { pfx, .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", f.name);
                let read_len = read_self_data_len_expr(quote! { __offset }, *pfx);
                steps.push(quote! {
                    if __hdr.#tag_name[0] != 0 {
                        let __byte_len = #read_len;
                        __offset += #pfx + __byte_len;
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::Vec { elem, pfx, .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", f.name);
                let mapped_elem = map_to_pod_type(elem);
                let read_len = read_self_data_len_expr(quote! { __offset }, *pfx);
                steps.push(quote! {
                    if __hdr.#tag_name[0] != 0 {
                        let __count = #read_len;
                        __offset += #pfx + __count * core::mem::size_of::<#mapped_elem>();
                    }
                });
            }
            _ => unreachable!(),
        }
    }

    quote! {
        let mut __offset = #header_size;
        #( #steps )*
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_markers_cover_lifetimes_and_types_without_const_layout_changes() {
        let generics: syn::Generics = syn::parse_quote!(<'source, T, const CAPACITY: usize>);

        let (field, init) = generic_marker_tokens(&generics);
        let field = field.to_string();

        assert!(field.contains("'source"));
        assert!(field.contains("* const T"));
        assert!(!field.contains("CAPACITY"));
        assert!(init.to_string().contains("PhantomData"));
    }

    #[test]
    fn concrete_compact_types_do_not_receive_a_marker() {
        let generics = syn::Generics::default();

        let (field, init) = generic_marker_tokens(&generics);

        assert!(field.is_empty());
        assert!(init.is_empty());
    }
}
