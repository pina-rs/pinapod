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
    reason = "PinaPod's audited zero-copy primitives require narrowly scoped unsafe operations"
)]
pub mod pod;
#[allow(
    clippy::inline_always,
    clippy::ptr_as_ptr,
    clippy::ref_as_ptr,
    clippy::wildcard_imports,
    unsafe_code,
    unused_qualifications,
    reason = "PinaPod's audited byte-casting contracts require narrowly scoped unsafe operations"
)]
pub mod traits;

pub use {
    error::PinaPodError,
    pinapod_derive::PinaPod,
    pod::{PodString, PodVec},
    traits::{PinaPod, PinaPodCompact, PinaPodFixed, PinaPodPatch, ZcElem, ZcField, ZcValidate},
};

// Schema-friendly aliases to pod storage types.
// These are NOT a separate abstraction layer — they ARE PodString/PodVec
// with default prefix sizes.
pub type String<const N: usize> = PodString<N, 1>;

/// Schema-friendly vector with a two-byte length prefix.
///
/// Use [`pod::PodVec`] directly when the wire format needs an explicit prefix
/// width, for example `PodVec<u64, 1024, 2>`.
pub type Vec<T, const N: usize> = PodVec<T, N, 2>;
