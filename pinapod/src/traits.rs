//! The representation contracts that a `PinaPod` schema implements.
//!
//! [`PinaPod`] marks a generated schema. [`PinaPodFixed`] and [`PinaPodCompact`] add the
//! read, validation, and update operations for the two layouts, and [`PinaPodPatch`]
//! carries one preflighted compact update.
//!
//! The remaining traits describe storage: [`ZcValidate`] checks that initialized bytes
//! hold a semantic value, [`ZcElem`] states the unsafe representation contract for a
//! stored type, and [`ZcField`] maps a native schema field to its stored pod.
//!
//! Implementing [`PinaPodFixed`], [`PinaPodCompact`], [`ZcElem`], or [`ZcField`] by hand
//! is unsafe, because each carries a contract that safe readers rely on. Prefer
//! `#[derive(PinaPod)]`.
//!
//! <!-- {=podMdtManagedDocNote|trim|linePrefix:"//! ":true} -->
//! This section is synchronized by `mdt` and expands from `api-docs.t.md`. Edit the provider, then run `devenv shell docs:sync`.<!-- {/podMdtManagedDocNote} -->

use crate::error::PinaPodError;
use crate::pod::*;

/// Validation trait for stored (pod) types.
/// Each pod type knows how to validate itself.
pub trait ZcValidate: Copy {
	/// Validate that this value's bytes represent a valid state.
	fn validate_ref(value: &Self) -> Result<(), PinaPodError>;

	/// Validate every element of a stored array of this type.
	///
	/// An array's stored validity is exactly its elements' validity, so this
	/// defaults to walking the array and calling
	/// [`validate_ref`](Self::validate_ref) on each item. That walk is
	/// load-bearing for any element with a restricted domain: it is what
	/// rejects a non-canonical `PodBool` byte, an out-of-range container prefix,
	/// or non-UTF-8 string bytes. An element type whose every initialized bit
	/// pattern is valid overrides this with a no-op and loses no coverage.
	///
	/// The override exists because a `[T; N]` validation loop that merely
	/// happens to be dead after inlining is not reliably removed at `-C
	/// opt-level=3` on SBF. Stating the trivial case as a separate
	/// implementation removes the loop from that instantiation outright, which
	/// keeps byte arrays on the same per-element cost as a hand-written
	/// `Ok(())`.
	#[inline(always)]
	fn validate_array<const N: usize>(value: &[Self; N]) -> Result<(), PinaPodError> {
		for item in value {
			Self::validate_ref(item)?;
		}

		Ok(())
	}

	/// Validate every element of a slice of this type.
	///
	/// A slice's stored validity is exactly its elements' validity, so this
	/// defaults to walking the slice and calling
	/// [`validate_ref`](Self::validate_ref) on each item. The same rationale as
	/// [`validate_array`](Self::validate_array) applies: the loop is
	/// load-bearing for a restricted-domain element and dead for a
	/// trivially-valid one, and SBF `-C opt-level=3` does not reliably remove
	/// the dead case. Stating the trivial case as a separate implementation
	/// removes the loop from that instantiation outright.
	///
	/// Generated code walks a stored tail through this method rather than
	/// spelling the loop at every emission site.
	#[inline(always)]
	fn validate_slice(value: &[Self]) -> Result<(), PinaPodError> {
		for item in value {
			Self::validate_ref(item)?;
		}

		Ok(())
	}
}

// --- ZcValidate: trivially valid types (all bit patterns valid) ---

impl ZcValidate for u8 {
	#[inline(always)]
	fn validate_ref(_: &Self) -> Result<(), PinaPodError> {
		Ok(())
	}

	#[inline(always)]
	fn validate_array<const N: usize>(_: &[Self; N]) -> Result<(), PinaPodError> {
		Ok(())
	}

	#[inline(always)]
	fn validate_slice(_: &[Self]) -> Result<(), PinaPodError> {
		Ok(())
	}
}

impl ZcValidate for i8 {
	#[inline(always)]
	fn validate_ref(_: &Self) -> Result<(), PinaPodError> {
		Ok(())
	}

	#[inline(always)]
	fn validate_array<const N: usize>(_: &[Self; N]) -> Result<(), PinaPodError> {
		Ok(())
	}

	#[inline(always)]
	fn validate_slice(_: &[Self]) -> Result<(), PinaPodError> {
		Ok(())
	}
}

macro_rules! impl_zc_validate_trivial {
    ($($ty:ty),*) => {
        $(
            impl ZcValidate for $ty {
                #[inline(always)]
                fn validate_ref(_: &Self) -> Result<(), PinaPodError> { Ok(()) }

                #[inline(always)]
                fn validate_array<const N: usize>(_: &[Self; N]) -> Result<(), PinaPodError> {
                    Ok(())
                }

                #[inline(always)]
                fn validate_slice(_: &[Self]) -> Result<(), PinaPodError> { Ok(()) }
            }
        )*
    };
}

impl_zc_validate_trivial!(
	PodU16, PodU32, PodU64, PodU128, PodI16, PodI32, PodI64, PodI128
);

// Arrays validate per element. The element type decides how: a trivially valid
// element answers with a no-op that contains no loop, while a restricted-domain
// element walks every item. See `ZcValidate::validate_array`.
impl<T: ZcValidate, const N: usize> ZcValidate for [T; N] {
	#[inline(always)]
	fn validate_ref(value: &Self) -> Result<(), PinaPodError> {
		<T as ZcValidate>::validate_array(value)
	}
}

// --- ZcValidate: PodBool (byte must be 0 or 1) ---

impl ZcValidate for PodBool {
	#[inline(always)]
	fn validate_ref(value: &Self) -> Result<(), PinaPodError> {
		// SAFETY: PodBool is #[repr(transparent)] over [u8; 1], alignment 1.
		// Dereferencing as *const u8 reads the single stored byte.
		let byte = unsafe { *(value as *const PodBool as *const u8) };
		if byte > 1 {
			Err(PinaPodError::InvalidBool)
		} else {
			Ok(())
		}
	}
}

// --- ZcValidate: PodString (len <= N, active bytes valid UTF-8) ---

impl<const N: usize, const PFX: usize> ZcValidate for PodString<N, PFX> {
	#[inline(always)]
	fn validate_ref(value: &Self) -> Result<(), PinaPodError> {
		let raw_len = value.try_decode_len()?;
		if raw_len > N {
			return Err(PinaPodError::InvalidLength);
		}
		// SAFETY: raw_len <= N, and data is a [MaybeUninit<u8>; N] array.
		// The bytes come from account data (initialized memory), not
		// MaybeUninit::uninit().
		let bytes =
			unsafe { core::slice::from_raw_parts(value.data.as_ptr() as *const u8, raw_len) };
		if core::str::from_utf8(bytes).is_err() {
			return Err(PinaPodError::InvalidUtf8);
		}
		Ok(())
	}
}

// --- ZcValidate: PodVec (len <= N) ---

impl<T: ZcElem, const N: usize, const PFX: usize> ZcValidate for PodVecRepr<T, N, PFX> {
	#[inline(always)]
	fn validate_ref(value: &Self) -> Result<(), PinaPodError> {
		if value.try_decode_len()? > N {
			return Err(PinaPodError::InvalidLength);
		}
		// Through `validate_slice` rather than a local loop, so a trivially
		// valid element removes the loop here too.
		T::validate_slice(value.as_slice())
	}
}

// --- ZcValidate: PodOption (tag 0 or 1, inner valid if Some) ---

impl<T: ZcElem, const PFX: usize> ZcValidate for PodOption<T, PFX> {
	#[inline(always)]
	fn validate_ref(value: &Self) -> Result<(), PinaPodError> {
		match value.raw_tag() {
			0 => Ok(()),
			1 => {
				// SAFETY: Tag validated as == 1 above, so the MaybeUninit value was
				// initialized by PodOption::some() or deserialization.
				let inner = unsafe { value.assume_init_ref() };
				T::validate_ref(inner)
			}
			_ => Err(PinaPodError::InvalidTag),
		}
	}
}

/// # Safety
///
/// Implementors MUST guarantee all five of the following. Any violation
/// makes the zero-copy pointer cast `&*(ptr as *const Self)` performed by
/// the deserialization path undefined behavior.
///
/// 1. **Alignment == 1.** `core::mem::align_of::<Self>() == 1`, so casts
///    from `*const u8` are well-defined at any byte offset.
///
/// 2. **No padding.** `Self` contains no padding bytes. Use
///    `#[repr(transparent)]` over a single wrapped type, or `#[repr(C)]` /
///    `#[repr(packed)]` composed only of `ZcElem` fields.
///
/// 3. **Validity invariant holds for every bit pattern.** It must be sound
///    to form `&Self` from any `size_of::<Self>()` bytes of *initialized*
///    memory, *before* `validate_ref` has been consulted. Forming an
///    invalid reference is UB under Rust's aliasing rules regardless of
///    whether the bytes are ever read. This rules out types whose validity
///    is bit-pattern-restricted at the Rust type level — e.g., bare `bool`,
///    `char`, `NonZero*`, or enums with fewer than `2^N` discriminants.
///    Wrappers that store `u8` / `[u8; N]` and only *interpret* bytes at
///    access time (like `PodBool`) are fine; a field of type `bool` is not.
///
/// 4. **`ZcValidate::validate_ref` is load-bearing.** It must reject every
///    bit pattern whose reading through a safe accessor could cause UB or
///    violate the type's documented invariants. For types where every
///    initialized bit pattern is a semantically valid value (Pod integers,
///    `[u8; N]`), `validate_ref` may trivially return `Ok(())`. For types
///    with a restricted domain (`PodBool`, enums with fewer than `2^N`
///    discriminants, length-prefix-bearing containers), `validate_ref` is
///    the sole gate and MUST NOT short-circuit.
///
///    A type that overrides [`ZcValidate::validate_array`] takes on the same
///    obligation for its array form. Overriding it with a no-op asserts that
///    the element's whole domain is valid, exactly as a trivial
///    `validate_ref` does, and an array of a restricted-domain type MUST
///    keep the default per-element walk.
///
/// 5. **The all-zero representation is safe to inspect while initializing.**
///    It does not need to be semantically valid, but safe accessors called on
///    it must not cause undefined behavior. [`PinaPodFixed::initialize`] uses
///    this state so callers can set enum fields whose valid discriminants do
///    not include zero before the completed value is validated.
pub unsafe trait ZcElem: Copy + ZcValidate {}

// SAFETY: u8 and i8 are single bytes, trivially align 1, all bit patterns
// valid.
unsafe impl ZcElem for u8 {}
unsafe impl ZcElem for i8 {}

// SAFETY: All Pod integer types are #[repr(transparent)] over [u8; N], align 1.
unsafe impl ZcElem for PodU16 {}
unsafe impl ZcElem for PodU32 {}
unsafe impl ZcElem for PodU64 {}
unsafe impl ZcElem for PodU128 {}
unsafe impl ZcElem for PodI16 {}
unsafe impl ZcElem for PodI32 {}
unsafe impl ZcElem for PodI64 {}
unsafe impl ZcElem for PodI128 {}

// SAFETY: PodBool is #[repr(transparent)] over [u8; 1], align 1.
unsafe impl ZcElem for PodBool {}

// SAFETY: `[T; N]` inherits alignment 1 from `T: ZcElem` —
// `align_of::<[T; N]>() == align_of::<T>() == 1` — and its size is exactly
// `N * size_of::<T>()`, so no padding bytes can exist between elements and
// every element boundary falls on a valid byte offset. Element bit validity
// composes: forming `&[T; N]` from any `N * size_of::<T>()` initialized bytes
// is sound before `validate_ref` runs, and validation recurses per element.
unsafe impl<T: ZcElem, const N: usize> ZcElem for [T; N] {}

// SAFETY: PodOption<T: ZcElem, PFX> is #[repr(C)] with tag: [u8; PFX] + MaybeUninit<T>.
// T: ZcElem guarantees T is align 1, so PodOption<T, PFX> is also align 1.
unsafe impl<T: ZcElem, const PFX: usize> ZcElem for PodOption<T, PFX> {}

// SAFETY: PodString<N, PFX> is #[repr(C)] over [u8; PFX] + [MaybeUninit<u8>; N],
// both align 1 with no padding. Every initialized bit pattern is a valid
// reference (raw bytes, no restricted-domain fields); validate_ref gates
// len <= N and UTF-8 of the active bytes.
unsafe impl<const N: usize, const PFX: usize> ZcElem for PodString<N, PFX> {}

// SAFETY: PodVecRepr<T: ZcElem, N, PFX> is #[repr(C)] over [u8; PFX] +
// [MaybeUninit<T>; N]. T: ZcElem guarantees T is align 1, so the struct is
// align 1 with no padding. Every initialized bit pattern is a valid reference
// (T's own validity holds for any bit pattern); validate_ref gates len <= N
// and recurses into each element.
unsafe impl<T: ZcElem, const N: usize, const PFX: usize> ZcElem for PodVecRepr<T, N, PFX> {}

// --- Feature-gated impls for external types ---

#[cfg(feature = "solana-address")]
mod solana_address_impls {
	use super::*;

	const _: () = assert!(core::mem::align_of::<solana_address::Address>() == 1);

	// SAFETY: solana_address::Address is #[repr(transparent)] over [u8; 32],
	// align 1, all bit patterns valid.
	impl ZcValidate for solana_address::Address {
		#[inline(always)]
		fn validate_ref(_: &Self) -> Result<(), PinaPodError> {
			Ok(())
		}

		#[inline(always)]
		fn validate_array<const N: usize>(_: &[Self; N]) -> Result<(), PinaPodError> {
			Ok(())
		}

		#[inline(always)]
		fn validate_slice(_: &[Self]) -> Result<(), PinaPodError> {
			Ok(())
		}
	}

	// SAFETY: Address is Copy, align 1, all bit patterns valid.
	unsafe impl ZcElem for solana_address::Address {}

	unsafe impl ZcField for solana_address::Address {
		type Pod = solana_address::Address;
	}
}

/// Describes the byte layout generated for a schema type.
///
/// `PinaPod` is the common public contract implemented by the derive. Fixed
/// and compact layouts provide their specialized operations through
/// [`PinaPodFixed`] and [`PinaPodCompact`].
///
/// Type names alone never grant a built-in representation. A caller-local
/// lookalike must provide the unsafe representation contract explicitly:
///
/// ```compile_fail
/// use pinapod::PinaPod;
///
/// #[allow(non_camel_case_types)]
/// struct i8(bool);
///
/// #[derive(PinaPod)]
/// struct Invalid {
///     value: i8,
/// }
/// ```
pub trait PinaPod: Sized {}

/// Zero-copy access for a schema whose complete representation has one size.
///
/// # Safety
///
/// Implementors must ensure `Zc` is the complete fixed representation of
/// `Self`. Its byte size and validation rules must not depend on runtime state.
/// Prefer `#[derive(PinaPod)]`; manual implementations are an advanced raw API.
pub unsafe trait PinaPodFixed: PinaPod {
	/// The complete fixed representation of this schema.
	///
	/// `size_of::<Zc>()` is the schema's size, so the size must not depend on runtime
	/// state. A derive generates a companion struct whose fields are the mapped pods in
	/// declaration order.
	type Zc: ZcElem;

	/// Read one fixed value and reject both truncated and trailing bytes.
	fn read_exact(data: &[u8]) -> Result<&Self::Zc, PinaPodError> {
		Self::validate_exact(data)?;

		// SAFETY: validate_exact proves the slice has exactly one complete
		// representation and ZcElem guarantees alignment one and bit validity.
		Ok(unsafe { &*data.as_ptr().cast::<Self::Zc>() })
	}

	/// Mutably read one fixed value and reject truncated and trailing bytes.
	fn read_exact_mut(data: &mut [u8]) -> Result<&mut Self::Zc, PinaPodError> {
		Self::validate_exact(data)?;

		// SAFETY: validate_exact proves the slice has exactly one complete
		// representation and ZcElem guarantees alignment one and bit validity.
		Ok(unsafe { &mut *data.as_mut_ptr().cast::<Self::Zc>() })
	}

	/// Read the first fixed value from a larger containing byte sequence.
	fn read_prefix(data: &[u8]) -> Result<&Self::Zc, PinaPodError> {
		Self::validate_prefix(data)?;

		// SAFETY: validate_prefix proves that the first representation-sized
		// prefix is valid. ZcElem guarantees alignment one.
		Ok(unsafe { &*data.as_ptr().cast::<Self::Zc>() })
	}

	/// Mutably read the first fixed value from a larger containing sequence.
	fn read_prefix_mut(data: &mut [u8]) -> Result<&mut Self::Zc, PinaPodError> {
		Self::validate_prefix(data)?;

		// SAFETY: validate_prefix proves that the first SIZE bytes contain a
		// valid representation. ZcElem guarantees alignment one.
		Ok(unsafe { &mut *data.as_mut_ptr().cast::<Self::Zc>() })
	}

	/// Validate one complete fixed value with no trailing bytes.
	fn validate_exact(data: &[u8]) -> Result<(), PinaPodError> {
		let size = core::mem::size_of::<Self::Zc>();

		if data.len() != size {
			if data.len() < size {
				return Err(PinaPodError::BufferTooSmall);
			}
			return Err(PinaPodError::InvalidLength);
		}

		// SAFETY: the length check proves a complete representation is present
		// and ZcElem permits forming a reference from initialized bytes before
		// semantic validation.
		let value = unsafe { &*data.as_ptr().cast::<Self::Zc>() };
		<Self::Zc as ZcValidate>::validate_ref(value)
	}

	/// Validate the first fixed value in a larger containing byte sequence.
	fn validate_prefix(data: &[u8]) -> Result<(), PinaPodError> {
		let size = core::mem::size_of::<Self::Zc>();

		if data.len() < size {
			return Err(PinaPodError::BufferTooSmall);
		}

		// SAFETY: the length check proves a complete representation is present
		// and ZcElem permits forming a reference from initialized bytes before
		// semantic validation.
		let value = unsafe { &*data.as_ptr().cast::<Self::Zc>() };
		<Self::Zc as ZcValidate>::validate_ref(value)
	}

	/// Initialize exactly one fixed value and validate it after configuration.
	///
	/// The destination is zeroed before `initialize` is called, so the closure
	/// can set fields such as enums whose valid discriminants exclude zero.
	/// Validation runs once, after the closure returns successfully.
	///
	/// If the closure or validation returns an error, the complete destination
	/// is zeroed again. This deterministic failure state is fully initialized,
	/// but it is not necessarily a semantically valid value for the schema.
	fn initialize(
		data: &mut [u8],
		initialize: impl FnOnce(&mut Self::Zc) -> Result<(), PinaPodError>,
	) -> Result<&mut Self::Zc, PinaPodError> {
		let size = core::mem::size_of::<Self::Zc>();

		if data.len() < size {
			return Err(PinaPodError::BufferTooSmall);
		}

		if data.len() != size {
			return Err(PinaPodError::InvalidLength);
		}

		data.fill(0);

		let pointer = data.as_mut_ptr().cast::<Self::Zc>();
		let result = {
			// SAFETY: the exact length is checked above, ZcElem has alignment
			// one, and its unsafe contract makes the all-zero initialization
			// state safe to inspect and mutate before semantic validation.
			let value = unsafe { &mut *pointer };

			initialize(value).and_then(|()| <Self::Zc as ZcValidate>::validate_ref(value))
		};

		if let Err(error) = result {
			data.fill(0);

			return Err(error);
		}

		// SAFETY: the closure completed and validate_ref accepted the same
		// representation. The mutable borrow of `data` remains exclusive.
		Ok(unsafe { &mut *pointer })
	}
}

/// Zero-copy access for compact schemas with a fixed header and dynamic tails.
///
/// # Safety
///
/// Implementors must ensure `Header`, `HEADER_SIZE`, and `validate` describe
/// the same representation. Dynamic length metadata must not be exposed for
/// direct mutable access.
pub unsafe trait PinaPodCompact: PinaPod {
	/// The fixed header that precedes the dynamic tails.
	///
	/// Its size must equal [`HEADER_SIZE`](Self::HEADER_SIZE).
	type Header: ZcElem;

	/// Smallest valid allocation for this compact schema.
	const MIN_SIZE: usize;

	/// Largest valid allocation for this compact schema.
	const MAX_SIZE: usize;

	/// Byte granularity of valid allocation growth beyond [`Self::MIN_SIZE`].
	const TAIL_ALIGNMENT: usize;

	/// Byte size of [`Header`](Self::Header).
	///
	/// Implementations must keep this equal to `size_of::<Self::Header>()`;
	/// [`validate`](Self::validate) relies on it to locate the first tail.
	const HEADER_SIZE: usize;

	/// Validate the physical allocation independently of its active contents.
	fn validate_storage_len(size: usize) -> Result<(), PinaPodError> {
		if Self::TAIL_ALIGNMENT == 0
			|| size < Self::MIN_SIZE
			|| size > Self::MAX_SIZE
			|| !(size - Self::MIN_SIZE).is_multiple_of(Self::TAIL_ALIGNMENT)
		{
			return Err(PinaPodError::InvalidLength);
		}

		Ok(())
	}

	/// Validate one complete compact representation.
	///
	/// Implementations must check the allocation through
	/// [`validate_storage_len`](Self::validate_storage_len) first, then walk the header
	/// and every tail: option tags, length prefixes, field capacities, offsets, and
	/// UTF-8. A caller may form references only after this returns `Ok`.
	///
	/// # Errors
	///
	/// Returns [`PinaPodError::InvalidLength`] for an allocation outside the schema's
	/// size bounds or tail granularity, and another [`PinaPodError`] variant when a
	/// stored value is not a valid representation.
	fn validate(data: &[u8]) -> Result<(), PinaPodError>;

	/// Validate only what a tail relocation needs: the allocation, the header's
	/// length prefixes and tags, and the chained tail bounds.
	///
	/// Relocating a tail reads exactly three things from the buffer —
	/// `data.len()`, the header size, and the stored length prefixes — then
	/// performs checked adds and moves bytes between the offsets they imply.
	/// Copying arbitrary bytes is safe, so the only hazard is an offset or end
	/// computed past the buffer. This method proves that hazard absent without
	/// the per-element semantic walk [`validate`](Self::validate) also performs:
	/// no element iteration, no UTF-8 check, and no header field validation
	/// beyond what the bounds require.
	///
	/// Use this where the bytes are about to be relocated but not interpreted.
	/// Every boundary that exposes or persists a *value* — a reader's
	/// constructor, an update's preflight, and initialization's post-commit
	/// check — must keep calling [`validate`](Self::validate).
	///
	/// The default falls back to [`validate`](Self::validate), so a hand-written
	/// implementation keeps the full check and cannot weaken itself by
	/// accident. Returns the same error variants as
	/// [`validate`](Self::validate) for the conditions it checks.
	///
	/// # Errors
	///
	/// Returns [`PinaPodError::InvalidLength`] for an allocation outside the
	/// schema's size bounds or tail granularity, a prefix above its field
	/// capacity, or an unreadable prefix width; [`PinaPodError::BufferTooSmall`]
	/// when a chained tail bound leaves the buffer; and
	/// [`PinaPodError::Overflow`] when an offset sum overflows.
	fn validate_layout(data: &[u8]) -> Result<(), PinaPodError> {
		Self::validate(data)
	}
}

/// Run the commit-entry check at the depth the current build selects.
///
/// Generated `commit` bodies call this rather than naming a depth directly,
/// because of how cargo features resolve. A `#[cfg(feature = "...")]` is
/// evaluated in the crate being compiled — the *consumer* of this crate — so a
/// cfg written into generated code would test the downstream crate's feature
/// namespace instead of this one. Where the name is undefined, the generated
/// code would silently take the disabled branch and the feature would mean
/// nothing. Keeping the dispatch here makes the feature mean what it says.
///
/// The default is [`PinaPodCompact::validate_layout`]. Relocation consumes
/// only the buffer length, the header size, and the stored prefixes, so layout
/// is the complete precondition for it, and this fails closed on a corrupt
/// prefix or a short buffer exactly as a full walk would. Enabling
/// `compact-commit-full-validation` widens the check to
/// [`PinaPodCompact::validate`]: one full semantic walk per commit, which is
/// what 0.4.2 did. That is a strictly stronger commit, not a weaker one, so
/// the feature can only add coverage.
///
/// # Errors
///
/// Returns whatever the selected depth returns; both reject a corrupt prefix,
/// an allocation outside the schema's bounds, and a tail bound that leaves the
/// buffer.
#[doc(hidden)]
#[inline(always)]
pub fn commit_entry_validate<T: PinaPodCompact>(data: &[u8]) -> Result<(), PinaPodError> {
	#[cfg(feature = "compact-commit-full-validation")]
	{
		T::validate(data)
	}

	#[cfg(not(feature = "compact-commit-full-validation"))]
	{
		T::validate_layout(data)
	}
}

/// An atomic, preflighted update for one compact schema.
///
/// Patches borrow semantic input values but never expose raw offsets, length
/// prefixes, or partially committed mutation state. Frameworks can use this
/// trait to plan a resize, release the old borrow, and apply the same patch to
/// the resized allocation.
pub trait PinaPodPatch<T: PinaPodCompact> {
	/// The allocation size [`update`](Self::update) would produce, without changing `data`.
	///
	/// Frameworks call this to plan a resize, release the old borrow, and then apply the
	/// same patch to the resized allocation.
	///
	/// # Errors
	///
	/// Returns [`PinaPodError`] when the supplied values cannot be encoded or `data` is
	/// not a valid existing representation.
	fn updated_len(&self, data: &[u8]) -> Result<usize, PinaPodError>;
	/// Applies the patch to an existing representation and returns the new length.
	///
	/// Capacity, arithmetic, and supplied-value checks all run before the first byte
	/// changes, so a rejected patch leaves `data` untouched.
	///
	/// # Errors
	///
	/// Returns [`PinaPodError::BufferTooSmall`] when the resized value does not fit
	/// `data`, and another [`PinaPodError`] variant when `data` is not a valid existing
	/// representation or a supplied value cannot be encoded. A derived patch also
	/// returns [`PinaPodError::InvalidLength`] if its preflighted length and its
	/// committed length ever disagree, which can only indicate a defect in the
	/// generated code rather than an input condition.
	fn update(&self, data: &mut [u8]) -> Result<usize, PinaPodError>;
	/// Writes the patch into a destination without reading a previous representation.
	///
	/// Use this for a fresh allocation or for a destination left zeroed by a failed
	/// update. The destination is zeroed before configuration and validated after it.
	///
	/// # Errors
	///
	/// Returns [`PinaPodError`] when the allocation size is invalid, the encoded value
	/// does not fit `data`, or a supplied value cannot be encoded. A failure leaves the
	/// destination zeroed rather than partially patched.
	fn initialize(&self, data: &mut [u8]) -> Result<usize, PinaPodError>;
}

impl<T, P> PinaPodPatch<T> for &P
where
	T: PinaPodCompact,
	P: PinaPodPatch<T> + ?Sized,
{
	fn updated_len(&self, data: &[u8]) -> Result<usize, PinaPodError> {
		<P as PinaPodPatch<T>>::updated_len(*self, data)
	}

	fn update(&self, data: &mut [u8]) -> Result<usize, PinaPodError> {
		<P as PinaPodPatch<T>>::update(*self, data)
	}

	fn initialize(&self, data: &mut [u8]) -> Result<usize, PinaPodError> {
		<P as PinaPodPatch<T>>::initialize(*self, data)
	}
}

/// Maps a native Rust type to its pod (zero-copy) companion.
///
/// # Safety
///
/// The associated pod type carries the alignment, padding, bit-validity, and
/// validation requirements through [`ZcElem`]. Its size is always derived with
/// `size_of::<Self::Pod>()`; implementors cannot provide conflicting metadata.
pub unsafe trait ZcField: Sized {
	/// The alignment-one pod that stores this type in a schema.
	///
	/// A field declared as `Self` is stored as `Pod`. The mapping must not depend on
	/// runtime state, because a representation's layout is fixed at compile time and its
	/// size is always derived from `size_of::<Self::Pod>()`.
	type Pod: ZcElem;
}

/// Converts a compact patch argument for a native [`Option<T>`] field into
/// its stored representation.
///
/// <!-- {=podDeriveSupportTraitContract|trim|linePrefix:"/// ":true} -->
/// This trait is public only because generated code expands in downstream crates.
///
/// It is not part of the hand-written PinaPod API. Its shape follows the generated output and changes only in breaking releases, in lockstep with the derive.<!-- {/podDeriveSupportTraitContract} -->
#[doc(hidden)]
pub trait IntoPodOption<T: ZcField> {
	/// Converts `Option<T>` or an already-stored [`PodOption`] into the stored form.
	fn into_pod_option(self) -> PodOption<T::Pod>;
}

impl<T> IntoPodOption<T> for Option<T>
where
	T: ZcField,
	T::Pod: From<T>,
{
	#[inline(always)]
	fn into_pod_option(self) -> PodOption<T::Pod> {
		match self {
			Some(value) => PodOption::some(value.into()),
			None => PodOption::none(),
		}
	}
}

impl<T> IntoPodOption<T> for PodOption<T::Pod>
where
	T: ZcField,
{
	#[inline(always)]
	fn into_pod_option(self) -> PodOption<T::Pod> {
		self
	}
}

// Built-in ZcField impls
macro_rules! impl_zc_field {
	($native:ty, $pod:ty) => {
		unsafe impl ZcField for $native {
			type Pod = $pod;
		}
	};
}

impl_zc_field!(u8, u8);
impl_zc_field!(u16, PodU16);
impl_zc_field!(u32, PodU32);
impl_zc_field!(u64, PodU64);
impl_zc_field!(u128, PodU128);
impl_zc_field!(i8, i8);
impl_zc_field!(i16, PodI16);
impl_zc_field!(i32, PodI32);
impl_zc_field!(i64, PodI64);
impl_zc_field!(i128, PodI128);
impl_zc_field!(bool, PodBool);

#[cfg(feature = "fixed")]
mod fixed_impls {
	use super::*;

	macro_rules! impl_fixed_zc_field {
		($fixed:ident, $pod:ty) => {
			// SAFETY: `fixed::$fixed<Frac>` is a schema type whose complete
			// bit pattern is stored in the matching little-endian integer pod.
			// The pod type is an alignment-one `ZcElem`; its size is derived
			// directly wherever it is used.
			unsafe impl<Frac> ZcField for fixed::$fixed<Frac> {
				type Pod = $pod;
			}
		};
	}

	impl_fixed_zc_field!(FixedI8, i8);
	impl_fixed_zc_field!(FixedI16, PodI16);
	impl_fixed_zc_field!(FixedI32, PodI32);
	impl_fixed_zc_field!(FixedI64, PodI64);
	impl_fixed_zc_field!(FixedI128, PodI128);
	impl_fixed_zc_field!(FixedU8, u8);
	impl_fixed_zc_field!(FixedU16, PodU16);
	impl_fixed_zc_field!(FixedU32, PodU32);
	impl_fixed_zc_field!(FixedU64, PodU64);
	impl_fixed_zc_field!(FixedU128, PodU128);
}

// SAFETY: The pod of an array is the array of its element pods. `[T::Pod; N]`
// is an alignment-one `ZcElem` whose validation recurses into `T`'s, so the
// representation contract carries through unchanged. `[u8; N]` keeps its
// identity mapping via `<u8 as ZcField>::Pod = u8`.
unsafe impl<T: ZcField, const N: usize> ZcField for [T; N] {
	type Pod = [<T as ZcField>::Pod; N];
}

/// Converts a native or pod-spelled array into its stored representation,
/// element-wise.
///
/// <!-- {=podDeriveSupportTraitContract|trim|linePrefix:"/// ":true} -->
/// This trait is public only because generated code expands in downstream crates.
///
/// It is not part of the hand-written PinaPod API. Its shape follows the generated output and changes only in breaking releases, in lockstep with the derive.<!-- {/podDeriveSupportTraitContract} -->
///
/// The blanket impl covers both spellings: `[u64; N]` resolves through
/// `PodU64: From<u64>`, while `[PodU64; N]` resolves through the reflexive
/// `From`.
#[doc(hidden)]
pub trait IntoPodArray<T: ZcField, const N: usize> {
	/// Converts each element through its [`ZcField`] mapping.
	fn into_pod_array(self) -> [<T as ZcField>::Pod; N];
}

impl<T, U, const N: usize> IntoPodArray<T, N> for [U; N]
where
	T: ZcField,
	T::Pod: From<U>,
{
	#[inline(always)]
	fn into_pod_array(self) -> [T::Pod; N] {
		self.map(<T::Pod as From<U>>::from)
	}
}

macro_rules! impl_zc_field_identity {
    ($($ty:ty),*) => {
        $(
            unsafe impl ZcField for $ty {
                type Pod = Self;
            }
        )*
    };
}

impl_zc_field_identity!(
	PodU16, PodU32, PodU64, PodU128, PodI16, PodI32, PodI64, PodI128, PodBool
);

unsafe impl<const N: usize, const PFX: usize> ZcField for PodString<N, PFX> {
	type Pod = Self;
}

unsafe impl<T: ZcElem, const N: usize, const PFX: usize> ZcField for PodVecRepr<T, N, PFX> {
	type Pod = Self;
}

unsafe impl<T: ZcElem, const PFX: usize> ZcField for PodOption<T, PFX> {
	type Pod = Self;
}

// Option<T> maps to PodOption<T::Pod, 1> (PFX=1 only, unchanged).
unsafe impl<T> ZcField for Option<T>
where
	T: ZcField,
{
	type Pod = PodOption<T::Pod, 1>;
}
