//! Alignment-one storage types for schema fields.
//!
//! These are the representations a schema field maps to. Integer, boolean, and float
//! pods are `#[repr(transparent)]` over their little-endian bytes; the container pods
//! store a length prefix or tag followed by fixed-capacity payload slots.
//!
//! <!-- {=podAlignmentAndValidationContract|trim|linePrefix:"//! ":true} -->
//! All representations have alignment one, so a stored field can be read at any byte offset without a copy or a relocation.
//!
//! Safe readers validate tags, lengths, UTF-8, enum discriminants, nested values, and slice bounds before they return a reference.<!-- {/podAlignmentAndValidationContract} -->
//!
//! <!-- {=podZeroedInactiveCapacityContract|trim|linePrefix:"//! ":true} -->
//! Every container starts with fully initialized backing storage.
//!
//! Operations that shorten or clear active data zero the bytes they vacate, so a later raw read or canonical serialization cannot disclose a previous value.<!-- {/podZeroedInactiveCapacityContract} -->
//!
//! Read a stored value through an accessor rather than by casting bytes: a pod may sit at
//! an offset that its own alignment would forbid for the native type. The containers
//! expose `as_str` and `as_slice`, while the numeric and float pods expose `get` and
//! `set` to cross the unaligned boundary.
//!
//! <!-- {=podMdtManagedDocNote|trim|linePrefix:"//! ":true} -->
//! This section is synchronized by `mdt` and expands from `api-docs.t.md`. Edit the provider, then run `devenv shell docs:sync`.<!-- {/podMdtManagedDocNote} -->

mod bool;
#[cfg(feature = "floats")]
mod float;
mod numeric;
mod option;
mod string;
mod vec;

pub use numeric::*;
pub use option::*;
pub use string::*;
pub use vec::*;

pub use self::bool::*;
#[cfg(feature = "floats")]
pub use self::float::*;

#[cfg(feature = "wincode")]
mod wincode;
