#![allow(
    unsafe_code,
    unused_qualifications,
    reason = "adversarial tests construct raw layouts and preserve explicit upstream trait paths"
)]

use pinapod::{pod::*, PinaPodError};

// ---- PodOption tests ----

#[test]
fn pod_option_none() {
    let opt = PodOption::<u8>::none();
    assert!(opt.is_none());
    assert!(!opt.is_some());
    assert!(opt.tag_valid());
    assert_eq!(opt.get(), None);
    assert_eq!(opt.raw_tag(), 0);
}

#[test]
fn pod_option_some() {
    let opt = PodOption::<u8>::some(42u8);
    assert!(opt.is_some());
    assert!(!opt.is_none());
    assert!(opt.tag_valid());
    assert_eq!(opt.get(), Some(42u8));
    assert_eq!(opt.raw_tag(), 1);
}

#[test]
fn pod_option_tag_valid_all_prefixes() {
    assert!(PodOption::<u8, 1>::some(1).tag_valid());
    assert!(PodOption::<u8, 2>::some(1).tag_valid());
    assert!(PodOption::<u8, 4>::some(1).tag_valid());
    assert!(PodOption::<u8, 8>::some(1).tag_valid());

    let invalid = [2u8, 0, 0, 0, 0];
    let opt = unsafe { &*invalid.as_ptr().cast::<PodOption<u8, 4>>() };
    assert_eq!(opt.raw_tag(), 2);
    assert!(!opt.tag_valid());
}

#[test]
fn pod_option_eight_byte_prefix_roundtrip_and_rejection() {
    let mut buffer = [0u8; 9];
    buffer[..8].copy_from_slice(&1u64.to_le_bytes());
    buffer[8] = 7;
    let opt = unsafe { &*buffer.as_ptr().cast::<PodOption<u8, 8>>() };
    assert_eq!(opt.raw_tag(), 1);
    assert_eq!(opt.get(), Some(7));
    assert!(pinapod::ZcValidate::validate_ref(opt).is_ok());

    // A tag above one must be rejected without truncation.
    buffer[..8].copy_from_slice(&0x1_0000_0001u64.to_le_bytes());
    let opt = unsafe { &*buffer.as_ptr().cast::<PodOption<u8, 8>>() };
    assert_eq!(opt.raw_tag(), 0x1_0000_0001);
    assert!(!opt.tag_valid());
    assert!(opt.is_none());
    assert_eq!(
        pinapod::ZcValidate::validate_ref(opt),
        Err(PinaPodError::InvalidTag)
    );

    let built = PodOption::<u8, 8>::some(7);
    let built_bytes = unsafe { core::slice::from_raw_parts((&built as *const _) as *const u8, 9) };
    assert_eq!(&built_bytes[..8], &1u64.to_le_bytes());
    assert_eq!(built_bytes[8], 7);
}

#[test]
fn pod_option_set() {
    let mut opt = PodOption::<u8>::none();
    opt.set(Some(10));
    assert_eq!(opt.get(), Some(10));
    opt.set(None);
    assert!(opt.is_none());
}

#[test]
fn pod_option_alignment() {
    assert_eq!(core::mem::align_of::<PodOption<u8>>(), 1);
    assert_eq!(core::mem::align_of::<PodOption<[u8; 32]>>(), 1);
}

#[test]
fn pod_option_size() {
    // tag (1) + value size
    assert_eq!(core::mem::size_of::<PodOption<u8>>(), 2);
    assert_eq!(core::mem::size_of::<PodOption<[u8; 32]>>(), 33);
}

#[test]
fn pod_option_default() {
    let opt = PodOption::<u8>::default();
    assert!(opt.is_none());
}

#[test]
fn pod_option_eq() {
    let first = PodOption::<u8>::some(5u8);
    let same = PodOption::<u8>::some(5u8);
    let different = PodOption::<u8>::some(6u8);
    let absent = PodOption::<u8>::none();
    let also_absent = PodOption::<u8>::none();
    assert_eq!(first, same);
    assert_ne!(first, different);
    assert_ne!(first, absent);
    assert_eq!(absent, also_absent);
}

#[test]
fn pod_option_debug() {
    let some = PodOption::<u8>::some(42u8);
    let none = PodOption::<u8>::none();
    assert_eq!(format!("{some:?}"), "Some(42)");
    assert_eq!(format!("{none:?}"), "None");
}

// ---- Numeric pod type smoke tests ----

#[test]
fn numeric_alignment() {
    assert_eq!(core::mem::align_of::<PodU16>(), 1);
    assert_eq!(core::mem::align_of::<PodU32>(), 1);
    assert_eq!(core::mem::align_of::<PodU64>(), 1);
    assert_eq!(core::mem::align_of::<PodU128>(), 1);
    assert_eq!(core::mem::align_of::<PodI16>(), 1);
    assert_eq!(core::mem::align_of::<PodI32>(), 1);
    assert_eq!(core::mem::align_of::<PodI64>(), 1);
    assert_eq!(core::mem::align_of::<PodI128>(), 1);
    assert_eq!(core::mem::align_of::<PodBool>(), 1);
}

#[test]
fn numeric_size() {
    assert_eq!(core::mem::size_of::<PodU16>(), 2);
    assert_eq!(core::mem::size_of::<PodU32>(), 4);
    assert_eq!(core::mem::size_of::<PodU64>(), 8);
    assert_eq!(core::mem::size_of::<PodU128>(), 16);
    assert_eq!(core::mem::size_of::<PodI16>(), 2);
    assert_eq!(core::mem::size_of::<PodI32>(), 4);
    assert_eq!(core::mem::size_of::<PodI64>(), 8);
    assert_eq!(core::mem::size_of::<PodI128>(), 16);
    assert_eq!(core::mem::size_of::<PodBool>(), 1);
}

// ---- PodU64 roundtrip, explicit arithmetic, comparison ----

#[test]
fn pod_u64_roundtrip() {
    let val = 123_456_789_u64;
    let pod = PodU64::from(val);
    assert_eq!(pod.get(), val);
    let back: u64 = pod.into();
    assert_eq!(back, val);
}

#[test]
fn pod_u32_new_from_array() {
    let pod = PodU32::new_from_array([0x78, 0x56, 0x34, 0x12]);

    assert_eq!(pod.get(), 0x1234_5678);
    assert_eq!(pod, PodU32::from(0x1234_5678));
}

#[test]
fn pod_u64_comparison() {
    let a = PodU64::from(100u64);
    let b = PodU64::from(200u64);
    assert!(a < b);
    assert!(b > a);
    assert!(a == PodU64::from(100u64));
    assert!(a != b);
    assert!(a == 100u64);
}

#[test]
fn pod_u64_checked_arithmetic() {
    let max = PodU64::MAX;
    assert!(max.checked_add(PodU64::from(1u64)).is_none());
    assert_eq!(
        PodU64::from(10u64).checked_add(PodU64::from(5u64)),
        Some(PodU64::from(15u64))
    );
    assert!(PodU64::ZERO.checked_sub(PodU64::from(1u64)).is_none());
    assert!(PodU64::from(5u64).checked_div(PodU64::ZERO).is_none());
}

#[test]
fn pod_u64_is_zero() {
    assert!(PodU64::ZERO.is_zero());
    assert!(!PodU64::from(1u64).is_zero());
}

// ---- PodBool tests ----

#[test]
fn pod_bool_roundtrip() {
    assert!(PodBool::from(true).get());
    assert!(!PodBool::from(false).get());
    assert!(PodBool::from(true) == true);
    assert!(PodBool::from(false) == false);
}

// ---- PodString basic operations ----

#[test]
fn pod_string_basic() {
    let mut s = PodString::<32>::default();
    assert!(s.is_empty());
    s.try_set("hello").unwrap();
    assert_eq!(s.as_str(), "hello");
    assert_eq!(s.len(), 5);
}

#[test]
fn pod_string_alignment() {
    assert_eq!(core::mem::align_of::<PodString<32>>(), 1);
    assert_eq!(core::mem::align_of::<PodString<0>>(), 1);
    assert_eq!(core::mem::align_of::<PodString<32, 2>>(), 1);
}

#[test]
fn pod_string_overflow() {
    let mut s = PodString::<3>::default();
    assert!(s.try_set("abcd").is_err());
    assert!(s.is_empty());
}

#[test]
fn pod_string_push_str() {
    let mut s = PodString::<10>::default();
    s.try_set("hello").unwrap();
    s.try_push_str(" wor").unwrap();
    assert_eq!(s.as_str(), "hello wor");
    assert!(s.try_push_str("ld").is_err());
}

#[test]
fn pod_string_try_from() {
    let s = "hello";
    let pod_s: PodString<6> = PodString::try_from(s).unwrap();
    assert_eq!(s, pod_s.as_str());
    assert!(PodString::<4>::try_from(s).is_err());
}

// ---- PodVec basic operations ----

#[test]
fn pod_vec_basic() {
    let mut v = PodVec::<u8, 10>::default();
    assert!(v.is_empty());
    v.try_push(1).unwrap();
    v.try_push(2).unwrap();
    v.try_push(3).unwrap();
    assert_eq!(v.len(), 3);
    assert_eq!(v.as_slice(), &[1, 2, 3]);
}

#[test]
fn pod_vec_iterates_by_shared_and_mutable_reference() {
    let mut values = PodVec::<u8, 3>::default();
    values.try_set_from_slice(&[1, 2, 3]).unwrap();

    assert_eq!((&values).into_iter().copied().sum::<u8>(), 6);
    for value in &mut values {
        *value += 1;
    }
    assert_eq!(values.as_slice(), &[2, 3, 4]);
}

#[test]
fn pod_vec_alignment() {
    assert_eq!(core::mem::align_of::<PodVec<u8, 10>>(), 1);
    assert_eq!(core::mem::align_of::<PodVec<[u8; 32], 5>>(), 1);
    assert_eq!(core::mem::align_of::<PodVec<u8, 10, 1>>(), 1);
}

#[test]
fn pod_vec_maps_native_elements_with_an_explicit_prefix() {
    let mut values = PodVec::<u64, 1024, 2>::default();
    values.try_set([42_u64, 84]).unwrap();

    assert_eq!(core::mem::align_of_val(&values), 1);
    assert_eq!(values[0].get(), 42);
    assert_eq!(values[1].get(), 84);
}

#[test]
fn pod_vec_push_pop() {
    let mut v = PodVec::<u8, 3>::default();
    v.try_push(10).unwrap();
    v.try_push(20).unwrap();
    v.try_push(30).unwrap();
    assert!(v.try_push(40).is_err());
    assert_eq!(v.pop(), Some(30));
    assert_eq!(v.pop(), Some(20));
    assert_eq!(v.pop(), Some(10));
    assert_eq!(v.pop(), None);
}

#[test]
fn pod_vec_set_from_slice() {
    let mut v = PodVec::<u8, 5>::default();
    v.try_set_from_slice(&[1, 2, 3, 4, 5]).unwrap();
    assert_eq!(v.as_slice(), &[1, 2, 3, 4, 5]);
    assert!(v.try_set_from_slice(&[1, 2, 3, 4, 5, 6]).is_err());
}

use core::hash::{Hash, Hasher};

// A minimal hasher for testing
struct TestHasher(u64);
impl Hasher for TestHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = self.0.wrapping_mul(31).wrapping_add(u64::from(b));
        }
    }
}

#[test]
fn pod_u64_hash() {
    let a = PodU64::from(42u64);
    let b = PodU64::from(42u64);
    let mut ha = TestHasher(0);
    let mut hb = TestHasher(0);
    a.hash(&mut ha);
    b.hash(&mut hb);
    assert_eq!(ha.finish(), hb.finish());
}

#[test]
fn pod_u64_formatting() {
    let v = PodU64::from(255u64);
    assert_eq!(format!("{v:b}"), format!("{:b}", 255u64));
    assert_eq!(format!("{v:x}"), format!("{:x}", 255u64));
    assert_eq!(format!("{v:X}"), format!("{:X}", 255u64));
}

#[test]
fn pod_u64_wrapping() {
    let v = PodU64::from(u64::MAX);
    assert_eq!(v.wrapping_add(1u64).get(), 0u64);
    assert_eq!(PodU64::from(0u64).wrapping_sub(1u64).get(), u64::MAX);
    assert_eq!(
        PodU64::from(u64::MAX).wrapping_mul(2u64).get(),
        u64::MAX.wrapping_mul(2)
    );
}

#[test]
fn pod_u64_saturating() {
    assert_eq!(PodU64::MAX.saturating_add(1u64), PodU64::MAX);
    assert_eq!(PodU64::ZERO.saturating_sub(1u64), PodU64::ZERO);
    assert_eq!(PodU64::MAX.saturating_mul(2u64), PodU64::MAX);
}

#[test]
fn pod_u64_set() {
    let mut v = PodU64::from(0u64);
    v.set(42u64);
    assert_eq!(v.get(), 42u64);
}

#[test]
fn pod_i64_wrapping() {
    let v = PodI64::from(i64::MAX);
    assert_eq!(v.wrapping_add(1i64).get(), i64::MIN);
}

#[test]
fn pod_i64_explicit_negation() {
    let value = PodI64::from(42);

    assert_eq!(value.checked_neg(), Some(PodI64::from(-42)));
    assert!(PodI64::MIN.checked_neg().is_none());
    assert_eq!(PodI64::MIN.wrapping_neg(), PodI64::MIN);
}

#[test]
fn pod_bool_hash() {
    let a = PodBool::from(true);
    let b = PodBool::from(true);
    let mut ha = TestHasher(0);
    let mut hb = TestHasher(0);
    a.hash(&mut ha);
    b.hash(&mut hb);
    assert_eq!(ha.finish(), hb.finish());
}

#[test]
fn pod_bool_helpers() {
    assert!(PodBool::from(true).is_true());
    assert!(!PodBool::from(true).is_false());
    assert!(PodBool::from(false).is_false());
    assert!(!PodBool::from(false).is_true());
}

#[test]
fn pod_bool_bitops() {
    let t = PodBool::from(true);
    let f = PodBool::from(false);
    assert!((t & true).get());
    assert!(!(t & false).get());
    assert!((f | true).get());
    assert!(!(f | false).get());
}

#[test]
fn pod_bool_reverse_eq() {
    assert!(true == PodBool::from(true));
    assert!(false == PodBool::from(false));
    assert!(true != PodBool::from(false));
}

#[test]
fn pod_bool_set() {
    let mut b = PodBool::from(false);
    b.set(true);
    assert!(b.get());
}

#[test]
fn pod_numeric_as_ref_u8() {
    let v = PodU32::from(0x1234_5678u32);
    assert_eq!(v.as_ref(), &[0x78, 0x56, 0x34, 0x12]);

    let v = PodU64::from(0x0102_0304_0506_0708u64);
    assert_eq!(
        v.as_ref(),
        &[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
    );

    let v = PodI16::from(-1i16);
    assert_eq!(v.as_ref(), &[0xff, 0xff]);
}

#[test]
fn pod_bool_as_ref_u8() {
    assert_eq!(PodBool::from(true).as_ref(), &[1]);
    assert_eq!(PodBool::from(false).as_ref(), &[0]);
}

#[test]
fn pod_string_try_set() {
    let mut s = PodString::<4>::default();
    assert!(s.try_set("hi").is_ok());
    assert_eq!(s.as_str(), "hi");
    assert!(s.try_set("toolong").is_err());
    assert_eq!(s.as_str(), "hi"); // unchanged on error
}

#[test]
fn pod_string_try_push_str() {
    let mut s = PodString::<6>::default();
    assert!(s.try_push_str("hel").is_ok());
    assert!(s.try_push_str("lo!").is_ok());
    assert_eq!(s.as_str(), "hello!");
    assert!(s.try_push_str("x").is_err()); // full
}

#[test]
fn pod_string_capacity() {
    let s = PodString::<32>::default();
    assert_eq!(s.capacity(), 32);
}

#[test]
fn pod_string_chars_bytes() {
    let mut s = PodString::<32>::default();
    s.try_set("hello").unwrap();
    assert_eq!(s.chars().count(), 5);
    assert_eq!(s.bytes().count(), 5);
}

#[test]
fn pod_string_hash() {
    let mut a = PodString::<32>::default();
    let mut b = PodString::<32>::default();
    a.try_set("test").unwrap();
    b.try_set("test").unwrap();
    let mut ha = TestHasher(0);
    let mut hb = TestHasher(0);
    a.hash(&mut ha);
    b.hash(&mut hb);
    assert_eq!(ha.finish(), hb.finish());
}

#[test]
fn pod_string_eq_str() {
    let mut s = PodString::<32>::default();
    s.try_set("hello").unwrap();
    assert!(s == *"hello"); // PartialEq<str>
}

#[test]
fn pod_vec_try_push() {
    let mut v = PodVec::<u8, 3>::default();
    assert!(v.try_push(1).is_ok());
    assert!(v.try_push(2).is_ok());
    assert!(v.try_push(3).is_ok());
    assert!(v.try_push(4).is_err()); // full
    assert_eq!(v.as_slice(), &[1, 2, 3]);
}

#[test]
fn pod_vec_try_set_from_slice() {
    let mut v = PodVec::<u8, 3>::default();
    assert!(v.try_set_from_slice(&[1, 2]).is_ok());
    assert_eq!(v.as_slice(), &[1, 2]);
    assert!(v.try_set_from_slice(&[1, 2, 3, 4]).is_err());
    assert_eq!(v.as_slice(), &[1, 2]); // unchanged on error
}

#[test]
fn pod_vec_try_extend() {
    let mut v = PodVec::<u8, 5>::default();
    assert!(v.try_extend_from_slice(&[1, 2]).is_ok());
    assert!(v.try_extend_from_slice(&[3, 4]).is_ok());
    assert!(v.try_extend_from_slice(&[5, 6]).is_err()); // would exceed
    assert_eq!(v.as_slice(), &[1, 2, 3, 4]);
}

#[test]
fn pod_vec_slice_like_setters_are_atomic_on_overflow() {
    let mut values = PodVec::<u8, 3>::default();
    values.try_set([9, 8]).unwrap();

    assert_eq!(values.try_set([1, 2, 3, 4]), Err(PinaPodError::Overflow));
    assert_eq!(values.as_slice(), &[9, 8]);

    assert_eq!(values.try_extend(vec![1, 2]), Err(PinaPodError::Overflow));
    assert_eq!(values.as_slice(), &[9, 8]);
}

#[test]
fn pod_vec_capacity() {
    let v = PodVec::<u8, 10>::default();
    assert_eq!(v.capacity(), 10);
}

#[test]
fn pod_vec_hash() {
    let mut a = PodVec::<u8, 10>::default();
    let mut b = PodVec::<u8, 10>::default();
    let _ = a.try_push(1);
    let _ = a.try_push(2);
    let _ = b.try_push(1);
    let _ = b.try_push(2);
    let mut ha = TestHasher(0);
    let mut hb = TestHasher(0);
    a.hash(&mut ha);
    b.hash(&mut hb);
    assert_eq!(ha.finish(), hb.finish());
}

#[test]
fn pod_option_take() {
    let mut opt = PodOption::<PodU64>::some(PodU64::from(42u64));
    let taken = opt.take();
    assert_eq!(taken, Some(PodU64::from(42u64)));
    assert!(opt.is_none());
}

#[test]
fn pod_option_replace() {
    let mut opt = PodOption::<PodU64>::some(PodU64::from(42u64));
    let old = opt.replace(PodU64::from(99u64));
    assert_eq!(old, Some(PodU64::from(42u64)));
    assert_eq!(opt.get(), Some(PodU64::from(99u64)));
}

#[test]
fn pod_option_clear() {
    let mut opt = PodOption::<PodU64>::some(PodU64::from(42u64));
    opt.clear();
    assert!(opt.is_none());
}

#[test]
fn pod_option_unwrap_or() {
    let some = PodOption::<PodU64>::some(PodU64::from(42u64));
    let none = PodOption::<PodU64>::none();
    assert_eq!(some.unwrap_or(PodU64::from(0u64)), PodU64::from(42u64));
    assert_eq!(none.unwrap_or(PodU64::from(99u64)), PodU64::from(99u64));
}

#[test]
fn pod_option_map_or() {
    let some = PodOption::<PodU64>::some(PodU64::from(10u64));
    let none = PodOption::<PodU64>::none();
    assert_eq!(some.map_or(0u64, |v| v.get() * 2), 20u64);
    assert_eq!(none.map_or(99u64, |v| v.get() * 2), 99u64);
}

#[test]
fn pod_option_partial_eq_option() {
    let a = PodOption::<PodU64>::some(PodU64::from(42u64));
    assert!(a == Some(PodU64::from(42u64)));
    assert!(a != None);
    let b = PodOption::<PodU64>::none();
    assert!(b == None);
}

#[test]
fn zc_elem_bound_compiles() {
    fn assert_zc_elem<T: pinapod::ZcElem>() {}
    assert_zc_elem::<u8>();
    assert_zc_elem::<i8>();
    assert_zc_elem::<PodU64>();
    assert_zc_elem::<PodI32>();
    assert_zc_elem::<PodBool>();
    assert_zc_elem::<PodOption<PodU64>>();
    assert_zc_elem::<[u8; 32]>();
}

#[test]
fn pod_vec_of_pod_option() {
    let mut v = PodVec::<PodOption<PodU64>, 3>::default();
    assert!(v
        .try_push(PodOption::<PodU64>::some(PodU64::from(42u64)))
        .is_ok());
    assert!(v.try_push(PodOption::<PodU64>::none()).is_ok());
    assert_eq!(v.len(), 2);
    assert_eq!(v.as_slice()[0].get(), Some(PodU64::from(42u64)));
    assert!(v.as_slice()[1].is_none());
}
