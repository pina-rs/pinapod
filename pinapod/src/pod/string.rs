use {crate::error::PinaPodError, core::mem::MaybeUninit};

/// Returns the maximum `N` value representable by a `PFX`-byte length prefix.
///
/// Returns `0` for invalid `PFX` values, which causes `_CAP_CHECK` to fire.
pub(crate) const fn max_n_for_pfx(pfx: usize) -> usize {
    match pfx {
        1 => u8::MAX as usize,
        2 => u16::MAX as usize,
        4 => u32::MAX as usize,
        8 => usize::MAX,
        _ => 0,
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PodString<const N: usize, const PFX: usize = 1> {
    len: [u8; PFX],
    pub(crate) data: [MaybeUninit<u8>; N],
}

// Compile-time: PFX must be in {1,2,4,8} and N must fit in the prefix.
impl<const N: usize, const PFX: usize> PodString<N, PFX> {
    const _CAP_CHECK: () = {
        assert!(
            PFX == 1 || PFX == 2 || PFX == 4 || PFX == 8,
            "PodString<N, PFX>: PFX must be 1, 2, 4, or 8"
        );
        assert!(
            N <= max_n_for_pfx(PFX),
            "PodString<N, PFX>: N exceeds the maximum value representable by the PFX-byte length \
             prefix"
        );
    };

    pub const VALID: () = Self::_CAP_CHECK;
}

// Compile-time layout invariants — PFX=1 (default, backward-compat).
const _: () = assert!(core::mem::size_of::<PodString<0>>() == 1);
const _: () = assert!(core::mem::size_of::<PodString<1>>() == 2);
const _: () = assert!(core::mem::size_of::<PodString<32>>() == 33);
const _: () = assert!(core::mem::size_of::<PodString<255>>() == 256);
const _: () = assert!(core::mem::align_of::<PodString<0>>() == 1);
const _: () = assert!(core::mem::align_of::<PodString<32>>() == 1);
const _: () = assert!(core::mem::align_of::<PodString<255>>() == 1);
// Compile-time layout invariants — PFX=2.
const _: () = assert!(core::mem::size_of::<PodString<0, 2>>() == 2);
const _: () = assert!(core::mem::size_of::<PodString<100, 2>>() == 102);
const _: () = assert!(core::mem::align_of::<PodString<0, 2>>() == 1);
// Compile-time layout invariants — PFX=4.
const _: () = assert!(core::mem::size_of::<PodString<0, 4>>() == 4);
const _: () = assert!(core::mem::size_of::<PodString<100, 4>>() == 104);
const _: () = assert!(core::mem::align_of::<PodString<0, 4>>() == 1);
// Compile-time layout invariants — PFX=8.
const _: () = assert!(core::mem::size_of::<PodString<0, 8>>() == 8);
const _: () = assert!(core::mem::align_of::<PodString<0, 8>>() == 1);

impl<const N: usize, const PFX: usize> PodString<N, PFX> {
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

    /// The raw decoded length prefix.
    ///
    /// This is the unvalidated prefix value. On a prefix wider than `usize`
    /// (eight-byte prefixes on 32-bit targets), the sentinel `usize::MAX` is
    /// returned. Safe accessors such as [`len`](Self::len) clamp the value to
    /// the capacity; readers reject it during validation.
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

    /// The number of active bytes, clamped to the fixed capacity `N`.
    ///
    /// A forged or corrupt prefix can decode above `N`; this accessor never
    /// trusts it. Callers that need to distinguish a corrupt prefix from a
    /// valid one must validate through a reader first (see [`ZcValidate`]).
    ///
    /// [`ZcValidate`]: crate::ZcValidate
    #[inline(always)]
    pub fn len(&self) -> usize {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        self.decode_len().min(N)
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        N
    }

    #[inline(always)]
    pub fn as_str(&self) -> &str {
        let len = self.len();
        unsafe {
            let bytes = core::slice::from_raw_parts(self.data.as_ptr() as *const u8, len);
            core::str::from_utf8_unchecked(bytes)
        }
    }

    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        let len = self.len();
        unsafe { core::slice::from_raw_parts(self.data.as_ptr() as *const u8, len) }
    }

    /// Replaces the string contents.
    ///
    /// # Errors
    ///
    /// Returns [`PinaPodError::Overflow`] when `value` exceeds the fixed capacity.
    pub fn try_set(&mut self, value: &str) -> Result<(), PinaPodError> {
        let vlen = value.len();
        if vlen > N {
            return Err(PinaPodError::Overflow);
        }
        let old_len = self.len();
        unsafe {
            core::ptr::copy_nonoverlapping(value.as_ptr(), self.data.as_mut_ptr() as *mut u8, vlen);
        }
        if vlen < old_len {
            self.zero_range(vlen..old_len);
        }
        self.encode_len(vlen);
        Ok(())
    }

    /// Appends a string slice.
    ///
    /// # Errors
    ///
    /// Returns [`PinaPodError::Overflow`] when the combined contents exceed the fixed capacity.
    pub fn try_push_str(&mut self, value: &str) -> Result<(), PinaPodError> {
        let cur = self.len();
        let vlen = value.len();
        let new_len = cur.checked_add(vlen).ok_or(PinaPodError::Overflow)?;
        if new_len > N {
            return Err(PinaPodError::Overflow);
        }
        unsafe {
            core::ptr::copy_nonoverlapping(
                value.as_ptr(),
                (self.data.as_mut_ptr() as *mut u8).add(cur),
                vlen,
            );
        }
        self.encode_len(new_len);
        Ok(())
    }

    #[inline(always)]
    pub fn chars(&self) -> core::str::Chars<'_> {
        self.as_str().chars()
    }

    #[inline(always)]
    pub fn bytes(&self) -> core::str::Bytes<'_> {
        self.as_str().bytes()
    }

    #[inline(always)]
    pub fn truncate(&mut self, new_len: usize) {
        if new_len >= self.len() {
            return;
        }
        let s = self.as_str();
        let mut boundary = new_len;
        while boundary > 0 && !s.is_char_boundary(boundary) {
            boundary -= 1;
        }
        self.zero_range(boundary..self.len());
        self.encode_len(boundary);
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.zero_range(0..self.len());
        self.len = [0u8; PFX];
    }
}

impl<const N: usize, const PFX: usize> Default for PodString<N, PFX> {
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

impl<const N: usize, const PFX: usize> core::ops::Deref for PodString<N, PFX> {
    type Target = str;

    #[inline(always)]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize, const PFX: usize> AsRef<str> for PodString<N, PFX> {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize, const PFX: usize> AsRef<[u8]> for PodString<N, PFX> {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<const N: usize, const PFX: usize> PartialEq for PodString<N, PFX> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl<const N: usize, const PFX: usize> Eq for PodString<N, PFX> {}

impl<const N: usize, const PFX: usize> PartialEq<str> for PodString<N, PFX> {
    #[inline(always)]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl<const N: usize, const PFX: usize> PartialEq<&str> for PodString<N, PFX> {
    #[inline(always)]
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl<const N: usize, const PFX: usize> core::fmt::Debug for PodString<N, PFX> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_str(), f)
    }
}

impl<const N: usize, const PFX: usize> core::fmt::Display for PodString<N, PFX> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<const N: usize, const PFX: usize> core::hash::Hash for PodString<N, PFX> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl<const N: usize, const PFX: usize> TryFrom<&str> for PodString<N, PFX> {
    type Error = PinaPodError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut pod_str = PodString::default();
        pod_str.try_push_str(value)?;
        Ok(pod_str)
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
        let mut s = PodString::<255, 1>::default();
        s.encode_len(n);
        assert!(s.decode_len() == n);
    }

    #[kani::proof]
    fn encode_decode_roundtrip_pfx2() {
        let n: usize = kani::any();
        kani::assume(n <= u16::MAX as usize);
        let mut s = PodString::<255, 2>::default();
        s.encode_len(n);
        assert!(s.decode_len() == n);
    }

    #[kani::proof]
    fn encode_decode_roundtrip_pfx4() {
        let n: usize = kani::any();
        kani::assume(n <= u32::MAX as usize);
        let mut s = PodString::<255, 4>::default();
        s.encode_len(n);
        assert!(s.decode_len() == n);
    }

    #[kani::proof]
    fn len_clamp_pfx1() {
        let raw: [u8; 1] = kani::any();
        let s = PodString::<8, 1> {
            len: raw,
            data: [MaybeUninit::zeroed(); 8],
        };
        assert!(s.len() <= 8);
    }

    #[kani::proof]
    fn len_clamp_pfx2() {
        let raw: [u8; 2] = kani::any();
        let s = PodString::<8, 2> {
            len: raw,
            data: [MaybeUninit::zeroed(); 8],
        };
        assert!(s.len() <= 8);
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn set_then_as_bytes_len() {
        let vlen: usize = kani::any();
        kani::assume(vlen <= 8);
        let content = [0x41u8; 8];
        let mut s = PodString::<8>::default();
        let result = s.try_set(unsafe { core::str::from_utf8_unchecked(&content[..vlen]) });
        assert!(result.is_ok());
        assert!(s.len() == vlen);
        assert!(s.as_bytes().len() == vlen);
    }

    #[kani::proof]
    fn set_rejects_over_capacity() {
        let vlen: usize = kani::any();
        kani::assume(vlen > 4);
        kani::assume(vlen <= 8);
        let content = [0x41u8; 8];
        let mut s = PodString::<4>::default();
        assert!(s
            .try_set(unsafe { core::str::from_utf8_unchecked(&content[..vlen]) })
            .is_err());
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn push_str_len_accounting() {
        let a_len: usize = kani::any();
        let b_len: usize = kani::any();
        kani::assume(a_len <= 4);
        kani::assume(b_len <= 4);
        kani::assume(a_len + b_len <= 8);

        let buf = [0x41u8; 8];
        let mut s = PodString::<8>::default();
        assert!(s
            .try_set(unsafe { core::str::from_utf8_unchecked(&buf[..a_len]) })
            .is_ok());
        assert!(s
            .try_push_str(unsafe { core::str::from_utf8_unchecked(&buf[..b_len]) })
            .is_ok());
        assert!(s.len() == a_len + b_len);
    }

    #[kani::proof]
    fn push_str_rejects_overflow() {
        let a_len: usize = kani::any();
        let b_len: usize = kani::any();
        kani::assume(a_len <= 4);
        kani::assume(b_len <= 8);
        kani::assume(a_len + b_len > 4);

        let buf = [0x41u8; 8];
        let mut s = PodString::<4>::default();
        assert!(s
            .try_set(unsafe { core::str::from_utf8_unchecked(&buf[..a_len]) })
            .is_ok());
        assert!(s
            .try_push_str(unsafe { core::str::from_utf8_unchecked(&buf[..b_len]) })
            .is_err());
        assert!(s.len() == a_len);
    }
}
