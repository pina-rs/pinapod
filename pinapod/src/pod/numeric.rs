//! Alignment-one integer storage for zero-copy account access.

use core::fmt;

macro_rules! define_pod_integer {
    ($name:ident, $native:ty, $size:expr) => {
        #[doc = concat!("Alignment-one storage for a schema field declared as `", stringify!($native), "`.")]
        #[doc = ""]
        #[doc = concat!("The stored value is the little-endian bit pattern in ", stringify!($size), " bytes, so `", stringify!($name), "` is `#[repr(transparent)]` over `[u8; ", stringify!($size), "]` and can be read at any byte offset. Decode with [`get`](Self::get), encode with [`set`](Self::set), and pick an overflow contract per call with the `checked_*`, `wrapping_*`, and `saturating_*` methods.")]
        #[doc = ""]
        #[doc = concat!("A schema field declared as `", stringify!($native), "` maps to this pod through the [`ZcField`](crate::ZcField) implementation, so `PinaPod` derives accept the native spelling. The methods mirror the native integer arithmetic, but every operation returns a pod value and the arithmetic itself never panics.")]
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

            /// Decodes the stored value with a volatile read.
            ///
            /// A CPI callee can write the account this value lives in, and a
            /// plain [`get`](Self::get) lets the compiler reuse a load from before
            /// the call, because nothing in the caller's source proves the bytes
            /// changed. A volatile read forces the bytes to be re-read from
            /// memory, so a balance or counter updated by a callee is observed
            /// after the call returns.
            ///
            /// This is the account-data analogue of reading an `AtomicU64` with
            /// `Relaxed` ordering: it prevents compiler caching, and on the
            /// single-threaded BPF target that is the whole hazard. It is not an
            /// atomic operation and provides no synchronization.
            ///
            /// The read makes no claim about reference validity: a pod
            /// reference formed before the CPI is not made sound by reading it
            /// volatile afterwards. Release the account borrow before the call,
            /// then form a fresh view over the returned bytes — this accessor
            /// answers whether the compiler re-loads those bytes, not whether
            /// an older reference may still be used.
            #[inline(always)]
            pub fn get_volatile(&self) -> $native {
                // SAFETY: `self.0` is an initialized `[u8; $size]` with
                // alignment one, and reading it by value through a volatile
                // load cannot create aliasing or validity violations.
                let bytes =
                    unsafe { core::ptr::read_volatile(self.0.as_ptr() as *const [u8; $size]) };

                <$native>::from_le_bytes(bytes)
            }

            /// Replaces the stored value with a volatile write.
            ///
            /// The store cannot be sunk below a later call or hoisted above an
            /// earlier one, so a value written before a CPI is in memory when
            /// the call executes. Volatile carries no atomicity guarantee: a
            /// pod wider than the target's store width may lower to several
            /// stores, so an observer must not rely on the write being
            /// indivisible.
            #[inline(always)]
            pub fn set_volatile(&mut self, value: $native) {
                // SAFETY: `self.0` is a uniquely borrowed `[u8; $size]` with
                // alignment one; a volatile store of a fully initialized byte
                // array leaves every bit pattern valid.
                unsafe {
                    core::ptr::write_volatile(
                        self.0.as_mut_ptr() as *mut [u8; $size],
                        value.to_le_bytes(),
                    );
                }
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

				// Addition and subtraction share one harness; they are cheap
				// relative to multiplication and division, whose solver cost
				// grows far faster than the number of assertions. Splitting the
				// expensive operations into their own harnesses keeps each
				// verification condition small, so the wide integer proofs stay
				// solvable instead of accumulating one large formula.
				#[kani::proof]
				#[kani::solver(cvc5)]
				fn checked_add_sub_matches_native() {
					let left: $native = kani::any();
					let right: $native = kani::any();
					let pod = $pod::from(left);

					assert!(
						pod.checked_add(right).map(|value| value.get()) == left.checked_add(right)
					);
					assert!(
						pod.checked_sub(right).map(|value| value.get()) == left.checked_sub(right)
					);
				}

				#[kani::proof]
				#[kani::solver(cvc5)]
				fn checked_mul_matches_native() {
					let left: $native = kani::any();
					let right: $native = kani::any();
					let pod = $pod::from(left);

					assert!(
						pod.checked_mul(right).map(|value| value.get()) == left.checked_mul(right)
					);
				}

				#[kani::proof]
				#[kani::solver(cvc5)]
				fn checked_div_matches_native() {
					let left: $native = kani::any();
					let right: $native = kani::any();
					let pod = $pod::from(left);

					assert!(
						pod.checked_div(right).map(|value| value.get()) == left.checked_div(right)
					);
				}

				#[kani::proof]
				#[kani::solver(cvc5)]
				fn wrapping_add_sub_matches_native() {
					let left: $native = kani::any();
					let right: $native = kani::any();
					let pod = $pod::from(left);

					assert!(pod.wrapping_add(right).get() == left.wrapping_add(right));
					assert!(pod.wrapping_sub(right).get() == left.wrapping_sub(right));
				}

				#[kani::proof]
				#[kani::solver(cvc5)]
				fn wrapping_mul_matches_native() {
					let left: $native = kani::any();
					let right: $native = kani::any();
					let pod = $pod::from(left);

					assert!(pod.wrapping_mul(right).get() == left.wrapping_mul(right));
				}

				#[kani::proof]
				#[kani::solver(cvc5)]
				fn saturating_add_sub_matches_native() {
					let left: $native = kani::any();
					let right: $native = kani::any();
					let pod = $pod::from(left);

					assert!(pod.saturating_add(right).get() == left.saturating_add(right));
					assert!(pod.saturating_sub(right).get() == left.saturating_sub(right));
				}

				#[kani::proof]
				#[kani::solver(cvc5)]
				fn saturating_mul_matches_native() {
					let left: $native = kani::any();
					let right: $native = kani::any();
					let pod = $pod::from(left);

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
