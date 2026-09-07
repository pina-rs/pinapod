//! Alignment-one integer storage for zero-copy account access.

use core::fmt;

macro_rules! define_pod_integer {
    ($name:ident, $native:ty, $size:expr) => {
        #[repr(transparent)]
        #[derive(Copy, Clone, Default)]
        #[cfg_attr(feature = "wincode", derive(wincode::SchemaWrite, wincode::SchemaRead))]
        pub struct $name([u8; $size]);

        impl $name {
            /// Zero encoded in little-endian form.
            pub const ZERO: Self = Self([0u8; $size]);

            /// The largest value representable by the native integer.
            pub const MAX: Self = Self(<$native>::MAX.to_le_bytes());

            /// The smallest value representable by the native integer.
            pub const MIN: Self = Self(<$native>::MIN.to_le_bytes());

            /// Creates a value from its little-endian byte representation.
            #[inline(always)]
            pub const fn new_from_array(array: [u8; $size]) -> Self {
                Self(array)
            }

            /// Decodes the stored little-endian value.
            #[inline(always)]
            pub fn get(&self) -> $native {
                <$native>::from_le_bytes(self.0)
            }

            /// Replaces the stored value.
            #[inline(always)]
            pub fn set(&mut self, value: $native) {
                self.0 = value.to_le_bytes();
            }

            /// Returns `true` if the stored value is zero.
            #[inline(always)]
            pub fn is_zero(&self) -> bool {
                self.0 == [0u8; $size]
            }

            /// Adds two values, returning `None` on overflow.
            #[must_use]
            #[inline(always)]
            pub fn checked_add(self, rhs: impl Into<Self>) -> Option<Self> {
                self.get().checked_add(rhs.into().get()).map(Self::from)
            }

            /// Subtracts two values, returning `None` on overflow or underflow.
            #[must_use]
            #[inline(always)]
            pub fn checked_sub(self, rhs: impl Into<Self>) -> Option<Self> {
                self.get().checked_sub(rhs.into().get()).map(Self::from)
            }

            /// Multiplies two values, returning `None` on overflow.
            #[must_use]
            #[inline(always)]
            pub fn checked_mul(self, rhs: impl Into<Self>) -> Option<Self> {
                self.get().checked_mul(rhs.into().get()).map(Self::from)
            }

            /// Divides two values, returning `None` for an invalid result.
            ///
            /// Division by zero and signed division overflow both return `None`.
            #[must_use]
            #[inline(always)]
            pub fn checked_div(self, rhs: impl Into<Self>) -> Option<Self> {
                self.get().checked_div(rhs.into().get()).map(Self::from)
            }

            /// Adds two values with modular arithmetic.
            #[must_use]
            #[inline(always)]
            pub fn wrapping_add(self, rhs: impl Into<Self>) -> Self {
                Self::from(self.get().wrapping_add(rhs.into().get()))
            }

            /// Subtracts two values with modular arithmetic.
            #[must_use]
            #[inline(always)]
            pub fn wrapping_sub(self, rhs: impl Into<Self>) -> Self {
                Self::from(self.get().wrapping_sub(rhs.into().get()))
            }

            /// Multiplies two values with modular arithmetic.
            #[must_use]
            #[inline(always)]
            pub fn wrapping_mul(self, rhs: impl Into<Self>) -> Self {
                Self::from(self.get().wrapping_mul(rhs.into().get()))
            }

            /// Adds two values and clamps the result to the numeric bounds.
            #[must_use]
            #[inline(always)]
            pub fn saturating_add(self, rhs: impl Into<Self>) -> Self {
                Self::from(self.get().saturating_add(rhs.into().get()))
            }

            /// Subtracts two values and clamps the result to the numeric bounds.
            #[must_use]
            #[inline(always)]
            pub fn saturating_sub(self, rhs: impl Into<Self>) -> Self {
                Self::from(self.get().saturating_sub(rhs.into().get()))
            }

            /// Multiplies two values and clamps the result to the numeric bounds.
            #[must_use]
            #[inline(always)]
            pub fn saturating_mul(self, rhs: impl Into<Self>) -> Self {
                Self::from(self.get().saturating_mul(rhs.into().get()))
            }
        }

        impl From<$native> for $name {
            #[inline(always)]
            fn from(value: $native) -> Self {
                Self(value.to_le_bytes())
            }
        }

        impl From<$name> for $native {
            #[inline(always)]
            fn from(value: $name) -> Self {
                value.get()
            }
        }

        impl PartialEq for $name {
            #[inline(always)]
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }

        impl Eq for $name {}

        impl PartialEq<$native> for $name {
            #[inline(always)]
            fn eq(&self, other: &$native) -> bool {
                self.get() == *other
            }
        }

        impl PartialOrd for $name {
            #[inline(always)]
            fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for $name {
            #[inline(always)]
            fn cmp(&self, other: &Self) -> core::cmp::Ordering {
                self.get().cmp(&other.get())
            }
        }

        impl PartialOrd<$native> for $name {
            #[inline(always)]
            fn partial_cmp(&self, other: &$native) -> Option<core::cmp::Ordering> {
                self.get().partial_cmp(other)
            }
        }

        impl core::hash::Hash for $name {
            fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
                self.get().hash(state);
            }
        }

        impl fmt::Binary for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Binary::fmt(&self.get(), f)
            }
        }

        impl fmt::LowerHex for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::LowerHex::fmt(&self.get(), f)
            }
        }

        impl fmt::UpperHex for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::UpperHex::fmt(&self.get(), f)
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
    };
}

macro_rules! define_pod_signed {
    ($name:ident, $native:ty, $size:expr) => {
        define_pod_integer!($name, $native, $size);

        impl $name {
            /// Negates the value, returning `None` if the result is not representable.
            #[must_use]
            #[inline(always)]
            pub fn checked_neg(self) -> Option<Self> {
                self.get().checked_neg().map(Self::from)
            }

            /// Negates the value with modular arithmetic.
            #[must_use]
            #[inline(always)]
            pub fn wrapping_neg(self) -> Self {
                Self::from(self.get().wrapping_neg())
            }
        }
    };
}

define_pod_integer!(PodU128, u128, 16);
define_pod_integer!(PodU64, u64, 8);
define_pod_integer!(PodU32, u32, 4);
define_pod_integer!(PodU16, u16, 2);
define_pod_signed!(PodI128, i128, 16);
define_pod_signed!(PodI64, i64, 8);
define_pod_signed!(PodI32, i32, 4);
define_pod_signed!(PodI16, i16, 2);

macro_rules! assert_pod_layout {
    ($name:ident, $size:expr) => {
        const _: () = assert!(core::mem::align_of::<$name>() == 1);
        const _: () = assert!(core::mem::size_of::<$name>() == $size);
    };
}

assert_pod_layout!(PodU128, 16);
assert_pod_layout!(PodU64, 8);
assert_pod_layout!(PodU32, 4);
assert_pod_layout!(PodU16, 2);
assert_pod_layout!(PodI128, 16);
assert_pod_layout!(PodI64, 8);
assert_pod_layout!(PodI32, 4);
assert_pod_layout!(PodI16, 2);

#[cfg(all(kani, feature = "kani"))]
mod kani_proofs {
    macro_rules! prove_pod_integer {
        ($pod:ident, $native:ty, $module:ident) => {
            mod $module {
                use super::super::*;

                #[kani::proof]
                fn roundtrip() {
                    let value: $native = kani::any();
                    let pod = $pod::from(value);

                    assert!(pod.get() == value);
                    assert!(<$native>::from(pod) == value);
                }

                #[kani::proof]
                fn ordering_matches_native() {
                    let left: $native = kani::any();
                    let right: $native = kani::any();
                    let pod_left = $pod::from(left);
                    let pod_right = $pod::from(right);

                    assert!(pod_left.cmp(&pod_right) == left.cmp(&right));
                    assert!((pod_left == right) == (left == right));
                }

                #[kani::proof]
                fn zero_matches_native() {
                    let value: $native = kani::any();

                    assert!($pod::from(value).is_zero() == (value == 0));
                }

                #[kani::proof]
                #[kani::solver(z3)]
                fn checked_arithmetic_matches_native() {
                    let left: $native = kani::any();
                    let right: $native = kani::any();
                    let pod = $pod::from(left);

                    assert!(
                        pod.checked_add(right).map(|value| value.get()) == left.checked_add(right)
                    );
                    assert!(
                        pod.checked_sub(right).map(|value| value.get()) == left.checked_sub(right)
                    );
                    assert!(
                        pod.checked_mul(right).map(|value| value.get()) == left.checked_mul(right)
                    );
                    assert!(
                        pod.checked_div(right).map(|value| value.get()) == left.checked_div(right)
                    );
                }

                #[kani::proof]
                #[kani::solver(z3)]
                fn wrapping_arithmetic_matches_native() {
                    let left: $native = kani::any();
                    let right: $native = kani::any();
                    let pod = $pod::from(left);

                    assert!(pod.wrapping_add(right).get() == left.wrapping_add(right));
                    assert!(pod.wrapping_sub(right).get() == left.wrapping_sub(right));
                    assert!(pod.wrapping_mul(right).get() == left.wrapping_mul(right));
                }

                #[kani::proof]
                #[kani::solver(z3)]
                fn saturating_arithmetic_matches_native() {
                    let left: $native = kani::any();
                    let right: $native = kani::any();
                    let pod = $pod::from(left);

                    assert!(pod.saturating_add(right).get() == left.saturating_add(right));
                    assert!(pod.saturating_sub(right).get() == left.saturating_sub(right));
                    assert!(pod.saturating_mul(right).get() == left.saturating_mul(right));
                }
            }
        };
    }

    macro_rules! prove_pod_signed {
        ($pod:ident, $native:ty, $module:ident) => {
            mod $module {
                use super::super::*;

                #[kani::proof]
                fn explicit_negation_matches_native() {
                    let value: $native = kani::any();
                    let pod = $pod::from(value);

                    assert!(pod.checked_neg().map(|value| value.get()) == value.checked_neg());
                    assert!(pod.wrapping_neg().get() == value.wrapping_neg());
                }
            }
        };
    }

    prove_pod_integer!(PodU16, u16, u16_proofs);
    prove_pod_integer!(PodU32, u32, u32_proofs);
    prove_pod_integer!(PodU64, u64, u64_proofs);
    prove_pod_integer!(PodU128, u128, u128_proofs);
    prove_pod_integer!(PodI16, i16, i16_proofs);
    prove_pod_integer!(PodI32, i32, i32_proofs);
    prove_pod_integer!(PodI64, i64, i64_proofs);
    prove_pod_integer!(PodI128, i128, i128_proofs);

    prove_pod_signed!(PodI16, i16, signed_i16_proofs);
    prove_pod_signed!(PodI32, i32, signed_i32_proofs);
    prove_pod_signed!(PodI64, i64, signed_i64_proofs);
    prove_pod_signed!(PodI128, i128, signed_i128_proofs);
}
