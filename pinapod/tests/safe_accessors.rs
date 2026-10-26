//! Safe accessors for unchecked references and CPI-safe volatile reads.
//!
//! `as_str`, `as_slice`, and `get_ref` trust that a reader validated the bytes
//! before the reference was formed. These tests pin the checked counterparts —
//! `try_as_str`, `try_as_slice`, and `try_get_ref` — which reject the same
//! states the reader walk rejects, so a reference cast from raw account bytes
//! has a sound exit. They also pin the volatile integer reads and writes whose
//! purpose is the opposite direction: observing bytes a CPI callee changed
//! after the caller formed its view.

#![allow(
	unsafe_code,
	reason = "forging stored values over initialized byte arrays is the adversarial input these \
	          tests exist for"
)]

use pinapod::PinaPodError;
use pinapod::PodString;
use pinapod::pod::PodBool;
use pinapod::pod::PodI16;
use pinapod::pod::PodI32;
use pinapod::pod::PodI64;
use pinapod::pod::PodI128;
use pinapod::pod::PodU16;
use pinapod::pod::PodU32;
use pinapod::pod::PodU64;
use pinapod::pod::PodU128;

#[test]
fn volatile_access_round_trip_every_integer_pod() {
	let mut u16_value = PodU16::from(0x0102_u16);
	u16_value.set_volatile(0xA55A);
	assert_eq!(u16_value.get_volatile(), 0xA55A);

	let mut u32_value = PodU32::from(0);
	u32_value.set_volatile(0xDEAD_BEEF);
	assert_eq!(u32_value.get_volatile(), 0xDEAD_BEEF);

	let mut u64_value = PodU64::from(0);
	u64_value.set_volatile(0x0102_0304_0506_0708);
	assert_eq!(u64_value.get_volatile(), 0x0102_0304_0506_0708);

	let mut u128_value = PodU128::from(0);
	u128_value.set_volatile(u128::MAX);
	assert_eq!(u128_value.get_volatile(), u128::MAX);

	let mut i16_value = PodI16::from(0);
	i16_value.set_volatile(-300);
	assert_eq!(i16_value.get_volatile(), -300);

	let mut i32_value = PodI32::from(0);
	i32_value.set_volatile(-90_000);
	assert_eq!(i32_value.get_volatile(), -90_000);

	let mut i64_value = PodI64::from(0);
	i64_value.set_volatile(-1);
	assert_eq!(i64_value.get_volatile(), -1);

	let mut i128_value = PodI128::from(0);
	i128_value.set_volatile(i128::MIN + 1);
	assert_eq!(i128_value.get_volatile(), i128::MIN + 1);
}

#[test]
fn volatile_read_observes_bytes_written_through_the_pointer() {
	// The runtime writes updated account data through its own pointer after a
	// CPI call returns; writing through a raw pointer is the closest a host
	// test can get to that. `get_volatile` must observe the new bytes.
	let mut value = PodU64::from(1);
	let byte_pointer = core::ptr::from_mut(&mut value).cast::<u8>();

	// SAFETY: `value` is an initialized local, and the eight written bytes are
	// a valid little-endian `PodU64` pattern for 500.
	unsafe {
		core::ptr::copy_nonoverlapping(500_u64.to_le_bytes().as_ptr(), byte_pointer, 8);
	}

	assert_eq!(value.get_volatile(), 500);
}

#[test]
fn volatile_write_lands_as_the_plain_wire_bytes() {
	let mut value = PodU64::from(0);
	value.set_volatile(0x0102_0304_0506_0708);

	assert_eq!(value.as_ref(), &[8, 7, 6, 5, 4, 3, 2, 1]);
	assert_eq!(value.get(), 0x0102_0304_0506_0708);
}

#[test]
fn try_as_str_accepts_a_valid_string_and_rejects_forged_bytes() {
	let mut value = PodString::<8>::default();
	value.try_set("ifi").unwrap();

	assert_eq!(value.try_as_str().unwrap(), "ifi");

	// A prefix above the capacity is the length state the reader walk rejects.
	let forged_long = [9_u8, b'i', b'f', b'i', 0, 0, 0, 0, 0];
	let long: &PodString<8> = unsafe { &*(forged_long.as_ptr().cast()) };

	assert_eq!(long.try_as_str().unwrap_err(), PinaPodError::InvalidLength);

	// A prefix in range with non-UTF-8 active bytes is the content state the
	// reader walk rejects.
	let forged_utf8 = [3_u8, b'i', b'f', 0xFF, 0, 0, 0, 0, 0];
	let invalid: &PodString<8> = unsafe { &*(forged_utf8.as_ptr().cast()) };

	assert_eq!(invalid.try_as_str().unwrap_err(), PinaPodError::InvalidUtf8);
}

#[test]
fn try_as_slice_accepts_valid_elements_and_rejects_forged_states() {
	use pinapod::pod::PodVecRepr;

	let mut values: PodVecRepr<PodBool, 5, 2> = PodVecRepr::default();
	values.try_push(PodBool::from(true)).unwrap();
	values.try_push(PodBool::from(false)).unwrap();

	let active = values.try_as_slice().unwrap();

	assert_eq!(active.len(), 2);
	assert!(active[0].get());
	assert!(!active[1].get());

	// A count prefix above the capacity is rejected rather than clamped.
	let forged_long = [6_u8, 0, 1, 1, 0, 0, 0];
	let long: &PodVecRepr<PodBool, 5, 2> = unsafe { &*(forged_long.as_ptr().cast()) };

	assert_eq!(
		long.try_as_slice().unwrap_err(),
		PinaPodError::InvalidLength
	);

	// A count in range with an out-of-domain element is rejected by the
	// element walk, exactly as the reader would.
	let forged_bool = [2_u8, 0, 1, 2, 0, 0, 0];
	let invalid: &PodVecRepr<PodBool, 5, 2> = unsafe { &*(forged_bool.as_ptr().cast()) };

	assert_eq!(
		invalid.try_as_slice().unwrap_err(),
		PinaPodError::InvalidBool
	);
}

#[test]
fn try_get_ref_classifies_absent_present_and_corrupt_tags() {
	use pinapod::pod::PodOption;

	let absent = PodOption::<PodBool, 1>::none();

	assert!(absent.try_get_ref().unwrap().is_none());

	let present = PodOption::<PodBool, 1>::some(PodBool::from(true));

	assert!(present.try_get_ref().unwrap().is_some_and(PodBool::get));

	// A tag other than 0 or 1 is a corrupt state, not an absent one.
	let forged_tag = [2_u8, 0];
	let corrupt: &PodOption<PodBool> = unsafe { &*(forged_tag.as_ptr().cast()) };

	assert_eq!(corrupt.try_get_ref().unwrap_err(), PinaPodError::InvalidTag);

	// A present payload holding an out-of-domain byte is rejected by the
	// payload walk.
	let forged_payload = [1_u8, 7];
	let invalid: &PodOption<PodBool> = unsafe { &*(forged_payload.as_ptr().cast()) };

	assert_eq!(
		invalid.try_get_ref().unwrap_err(),
		PinaPodError::InvalidBool
	);
}
