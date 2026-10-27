//! Fuzzes the wincode deserialization path for pod storage types. Readers
//! must reject invalid stored bytes (bools, tags, lengths, UTF-8) instead of
//! constructing an invalid value, and valid round-trips must preserve the
//! active contents.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pinapod::PinaPodError;
use pinapod::ZcValidate;
use pinapod::pod::PodBool;
use pinapod::pod::PodI64;
use pinapod::pod::PodOption;
use pinapod::pod::PodString;
use pinapod::pod::PodU16;
use pinapod::pod::PodVec;

fuzz_target!(|data: &[u8]| {
	if let Ok(value) = wincode::deserialize::<PodBool>(data) {
		assert!(value.get() == (value.as_ref()[0] != 0));
	}
	if let Ok(value) = wincode::deserialize::<PodU16>(data) {
		assert_eq!(value.get(), u16::from_le_bytes([data[0], data[1]]));
	}
	if let Ok(value) = wincode::deserialize::<PodI64>(data) {
		let mut expected = [0u8; 8];
		expected.copy_from_slice(&data[..8]);
		assert_eq!(value.get(), i64::from_le_bytes(expected));
	}
	if let Ok(value) = wincode::deserialize::<PodString<8>>(data) {
		assert_canonical_wire(data, &value);
		assert!(value.len() <= 8);
		assert!(ZcValidate::validate_ref(&value).is_ok());
		assert_eq!(value.as_bytes(), &data[1..1 + value.len()]);
	}
	if let Ok(value) = wincode::deserialize::<PodVec<PodU16, 4>>(data) {
		assert_canonical_wire(data, &value);
		assert!(value.len() <= 4);
		assert!(ZcValidate::validate_ref(&value).is_ok());
	}
	if let Ok(value) = wincode::deserialize::<PodOption<PodBool>>(data) {
		assert_canonical_wire(data, &value);
		match value.get() {
			Some(_) => assert_eq!(value.raw_tag(), 1),
			None => assert!(value.raw_tag() != 1),
		}
	}

	if let Ok(value) = wincode::deserialize::<PodOption<PodString<4>>>(data) {
		assert_canonical_wire(data, &value);
		// A present payload is only reachable after its nested validation, so
		// both the tag and the nested string's bytes must hold up.
		assert!(ZcValidate::validate_ref(&value).is_ok());
		match value.get() {
			Some(string) => assert!(string.len() <= 4),
			None => {
				assert_ne!(
					value.raw_tag(),
					1,
					"an absent payload cannot carry the Some tag"
				)
			}
		}
	}
	if let Ok(value) = wincode::deserialize::<PodVec<PodBool, 6>>(data) {
		assert_canonical_wire(data, &value);
		// Every accepted element must be in the bool domain, so the whole
		// vector stays valid and in capacity.
		assert!(value.len() <= 6);
		assert!(ZcValidate::validate_ref(&value).is_ok());
	}

	// Wide length prefixes through the wincode reader: the eight-byte prefix
	// decode and the four-byte count share the validate-before-expose
	// boundary with the direct readers.
	if let Ok(value) = wincode::deserialize::<PodString<192, 8>>(data) {
		assert_canonical_wire(data, &value);
		assert!(value.len() <= 192);
		assert!(ZcValidate::validate_ref(&value).is_ok());
	}
	if let Ok(value) = wincode::deserialize::<PodVec<PodU16, 96, 4>>(data) {
		assert_canonical_wire(data, &value);
		assert!(value.len() <= 96);
		assert!(ZcValidate::validate_ref(&value).is_ok());
	}

	// Serialization is canonical: equivalent logical values encode equally,
	// and serialized bytes deserialize back to the same active contents.
	let mut first = PodString::<8>::default();
	first.try_set("hi").map_err(|e: PinaPodError| e).unwrap();
	let mut second = PodString::<8>::default();
	second.try_push_str("h").unwrap();
	second.try_push_str("i").unwrap();

	let size = wincode::serialized_size(&first).unwrap() as usize;
	let mut first_bytes = vec![0u8; size];
	wincode::serialize_into(first_bytes.as_mut_slice(), &first).unwrap();
	let mut second_bytes = vec![0u8; size];
	wincode::serialize_into(second_bytes.as_mut_slice(), &second).unwrap();

	assert_eq!(first_bytes, second_bytes);
	assert_eq!(
		wincode::deserialize::<PodString<8>>(&first_bytes)
			.unwrap()
			.as_str(),
		"hi"
	);
});

/// An accepted container must already have the canonical bytes its writer emits.
fn assert_canonical_wire<T>(data: &[u8], value: &T)
where
	T: wincode::SchemaWrite<wincode::config::DefaultConfig, Src = T>,
{
	let size = wincode::serialized_size(value).unwrap() as usize;
	let mut bytes = vec![0; size];
	wincode::serialize_into(bytes.as_mut_slice(), value).unwrap();
	assert_eq!(bytes, &data[..size]);
}
