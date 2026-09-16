use core::mem::MaybeUninit;

use super::string::max_n_for_pfx;
use crate::error::PinaPodError;
use crate::traits::ZcElem;
use crate::traits::ZcField;

/// Fixed-capacity vector storage using the `PinaPod` representation of `T`.
///
/// `PodVec<u64, 8, 2>` stores eight [`crate::pod::PodU64`] slots, while
/// `PodVec<PodU64, 8, 2>` remains valid through the identity [`ZcField`]
/// mapping. The second form exists for source compatibility; schema code
/// should normally use the native element type.
///
/// The representation is a `PFX`-byte little-endian element count followed by
/// `N` fixed-size element slots, so `PodVec<T, N, PFX>` occupies
/// `PFX + N * size_of::<T::Pod>()` bytes with alignment one.
///
/// <!-- {=podPrefixWidthRule|trim|linePrefix:"/// ":true} -->
/// `PFX` is the width in bytes of the length prefix or tag that precedes the payload, and it must be `1`, `2`, `4`, or `8`.<!-- {/podPrefixWidthRule} -->
///
/// <!-- {=podVecCapacityRule|trim|linePrefix:"/// ":true} -->
/// The element count must fit that prefix: `Vec<u64, 255>` is valid, `Vec<u64, 256>` is not, and `PodVec<u64, 256, 2>` restores it.<!-- {/podVecCapacityRule} -->
///
/// The default prefix is two bytes, so the [`Vec`](crate::Vec) alias is
/// `PodVec<T, N, 2>`.
pub type PodVec<T, const N: usize, const PFX: usize = 2> = PodVecRepr<<T as ZcField>::Pod, N, PFX>;

/// Raw fixed-capacity vector representation.
///
/// This backing type is public because it appears through [`PodVec`], but it
/// is not the schema-facing API. Prefer [`PodVec`] so native element types map
/// to their alignment-one PinaPod representation.
///
/// The representation is a `PFX`-byte little-endian element count followed by
/// `N` element slots of type `T`, so `PodVecRepr<T, N, PFX>` occupies
/// `PFX + N * size_of::<T>()` bytes with alignment one.
///
/// <!-- {=podPrefixWidthRule|trim|linePrefix:"/// ":true} -->
/// `PFX` is the width in bytes of the length prefix or tag that precedes the payload, and it must be `1`, `2`, `4`, or `8`.<!-- {/podPrefixWidthRule} -->
///
/// <!-- {=podVecCapacityRule|trim|linePrefix:"/// ":true} -->
/// The element count must fit that prefix: `Vec<u64, 255>` is valid, `Vec<u64, 256>` is not, and `PodVec<u64, 256, 2>` restores it.<!-- {/podVecCapacityRule} -->
///
/// # Stability
///
/// `PodVecRepr` is part of the wire format and the generated-code contract:
/// derives expand it into schema companions, and its field order is the stored
/// layout. Its public surface changes only in breaking releases, in lockstep
/// with [`PodVec`](self::PodVec).
#[doc(hidden)]
#[repr(C)]
#[derive(Copy, Clone)]
pub struct PodVecRepr<T: ZcElem, const N: usize, const PFX: usize = 2> {
    len: [u8; PFX],
    data: [MaybeUninit<T>; N],
}

// Compile-time layout invariants — PFX=2 (default, backward-compat).
const _: () = assert!(core::mem::size_of::<PodVecRepr<u8, 10>>() == 2 + 10);
const _: () = assert!(core::mem::align_of::<PodVecRepr<u8, 10>>() == 1);
const _: () = assert!(core::mem::size_of::<PodVecRepr<[u8; 32], 10>>() == 2 + 320);
const _: () = assert!(core::mem::align_of::<PodVecRepr<[u8; 32], 10>>() == 1);
// Compile-time layout invariants — PFX=1.
const _: () = assert!(core::mem::size_of::<PodVecRepr<u8, 10, 1>>() == 1 + 10);
const _: () = assert!(core::mem::align_of::<PodVecRepr<u8, 10, 1>>() == 1);
// Compile-time layout invariants — PFX=4.
const _: () = assert!(core::mem::size_of::<PodVecRepr<u8, 10, 4>>() == 4 + 10);
const _: () = assert!(core::mem::align_of::<PodVecRepr<u8, 10, 4>>() == 1);

impl<T: ZcElem, const N: usize, const PFX: usize> PodVecRepr<T, N, PFX> {
    /// Compile-time assertion that this capacity and prefix are representable.
    ///
    /// The associated constant is the only way to name the check: referring to
    /// `PodVec::<T, N, PFX>::VALID` forces the compiler to evaluate it, which rejects an
    /// unsupported prefix width or a capacity that does not fit the prefix. Generated
    /// code references it so a bad schema fails at the declaration site.
    pub const VALID: () = Self::_CAP_CHECK;
    const _CAP_CHECK: () = {
        assert!(
            PFX == 1 || PFX == 2 || PFX == 4 || PFX == 8,
            "PodVec<T, N, PFX>: PFX must be 1, 2, 4, or 8"
        );
        assert!(
            N <= max_n_for_pfx(PFX),
            "PodVec<T, N, PFX>: N exceeds the maximum value representable by the PFX-byte length \
             prefix"
        );
    };

    #[inline(always)]
    pub(crate) fn try_decode_len(&self) -> Result<usize, PinaPodError> {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        match PFX {
            1 => Ok(self.len[0] as usize),
            2 => Ok(u16::from_le_bytes([self.len[0], self.len[1]]) as usize),
            _ => {
                let mut buf = [0u8; 8];
                buf[..PFX].copy_from_slice(&self.len);
                let raw = u64::from_le_bytes(buf);
                if raw > usize::MAX as u64 {
                    Err(PinaPodError::InvalidLength)
                } else {
                    Ok(raw as usize)
                }
            }
        }
    }

    /// <!-- {=podRawDecodeLenContract|trim|linePrefix:"/// ":true|indent:"    "} -->
    /// The raw decoded length prefix.
    ///
    /// This is the unvalidated prefix value. On a prefix wider than `usize` (eight-byte prefixes on 32-bit targets) the sentinel `usize::MAX` is returned. Safe accessors such as [`len`](Self::len) clamp the value to the capacity; readers reject it during validation.<!-- {/podRawDecodeLenContract} -->
    #[inline(always)]
    pub fn decode_len(&self) -> usize {
        self.try_decode_len().unwrap_or(usize::MAX)
    }

    #[inline(always)]
    fn encode_len(&mut self, n: usize) {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        match PFX {
            1 => self.len[0] = n as u8,
            2 => {
                let bytes = (n as u16).to_le_bytes();
                self.len[0] = bytes[0];
                self.len[1] = bytes[1];
            }
            _ => {
                let bytes = (n as u64).to_le_bytes();
                self.len.copy_from_slice(&bytes[..PFX]);
            }
        }
    }

    #[inline(always)]
    fn zero_range(&mut self, range: core::ops::Range<usize>) {
        self.data[range].fill(MaybeUninit::zeroed());
    }

    /// <!-- {=podClampedLenContract|trim|linePrefix:"/// ":true|indent:"    "} -->
    /// The active length, clamped to the fixed capacity `N`.
    ///
    /// A forged or corrupt prefix can decode above `N`; this accessor never trusts it. Callers that need to distinguish a corrupt prefix from a valid one must validate through a reader first (see [`ZcValidate`](crate::ZcValidate)).<!-- {/podClampedLenContract} -->
    #[inline(always)]
    pub fn len(&self) -> usize {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        self.decode_len().min(N)
    }

    /// Returns `true` when no elements are active.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The fixed element capacity `N`.
    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Borrows the active elements.
    ///
    /// The length is clamped to the capacity, so this never reads past `N`. Validate
    /// through a reader first when the prefix itself must be trusted.
    #[inline(always)]
    pub fn as_slice(&self) -> &[T] {
        let len = self.len();
        // SAFETY: data[..len] written by push/set methods. MaybeUninit<T> and T have
        // identical layout. len clamped to N.
        unsafe { core::slice::from_raw_parts(self.data.as_ptr() as *const T, len) }
    }

    /// Mutably borrows the active elements.
    ///
    /// Only the active range is exposed; inactive capacity stays out of reach.
    #[inline(always)]
    pub fn as_slice_mut(&mut self) -> &mut [T] {
        let len = self.len();
        unsafe { core::slice::from_raw_parts_mut(self.data.as_mut_ptr() as *mut T, len) }
    }

    /// Returns a reference to the element at `index`, or `None` when out of bounds.
    #[inline(always)]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.as_slice().get(index)
    }

    /// Returns a mutable reference to the element at `index`, or `None` when out of bounds.
    #[inline(always)]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.as_slice_mut().get_mut(index)
    }

    /// Iterates the active elements.
    #[inline(always)]
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    /// Mutably iterates the active elements.
    #[inline(always)]
    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, T> {
        self.as_slice_mut().iter_mut()
    }

    /// Appends one value.
    ///
    /// # Errors
    ///
    /// <!-- {=podWriteCapacityContract|trim|linePrefix:"/// ":true|indent:"    "} -->
    /// Returns [`PinaPodError::Overflow`](crate::PinaPodError::Overflow) when the write would exceed the fixed capacity.
    ///
    /// The destination keeps its previous contents, so a rejected write is a no-op.<!-- {/podWriteCapacityContract} -->
    pub fn try_push<V: Into<T>>(&mut self, value: V) -> Result<(), PinaPodError> {
        let cur = self.len();
        if cur >= N {
            return Err(PinaPodError::Overflow);
        }
        self.data[cur] = MaybeUninit::new(value.into());
        self.encode_len(cur + 1);
        Ok(())
    }

    /// Replaces the active values from a representation slice.
    ///
    /// # Errors
    ///
    /// <!-- {=podWriteCapacityContract|trim|linePrefix:"/// ":true|indent:"    "} -->
    /// Returns [`PinaPodError::Overflow`](crate::PinaPodError::Overflow) when the write would exceed the fixed capacity.
    ///
    /// The destination keeps its previous contents, so a rejected write is a no-op.<!-- {/podWriteCapacityContract} -->
    pub fn try_set_from_slice(&mut self, values: &[T]) -> Result<(), PinaPodError> {
        let vlen = values.len();
        if vlen > N {
            return Err(PinaPodError::Overflow);
        }
        let old_len = self.len();
        unsafe {
            core::ptr::copy_nonoverlapping(values.as_ptr(), self.data.as_mut_ptr() as *mut T, vlen);
        }
        if vlen < old_len {
            self.zero_range(vlen..old_len);
        }
        self.encode_len(vlen);
        Ok(())
    }

    /// Replaces the active values from a slice-like collection.
    ///
    /// Each item converts to the stored `PinaPod` representation. This accepts
    /// native values for schema-facing aliases such as `PodVec<u64, N>` while
    /// retaining [`try_set_from_slice`](Self::try_set_from_slice) for an
    /// already-converted representation slice.
    ///
    /// # Errors
    ///
    /// <!-- {=podWriteCapacityContract|trim|linePrefix:"/// ":true|indent:"    "} -->
    /// Returns [`PinaPodError::Overflow`](crate::PinaPodError::Overflow) when the write would exceed the fixed capacity.
    ///
    /// The destination keeps its previous contents, so a rejected write is a no-op.<!-- {/podWriteCapacityContract} -->
    pub fn try_set<V>(&mut self, values: impl AsRef<[V]>) -> Result<(), PinaPodError>
    where
        V: Copy + Into<T>,
    {
        let values = values.as_ref();
        let new_len = values.len();
        if new_len > N {
            return Err(PinaPodError::Overflow);
        }

        let old_len = self.len();
        for (slot, value) in self.data.iter_mut().zip(values) {
            *slot = MaybeUninit::new((*value).into());
        }

        if new_len < old_len {
            self.zero_range(new_len..old_len);
        }
        self.encode_len(new_len);
        Ok(())
    }

    /// Appends values from a representation slice.
    ///
    /// # Errors
    ///
    /// <!-- {=podWriteCapacityContract|trim|linePrefix:"/// ":true|indent:"    "} -->
    /// Returns [`PinaPodError::Overflow`](crate::PinaPodError::Overflow) when the write would exceed the fixed capacity.
    ///
    /// The destination keeps its previous contents, so a rejected write is a no-op.<!-- {/podWriteCapacityContract} -->
    pub fn try_extend_from_slice(&mut self, values: &[T]) -> Result<(), PinaPodError> {
        let cur = self.len();
        let new_len = cur
            .checked_add(values.len())
            .filter(|new_len| *new_len <= N)
            .ok_or(PinaPodError::Overflow)?;
        unsafe {
            core::ptr::copy_nonoverlapping(
                values.as_ptr(),
                (self.data.as_mut_ptr() as *mut T).add(cur),
                values.len(),
            );
        }
        self.encode_len(new_len);
        Ok(())
    }

    /// Appends native or representation values from a slice-like collection.
    ///
    /// # Errors
    ///
    /// <!-- {=podWriteCapacityContract|trim|linePrefix:"/// ":true|indent:"    "} -->
    /// Returns [`PinaPodError::Overflow`](crate::PinaPodError::Overflow) when the write would exceed the fixed capacity.
    ///
    /// The destination keeps its previous contents, so a rejected write is a no-op.<!-- {/podWriteCapacityContract} -->
    pub fn try_extend<V>(&mut self, values: impl AsRef<[V]>) -> Result<(), PinaPodError>
    where
        V: Copy + Into<T>,
    {
        let values = values.as_ref();
        let cur = self.len();
        let added_len = values.len();
        let new_len = cur
            .checked_add(added_len)
            .filter(|new_len| *new_len <= N)
            .ok_or(PinaPodError::Overflow)?;

        for (slot, value) in self.data[cur..new_len].iter_mut().zip(values) {
            *slot = MaybeUninit::new((*value).into());
        }

        self.encode_len(new_len);
        Ok(())
    }

    /// Removes and returns the last element, or `None` when the vector is empty.
    ///
    /// <!-- {=podZeroedInactiveCapacityContract|trim|linePrefix:"/// ":true|indent:"    "} -->
    /// Every container starts with fully initialized backing storage.
    ///
    /// Operations that shorten or clear active data zero the bytes they vacate, so a later raw read or canonical serialization cannot disclose a previous value.<!-- {/podZeroedInactiveCapacityContract} -->
    #[must_use = "returns None if the vector is empty"]
    #[inline(always)]
    pub fn pop(&mut self) -> Option<T> {
        let cur = self.len();
        if cur == 0 {
            return None;
        }
        let new_len = cur - 1;
        let val = unsafe { self.data[new_len].assume_init() };
        self.data[new_len] = MaybeUninit::zeroed();
        self.encode_len(new_len);
        Some(val)
    }

    /// Removes the element at `index` and returns it, filling the gap with the last element.
    ///
    /// This moves only one element but does not preserve order. Returns `None` when
    /// `index` is out of bounds.
    ///
    /// <!-- {=podZeroedInactiveCapacityContract|trim|linePrefix:"/// ":true|indent:"    "} -->
    /// Every container starts with fully initialized backing storage.
    ///
    /// Operations that shorten or clear active data zero the bytes they vacate, so a later raw read or canonical serialization cannot disclose a previous value.<!-- {/podZeroedInactiveCapacityContract} -->
    #[must_use = "returns None if index is out of bounds"]
    #[inline(always)]
    pub fn swap_remove(&mut self, index: usize) -> Option<T> {
        let cur = self.len();
        if index >= cur {
            return None;
        }
        let last = cur - 1;
        let removed = unsafe { self.data[index].assume_init() };
        if index != last {
            self.data[index] = self.data[last];
        }
        self.data[last] = MaybeUninit::zeroed();
        self.encode_len(last);
        Some(removed)
    }

    /// Removes the element at `index` and returns it, shifting the later elements down.
    ///
    /// This preserves order and moves every element after `index`. Returns `None` when
    /// `index` is out of bounds.
    ///
    /// <!-- {=podZeroedInactiveCapacityContract|trim|linePrefix:"/// ":true|indent:"    "} -->
    /// Every container starts with fully initialized backing storage.
    ///
    /// Operations that shorten or clear active data zero the bytes they vacate, so a later raw read or canonical serialization cannot disclose a previous value.<!-- {/podZeroedInactiveCapacityContract} -->
    #[must_use = "returns None if index is out of bounds"]
    #[inline(always)]
    pub fn remove(&mut self, index: usize) -> Option<T> {
        let cur = self.len();
        if index >= cur {
            return None;
        }
        let removed = unsafe { self.data[index].assume_init() };
        let tail = cur - index - 1;
        if tail > 0 {
            unsafe {
                core::ptr::copy(
                    self.data.as_ptr().add(index + 1),
                    self.data.as_mut_ptr().add(index),
                    tail,
                );
            }
        }
        let new_len = cur - 1;
        self.data[new_len] = MaybeUninit::zeroed();
        self.encode_len(new_len);
        Some(removed)
    }

    /// Shortens the active elements to at most `new_len`.
    ///
    /// <!-- {=podZeroedInactiveCapacityContract|trim|linePrefix:"/// ":true|indent:"    "} -->
    /// Every container starts with fully initialized backing storage.
    ///
    /// Operations that shorten or clear active data zero the bytes they vacate, so a later raw read or canonical serialization cannot disclose a previous value.<!-- {/podZeroedInactiveCapacityContract} -->
    #[inline(always)]
    pub fn truncate(&mut self, new_len: usize) {
        let cur = self.len();
        if new_len < cur {
            self.zero_range(new_len..cur);
            self.encode_len(new_len);
        }
    }

    /// Keeps only the elements for which `f` returns `true`, preserving their order.
    ///
    /// <!-- {=podZeroedInactiveCapacityContract|trim|linePrefix:"/// ":true|indent:"    "} -->
    /// Every container starts with fully initialized backing storage.
    ///
    /// Operations that shorten or clear active data zero the bytes they vacate, so a later raw read or canonical serialization cannot disclose a previous value.<!-- {/podZeroedInactiveCapacityContract} -->
    pub fn retain(&mut self, mut f: impl FnMut(&T) -> bool) {
        let mut write = 0;
        let cur = self.len();
        for read in 0..cur {
            let val = unsafe { self.data[read].assume_init() };
            if f(&val) {
                self.data[write] = MaybeUninit::new(val);
                write += 1;
            }
        }
        self.zero_range(write..cur);
        self.encode_len(write);
    }

    /// Removes every element.
    ///
    /// <!-- {=podZeroedInactiveCapacityContract|trim|linePrefix:"/// ":true|indent:"    "} -->
    /// Every container starts with fully initialized backing storage.
    ///
    /// Operations that shorten or clear active data zero the bytes they vacate, so a later raw read or canonical serialization cannot disclose a previous value.<!-- {/podZeroedInactiveCapacityContract} -->
    #[inline(always)]
    pub fn clear(&mut self) {
        self.zero_range(0..self.len());
        self.len = [0u8; PFX];
    }
}

impl<T: ZcElem, const N: usize, const PFX: usize> Default for PodVecRepr<T, N, PFX> {
    fn default() -> Self {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;

        Self {
            len: [0u8; PFX],
            // Typed assignments and compact copies include inactive capacity.
            data: [MaybeUninit::zeroed(); N],
        }
    }
}

impl<T: ZcElem, const N: usize, const PFX: usize> core::ops::Deref for PodVecRepr<T, N, PFX> {
    type Target = [T];

    #[inline(always)]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: ZcElem, const N: usize, const PFX: usize> core::ops::DerefMut for PodVecRepr<T, N, PFX> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_slice_mut()
    }
}

impl<T: ZcElem, const N: usize, const PFX: usize> AsRef<[T]> for PodVecRepr<T, N, PFX> {
    #[inline(always)]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: ZcElem, const N: usize, const PFX: usize> AsMut<[T]> for PodVecRepr<T, N, PFX> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [T] {
        self.as_slice_mut()
    }
}

impl<'a, T: ZcElem, const N: usize, const PFX: usize> IntoIterator for &'a PodVecRepr<T, N, PFX> {
    type IntoIter = core::slice::Iter<'a, T>;
    type Item = &'a T;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: ZcElem, const N: usize, const PFX: usize> IntoIterator
    for &'a mut PodVecRepr<T, N, PFX>
{
    type IntoIter = core::slice::IterMut<'a, T>;
    type Item = &'a mut T;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: ZcElem + PartialEq, const N: usize, const PFX: usize> PartialEq for PodVecRepr<T, N, PFX> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: ZcElem + PartialEq, const N: usize, const PFX: usize> PartialEq<[T]>
    for PodVecRepr<T, N, PFX>
{
    #[inline(always)]
    fn eq(&self, other: &[T]) -> bool {
        self.as_slice() == other
    }
}

impl<T: ZcElem + PartialEq, const N: usize, const PFX: usize> PartialEq<&[T]>
    for PodVecRepr<T, N, PFX>
{
    #[inline(always)]
    fn eq(&self, other: &&[T]) -> bool {
        self.as_slice() == *other
    }
}

impl<T: ZcElem + Eq, const N: usize, const PFX: usize> Eq for PodVecRepr<T, N, PFX> {}

impl<T: ZcElem + core::fmt::Debug, const N: usize, const PFX: usize> core::fmt::Debug
    for PodVecRepr<T, N, PFX>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_slice(), f)
    }
}

impl<T: ZcElem + core::hash::Hash, const N: usize, const PFX: usize> core::hash::Hash
    for PodVecRepr<T, N, PFX>
{
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(all(kani, feature = "kani"))]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn encode_decode_roundtrip_pfx1() {
        let n: usize = kani::any();
        kani::assume(n <= u8::MAX as usize);
        let mut v = PodVecRepr::<u8, 255, 1>::default();
        v.encode_len(n);
        assert!(v.decode_len() == n);
    }

    #[kani::proof]
    fn encode_decode_roundtrip_pfx2() {
        let n: usize = kani::any();
        kani::assume(n <= u16::MAX as usize);
        let mut v = PodVecRepr::<u8, 255, 2>::default();
        v.encode_len(n);
        assert!(v.decode_len() == n);
    }

    #[kani::proof]
    fn encode_decode_roundtrip_pfx4() {
        let n: usize = kani::any();
        kani::assume(n <= u32::MAX as usize);
        let mut v = PodVecRepr::<u8, 255, 4>::default();
        v.encode_len(n);
        assert!(v.decode_len() == n);
    }

    #[kani::proof]
    fn len_clamp_pfx2() {
        let raw: [u8; 2] = kani::any();
        let v = PodVecRepr::<u8, 8, 2> {
            len: raw,
            data: [MaybeUninit::zeroed(); 8],
        };
        assert!(v.len() <= 8);
    }

    #[kani::proof]
    fn len_clamp_pfx1() {
        let raw: [u8; 1] = kani::any();
        let v = PodVecRepr::<u8, 8, 1> {
            len: raw,
            data: [MaybeUninit::zeroed(); 8],
        };
        assert!(v.len() <= 8);
    }

    #[kani::proof]
    fn push_pop_roundtrip() {
        let val: u8 = kani::any();
        let mut v = PodVecRepr::<u8, 4, 1>::default();
        assert!(v.try_push(val).is_ok());
        assert!(v.len() == 1);
        assert!(v.pop() == Some(val));
        assert!(v.is_empty());
    }

    #[kani::proof]
    fn push_full_rejects() {
        let mut v = PodVecRepr::<u8, 2, 1>::default();
        assert!(v.try_push(1).is_ok());
        assert!(v.try_push(2).is_ok());
        assert!(v.try_push(3).is_err());
        assert!(v.len() == 2);
    }

    #[kani::proof]
    fn push_pop_lifo() {
        let a: u8 = kani::any();
        let b: u8 = kani::any();
        let mut v = PodVecRepr::<u8, 4, 1>::default();
        assert!(v.try_push(a).is_ok());
        assert!(v.try_push(b).is_ok());
        assert!(v.pop() == Some(b));
        assert!(v.pop() == Some(a));
    }

    #[kani::proof]
    fn swap_remove_correctness() {
        let a: u8 = kani::any();
        let b: u8 = kani::any();
        let c: u8 = kani::any();
        let mut v = PodVecRepr::<u8, 4, 1>::default();
        assert!(v.try_push(a).is_ok());
        assert!(v.try_push(b).is_ok());
        assert!(v.try_push(c).is_ok());
        assert!(v.swap_remove(0) == Some(a));
        assert!(v.len() == 2);
        assert!(v.as_slice()[0] == c);
        assert!(v.as_slice()[1] == b);
    }

    #[kani::proof]
    fn swap_remove_oob() {
        let idx: usize = kani::any();
        let mut v = PodVecRepr::<u8, 4, 1>::default();
        assert!(v.try_push(1).is_ok());
        assert!(v.try_push(2).is_ok());
        kani::assume(idx >= 2);
        kani::assume(idx <= 8);
        assert!(v.swap_remove(idx).is_none());
        assert!(v.len() == 2);
    }

    #[kani::proof]
    fn set_from_slice_rejects_over_capacity() {
        let count: usize = kani::any();
        kani::assume(count > 4);
        kani::assume(count <= 8);
        let data = [0u8; 8];
        let mut v = PodVecRepr::<u8, 4, 1>::default();
        assert!(v.try_set_from_slice(&data[..count]).is_err());
        assert!(v.is_empty());
    }
}
