//! Alignment-one, zero-copy representations for Solana account and instruction bytes.
//!
//! `PinaPod` maps validated account bytes to Rust types whose stored layout is exactly the
//! wire layout. The derive generates the representation, and every safe reader proves the
//! representation's invariants before it hands out a reference.
//!
//! <!-- {=podAlignmentAndValidationContract|trim|linePrefix:"//! ":true} -->
//! All representations have alignment one, so a stored field can be read at any byte offset without a copy or a relocation.
//!
//! Safe readers validate tags, lengths, UTF-8, enum discriminants, nested values, and slice bounds before they return a reference.<!-- {/podAlignmentAndValidationContract} -->
//!
//! # Layouts
//!
//! A fixed layout always occupies `Type::SIZE` bytes, so a field offset never moves and
//! every access is direct after validation. A compact layout keeps fixed fields in a
//! header and packs active string and vector bytes after that header, so the allocation
//! tracks active data at the cost of a resize lifecycle.
//!
//! # Example
//!
//! A derive generates a schema over these pod types. The pods themselves are the
//! everyday API for writing and reading a bounded value:
//!
//! ```
//! use pinapod::PodString;
//! use pinapod::PodVec;
//!
//! let mut display_name = PodString::<32>::default();
//! display_name.try_set("ifi")?;
//!
//! let mut roles = PodVec::<u16, 8>::default();
//! roles.try_set([7_u16, 11])?;
//!
//! assert_eq!(display_name.as_str(), "ifi");
//! assert_eq!(roles[0], 7);
//! # Ok::<(), pinapod::PinaPodError>(())
//! ```
//!
//! The same containers appear in a schema as `String<32>` and `Vec<u16, 8>`, and the
//! derive generates the alignment-one representation plus its reader and writer. The
//! [book](https://pina-rs.github.io/pinapod/) walks through a full schema.
//!
//! # Pod types
//!
//! <!-- {=podTypesTable|trim|linePrefix:"//! ":true} -->
//! | Type                       |                     Stored size | Meaning                                     |
//! | -------------------------- | ------------------------------: | ------------------------------------------- |
//! | `PodU16` through `PodU128` |              2 through 16 bytes | Unsigned, little-endian integer             |
//! | `PodI16` through `PodI128` |              2 through 16 bytes | Signed, little-endian integer               |
//! | `PodBool`                  |                          1 byte | Boolean with a `0` or `1` byte              |
//! | `PodF32`                   |                         4 bytes | IEEE-754 binary32 stored as its bit pattern |
//! | `PodF64`                   |                         8 bytes | IEEE-754 binary64 stored as its bit pattern |
//! | `PodOption<T, PFX>`        |          `PFX + size_of::<T>()` | Optional fixed representation               |
//! | `PodString<N, PFX>`        |                       `PFX + N` | UTF-8 string with at most `N` bytes         |
//! | `PodVec<T, N, PFX>`        | `PFX + N * mapped element size` | Vector with at most `N` mapped pod elements |<!-- {/podTypesTable} -->
//!
//! <!-- {=podSchemaAliases|trim|linePrefix:"//! ":true} -->
//! The schema aliases choose common prefix widths, so ordinary declarations stay short:
//!
//! - `String<N>` is `PodString<N, 1>`.
//! - `Vec<T, N>` is `PodVec<T, N, 2>`.<!-- {/podSchemaAliases} -->
//!
//! # Features
//!
//! <!-- {=podFeatureTable|trim|linePrefix:"//! ":true} -->
//! | Feature                | Adds                                                     |
//! | ---------------------- | -------------------------------------------------------- |
//! | `fixed`                | Mappings for signed and unsigned `fixed` 1.30.0 values   |
//! | `floats`               | `PodF32`/`PodF64` and mappings for native `f32`/`f64`    |
//! | `solana-address`       | A mapping for `solana_address::Address`                  |
//! | `solana-program-error` | Conversion from `PinaPodError` to `ProgramError`         |
//! | `wincode`              | Canonical `SchemaRead` and `SchemaWrite` implementations |<!-- {/podFeatureTable} -->
//!
//! <!-- {=podFeatureDefaultsContract|trim|linePrefix:"//! ":true} -->
//! No feature is enabled by default, so the core crate stays `no_std` and dependency-free.
//!
//! Enable only what a program reads from or writes to the wire.<!-- {/podFeatureDefaultsContract} -->
//!
//! # Errors
//!
//! <!-- {=podErrorContract|trim|linePrefix:"//! ":true} -->
//! | Variant               | Meaning                                                                      |
//! | --------------------- | ---------------------------------------------------------------------------- |
//! | `BufferTooSmall`      | The supplied slice cannot contain the required header, value, or active tail |
//! | `Overflow`            | A requested write exceeds a field capacity or checked arithmetic fails       |
//! | `InvalidBool`         | A stored boolean byte is not zero or one                                     |
//! | `InvalidTag`          | A stored option tag is not zero or one                                       |
//! | `InvalidDiscriminant` | A stored enum value has no declared variant                                  |
//! | `InvalidLength`       | A stored length exceeds capacity or violates the read contract               |
//! | `InvalidUtf8`         | Active string bytes are not UTF-8                                            |<!-- {/podErrorContract} -->
//!
//! # Safety model
//!
//! Forming a reference over account bytes is a memory-safety operation rather than a
//! data-quality check. [`ZcElem`] is the central unsafe contract: implementors guarantee
//! alignment one, no padding, validity for every bit pattern, and a load-bearing
//! [`ZcValidate`].
//!
//! Prefer `#[derive(PinaPod)]`. Manual implementations of [`PinaPodFixed`],
//! [`PinaPodCompact`], [`ZcElem`], and [`ZcField`] are an advanced raw API.
//!
//! # Documentation
//!
//! The [PinaPod book](https://pina-rs.github.io/pinapod/) covers layout choices,
//! migration steps, and the safety model. Use this API reference for item signatures.
//!
//! <!-- {=podMdtManagedDocNote|trim|linePrefix:"//! ":true} -->
//! This section is synchronized by `mdt` and expands from `api-docs.t.md`. Edit the provider, then run `devenv shell docs:sync`.<!-- {/podMdtManagedDocNote} -->

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

pub use error::PinaPodError;
pub use pinapod_derive::PinaPod;
pub use pod::PodString;
pub use pod::PodVec;
pub use traits::PinaPod;
pub use traits::PinaPodCompact;
pub use traits::PinaPodFixed;
pub use traits::PinaPodPatch;
pub use traits::ZcElem;
pub use traits::ZcField;
pub use traits::ZcValidate;

// Kani proofs over derive-generated schemas. The derive expands audited
// unsafe readers inside this module, so the workspace unsafe denial is
// lifted for it exactly like the handwritten pod modules.
#[cfg(all(kani, feature = "kani"))]
#[allow(
	unsafe_code,
	reason = "the derive expands PinaPod's audited byte-casting implementation inside the proof \
	          module"
)]
mod generated_proofs;

/// Schema-friendly string with a one-byte length prefix.
///
/// This is an alias, not a separate type: it is [`PodString`] with `PFX = 1`. Use
/// [`pod::PodString`] directly when the wire format needs an explicit prefix width,
/// for example `PodString<300, 2>`.
///
/// <!-- {=podPrefixWidthRule|trim|linePrefix:"/// ":true} -->
/// `PFX` is the width in bytes of the length prefix or tag that precedes the payload, and it must be `1`, `2`, `4`, or `8`.<!-- {/podPrefixWidthRule} -->
///
/// <!-- {=podStringCapacityRule|trim|linePrefix:"/// ":true} -->
/// The capacity must fit that prefix: `String<255>` is valid, `String<256>` is not, and `PodString<256, 2>` restores it.<!-- {/podStringCapacityRule} -->
///
/// <!-- {=podCapacityOverflowAdvice|trim|linePrefix:"/// ":true} -->
/// Choose the capacity from the largest value the schema must hold, because a write that does not fit is rejected rather than truncated.<!-- {/podCapacityOverflowAdvice} -->
pub type String<const N: usize> = PodString<N, 1>;

/// Schema-friendly vector with a two-byte length prefix.
///
/// This is an alias, not a separate type: it is [`PodVec`] with `PFX = 2`, and its
/// elements are the mapped pods of `T`, so `Vec<u64, 8>` stores `PodU64` elements.
/// Use [`pod::PodVec`] directly when the wire format needs an explicit prefix width,
/// for example `PodVec<u64, 1024, 4>`.
///
/// <!-- {=podPrefixWidthRule|trim|linePrefix:"/// ":true} -->
/// `PFX` is the width in bytes of the length prefix or tag that precedes the payload, and it must be `1`, `2`, `4`, or `8`.<!-- {/podPrefixWidthRule} -->
///
/// <!-- {=podVecCapacityRule|trim|linePrefix:"/// ":true} -->
/// The element count must fit that prefix: `Vec<u64, 255>` is valid, `Vec<u64, 256>` is not, and `PodVec<u64, 256, 2>` restores it.<!-- {/podVecCapacityRule} -->
///
/// <!-- {=podCapacityOverflowAdvice|trim|linePrefix:"/// ":true} -->
/// Choose the capacity from the largest value the schema must hold, because a write that does not fit is rejected rather than truncated.<!-- {/podCapacityOverflowAdvice} -->
pub type Vec<T, const N: usize> = PodVec<T, N, 2>;
