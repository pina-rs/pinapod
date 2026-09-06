#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(
    kani,
    allow(
        unstable_features,
        reason = "Kani injects the unstable register_tool feature while compiling proof harnesses"
    )
)]

pub mod error;
#[allow(
    clippy::cast_lossless,
    clippy::ignored_unit_patterns,
    clippy::inline_always,
    clippy::ptr_as_ptr,
    clippy::ref_as_ptr,
    clippy::single_match_else,
    clippy::uninlined_format_args,
    unsafe_code,
    unused_qualifications,
    reason = "Pinapod's audited zero-copy primitives require narrowly scoped unsafe operations"
)]
pub mod pod;
#[allow(
    clippy::inline_always,
    clippy::ptr_as_ptr,
    clippy::ref_as_ptr,
    clippy::wildcard_imports,
    unsafe_code,
    unused_qualifications,
    reason = "Pinapod's audited byte-casting contracts require narrowly scoped unsafe operations"
)]
pub mod traits;

pub use {
    error::ZeroPodError,
    pinapod_derive::ZeroPod,
    traits::{
        LayoutKind, ZcElem, ZcField, ZcValidate, ZeroPodCompact, ZeroPodFixed, ZeroPodSchema,
    },
};

// Schema-friendly aliases to pod storage types.
// These are NOT a separate abstraction layer — they ARE PodString/PodVec
// with default prefix sizes.
pub type String<const N: usize> = pod::PodString<N, 1>;

/// Schema-friendly Vec alias. Maps native types to their pod companions
/// via `ZcField`, so `Vec<u64, 8>` becomes `PodVec<PodU64, 8, 2>`.
#[allow(type_alias_bounds)]
pub type Vec<T: ZcField<Pod: ZcElem>, const N: usize> = pod::PodVec<<T as ZcField>::Pod, N, 2>;
