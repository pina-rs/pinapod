//! Alignment-one IEEE-754 storage for `f32` and `f64` schema fields.

use core::fmt;

use crate::PinaPodError;
use crate::traits::ZcElem;
use crate::traits::ZcField;
use crate::traits::ZcValidate;

/// Defines an alignment-one IEEE-754 pod over its little-endian bit pattern.
///
/// The pod is a byte container, not an arithmetic type: it preserves the bit
/// pattern exactly, including NaN payloads, infinities, and the sign of zero.
/// Every bit pattern is a valid stored value, so [`ZcValidate`] accepts any
/// bytes and an all-zero field decodes as `+0.0`.
macro_rules! define_pod_float {
    ($(#[$struct_doc:meta])* $name:ident, $native:ty, $bits:ty, $size:expr) => {
        $(#[$struct_doc])*
        #[repr(transparent)]
        #[derive(Copy, Clone, Default)]
        #[cfg_attr(
            feature = "wincode",
            derive(wincode::SchemaWrite, wincode::SchemaRead)
        )]
        pub struct $name([u8; $size]);

        impl $name {
            /// Zero (`+0.0`) encoded in little-endian form.
            pub const ZERO: Self = Self([0u8; $size]);

            /// The smallest positive normal bit pattern.
            pub const MIN_POSITIVE: Self = Self(<$native>::MIN_POSITIVE.to_bits().to_le_bytes());

            /// The largest finite bit pattern.
            pub const MAX: Self = Self(<$native>::MAX.to_bits().to_le_bytes());

            /// Creates a value from its little-endian byte representation.
            #[inline(always)]
            pub const fn new_from_array(array: [u8; $size]) -> Self {
                Self(array)
            }

            /// Decodes the stored little-endian bit pattern.
            #[inline(always)]
            pub fn get(&self) -> $native {
                <$native>::from_bits(<$bits>::from_le_bytes(self.0))
            }

            /// Replaces the stored value with the bit pattern of `value`.
            #[inline(always)]
            pub fn set(&mut self, value: $native) {
                self.0 = value.to_bits().to_le_bytes();
            }

            /// Returns `true` if the stored bit pattern is all zeros (`+0.0`).
            #[inline(always)]
            pub fn is_zero(&self) -> bool {
                self.0 == [0u8; $size]
            }

            /// The stored bit pattern as the backing little-endian integer.
            #[inline(always)]
            pub const fn to_bits(&self) -> $bits {
                <$bits>::from_le_bytes(self.0)
            }

            /// Replaces the stored value with a raw bit pattern.
            #[inline(always)]
            pub const fn set_bits(&mut self, bits: $bits) {
                self.0 = bits.to_le_bytes();
            }
        }

        impl From<$native> for $name {
            #[inline(always)]
            fn from(value: $native) -> Self {
                Self(value.to_bits().to_le_bytes())
            }
        }

        impl From<$name> for $native {
            #[inline(always)]
            fn from(value: $name) -> Self {
                value.get()
            }
        }

        impl PartialEq for $name {
            /// <!-- {=podFloatBitwiseEqualityContract|trim|linePrefix:"/// ":true|indent:"            "} -->
            /// Equality compares stored bit patterns rather than decoded floats.
            ///
            /// That keeps `Eq` sound in the presence of NaN payloads and preserves the distinction between `+0.0` and `-0.0`. The pods deliberately implement no `PartialOrd` or `Ord`, because bitwise equality and float ordering cannot both hold: an ordering would have to rank NaN payloads and separate `+0.0` from `-0.0`. Decode with `get` and compare the natives when an ordering is needed.<!-- {/podFloatBitwiseEqualityContract} -->
            #[inline(always)]
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }

        impl Eq for $name {}

        // Deliberately no `PartialOrd`/`Ord`: bitwise equality and float
        // ordering cannot both hold, because ordering would have to rank NaN
        // payloads and separate `+0.0` from `-0.0`. Decode with `get` and
        // compare the natives when an ordering is needed.

        impl core::hash::Hash for $name {
            fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
                self.0.hash(state);
            }
        }

        impl fmt::Binary for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Binary::fmt(&self.to_bits(), f)
            }
        }

        impl fmt::LowerHex for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::LowerHex::fmt(&self.to_bits(), f)
            }
        }

        impl fmt::UpperHex for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::UpperHex::fmt(&self.to_bits(), f)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.get().fmt(f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(&self.get(), f)
            }
        }

        impl AsRef<[u8]> for $name {
            #[inline(always)]
            fn as_ref(&self) -> &[u8] {
                &self.0
            }
        }

        // Every bit pattern of an IEEE-754 value is a valid float (including
        // NaN and the sign of zero), so stored bytes never need a validity
        // check beyond their length.
        impl ZcValidate for $name {
            #[inline(always)]
            fn validate_ref(_: &Self) -> Result<(), PinaPodError> {
                Ok(())
            }
        }

        // SAFETY: `$name` is `#[repr(transparent)]` over `[u8; $size]`, so it
        // is align 1, and reinterpreting any bit pattern as `$native` yields a
        // float value rather than undefined behavior.
        unsafe impl ZcElem for $name {}

        // SAFETY: `$name` is its own alignment-one pod; its size is derived
        // with `size_of::<Self::Pod>()` wherever it is used.
        unsafe impl ZcField for $name {
            type Pod = Self;
        }

        // SAFETY: `$name` is align 1 and every bit pattern is valid, so a
        // schema field of this type needs no validity metadata.
        unsafe impl ZcField for $native {
            type Pod = $name;
        }

        const _: () = assert!(core::mem::align_of::<$name>() == 1);
        const _: () = assert!(core::mem::size_of::<$name>() == $size);
        const _: () = assert!(core::mem::size_of::<$name>() == core::mem::size_of::<$native>());
    };
}

define_pod_float!(
    /// Alignment-one storage for a 32-bit IEEE-754 float schema field.
    ///
    /// `get` and `set` convert through the bit pattern, while `to_bits` and `set_bits`
    /// expose it directly. The pod is exactly four bytes wide.
    ///
    /// <!-- {=podFloatBitPatternContract|trim|linePrefix:"/// ":true|indent:"    "} -->
    /// Storage is the complete IEEE-754 bit pattern, little-endian.
    ///
    /// Every bit pattern is a valid stored value, so validation never rejects a NaN, an infinity, or the sign of zero, and an all-zero field decodes as `+0.0`.<!-- {/podFloatBitPatternContract} -->
    ///
    /// A schema field declared as `f32` maps to this pod through the [`ZcField`]
    /// implementation below, so `PinaPod` derives accept the native spelling.
    ///
    /// ```
    /// use pinapod::pod::PodF32;
    ///
    /// let mut pod = PodF32::ZERO;
    /// pod.set(-1.5);
    /// assert_eq!(pod.get(), -1.5);
    /// assert_eq!(pod.to_bits(), (-1.5_f32).to_bits());
    ///
    /// // Raw bit patterns survive a round trip, including a signaling NaN.
    /// pod.set_bits(0x7f80_0001);
    /// assert_eq!(pod.to_bits(), 0x7f80_0001);
    /// ```
    PodF32,
    f32,
    u32,
    4
);

define_pod_float!(
    /// Alignment-one storage for a 64-bit IEEE-754 float schema field.
    ///
    /// `get` and `set` convert through the bit pattern, while `to_bits` and `set_bits`
    /// expose it directly. The pod is exactly eight bytes wide.
    ///
    /// <!-- {=podFloatBitPatternContract|trim|linePrefix:"/// ":true|indent:"    "} -->
    /// Storage is the complete IEEE-754 bit pattern, little-endian.
    ///
    /// Every bit pattern is a valid stored value, so validation never rejects a NaN, an infinity, or the sign of zero, and an all-zero field decodes as `+0.0`.<!-- {/podFloatBitPatternContract} -->
    ///
    /// A schema field declared as `f64` maps to this pod through the [`ZcField`]
    /// implementation below, so `PinaPod` derives accept the native spelling.
    ///
    /// ```
    /// use pinapod::pod::PodF64;
    ///
    /// let mut pod = PodF64::ZERO;
    /// pod.set(3.125);
    /// assert_eq!(pod.get(), 3.125);
    /// assert_eq!(pod.to_bits(), 3.125_f64.to_bits());
    /// ```
    PodF64,
    f64,
    u64,
    8
);

#[cfg(all(kani, feature = "kani"))]
mod kani_proofs {
    use super::*;

    // These harnesses prove the property the pods actually audit: storage is a
    // lossless, alignment-one byte container for an arbitrary bit pattern.
    //
    // They are deliberately stated over the backing integer rather than over
    // float values. Kani models `f32`/`f64` as values with limited bit
    // precision, so a bits -> float -> bits round trip is not a sound proof
    // obligation at this point (`from_bits`/`to_bits` are standard-library
    // reinterpretations, not PinaPod code). Bit-pattern preservation is
    // covered exhaustively by the runtime and Miri suites, including NaN
    // payloads that no float-valued model can represent.
    macro_rules! prove_pod_float {
        ($pod:ident, $bits:ty, $module:ident) => {
            mod $module {
                use super::super::*;

                #[kani::proof]
                fn set_bits_then_read_preserves_the_pattern() {
                    let bits: $bits = kani::any();
                    let mut pod = $pod::ZERO;
                    pod.set_bits(bits);

                    assert!(pod.to_bits() == bits);
                }

                #[kani::proof]
                fn new_from_array_is_little_endian() {
                    let bytes: [u8; core::mem::size_of::<$pod>()] = kani::any();
                    let pod = $pod::new_from_array(bytes);

                    assert!(pod.to_bits() == <$bits>::from_le_bytes(bytes));
                    assert!(pod.as_ref() == &bytes);
                }

                #[kani::proof]
                fn zero_is_the_all_zero_pattern() {
                    assert!($pod::ZERO.is_zero());
                    assert!($pod::ZERO.to_bits() == 0);
                    assert!(!$pod::new_from_array([1; core::mem::size_of::<$pod>()]).is_zero());
                }
            }
        };
    }

    prove_pod_float!(PodF32, u32, f32_proofs);
    prove_pod_float!(PodF64, u64, f64_proofs);
}
