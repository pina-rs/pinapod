#![allow(
	unused_qualifications,
	missing_docs,
	reason = "layout assertions intentionally spell out trait paths and these test fixtures are \
	          not a published surface"
)]

use pinapod::PinaPod;
use pinapod::PinaPodCompact;
use pinapod::pod::*;

#[allow(dead_code)]
#[derive(PinaPod)]
struct Weights {
	pub values: [u64; 3],
	pub flags: [bool; 2],
	pub raw: [[u8; 4]; 2],
	pub spelled: [PodU64; 2],
	pub maybe: Option<[u64; 2]>,
}

#[test]
fn typed_array_sizes_and_alignment() {
	// 3 * 8 + 2 * 1 + 2 * 4 + 2 * 8 + (1 + 2 * 8)
	assert_eq!(Weights::SIZE, 67);
	assert_eq!(
		core::mem::align_of::<<Weights as pinapod::PinaPodFixed>::Zc>(),
		1
	);
}

#[test]
fn typed_array_reads_little_endian_elements() {
	let mut buf = [0u8; Weights::SIZE];
	buf[0..8].copy_from_slice(&0x0102_0304_0506_0708_u64.to_le_bytes());
	buf[8..16].copy_from_slice(&0x1111_1111_1111_1111_u64.to_le_bytes());
	buf[16..24].copy_from_slice(&42_u64.to_le_bytes());

	let zc = Weights::read_exact(&buf).unwrap();
	let values: &[PodU64; 3] = zc.values();
	assert_eq!(values[0].get(), 0x0102_0304_0506_0708);
	assert_eq!(values[1].get(), 0x1111_1111_1111_1111);
	assert_eq!(values[2].get(), 42);

	let raw: &[[u8; 4]; 2] = zc.raw();
	assert_eq!(raw[0], [0; 4]);
}

#[test]
fn typed_array_writes_elements_in_place() {
	let mut buf = [0u8; Weights::SIZE];
	{
		let zc = Weights::read_exact_mut(&mut buf).unwrap();

		zc.values = [PodU64::from(1), PodU64::from(2), PodU64::from(3)];
		zc.flags[1] = true.into();
		zc.spelled = [PodU64::from(9), PodU64::from(10)];
		zc.maybe = PodOption::some([PodU64::from(7), PodU64::from(8)]);
	}

	assert_eq!(&buf[0..8], &1_u64.to_le_bytes());
	assert_eq!(&buf[8..16], &2_u64.to_le_bytes());
	assert_eq!(&buf[16..24], &3_u64.to_le_bytes());
	assert_eq!(buf[25], 1, "flags[1] stores the canonical true byte");

	let zc = Weights::read_exact(&buf).unwrap();
	assert_eq!(zc.values()[1].get(), 2);
	assert_eq!(zc.spelled()[0].get(), 9);
	assert_eq!(
		zc.maybe.get_ref().map(|values| values[1].get()),
		Some(8),
		"Option<[u64; N]> resolves its payload through the array pod"
	);
}

#[test]
fn typed_array_accepts_pod_spelled_values() {
	let mut buf = [0u8; Weights::SIZE];
	let zc = Weights::read_exact_mut(&mut buf).unwrap();

	// The field itself is stored as the pod array, so pod-spelled values
	// assign directly; the generated patch builders additionally accept both
	// spellings through `IntoPodArray` (see the compact tests below).
	zc.values = [PodU64::from(4), PodU64::from(5), PodU64::from(6)];

	assert_eq!(zc.values()[0].get(), 4);
	assert_eq!(zc.values()[2].get(), 6);
}

#[test]
fn typed_array_validation_rejects_invalid_bool_element() {
	let mut buf = [0u8; Weights::SIZE];
	buf[24] = 1; // flags[0] = true
	buf[25] = 2; // flags[1] is not a canonical bool byte

	assert!(Weights::read_exact(&buf).is_err());
}

#[test]
fn typed_array_validation_rejects_invalid_option_payload() {
	let mut buf = [0u8; Weights::SIZE];
	// `maybe` starts at offset 50: a tag of 2 is not a canonical option tag.
	buf[50] = 2;

	assert!(Weights::read_exact(&buf).is_err());
}

#[test]
fn typed_array_keeps_the_identity_mapping_for_byte_arrays() {
	// This function only compiles when `<[u8; 4] as ZcField>::Pod` is exactly
	// `[u8; 4]`, pinning the generalized array mapping for the byte case.
	const fn assert_identity(value: [u8; 4]) -> <[u8; 4] as pinapod::ZcField>::Pod {
		value
	}

	assert_eq!(assert_identity([1, 2, 3, 4]), [1, 2, 3, 4]);
}

#[test]
fn typed_array_maps_native_arrays_to_pod_arrays() {
	// Compiles only when `<[u64; 2] as ZcField>::Pod` is exactly
	// `[PodU64; 2]`.
	const fn assert_mapping(value: [PodU64; 2]) -> <[u64; 2] as pinapod::ZcField>::Pod {
		value
	}

	assert_eq!(
		assert_mapping([PodU64::from(1), PodU64::from(2)])[1].get(),
		2
	);
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct Table {
	pub weights: [u64; 2],
	pub label: pinapod::String<8>,
}

#[test]
fn compact_typed_array_header_layout() {
	// Header: 2 * 8 weights + 1 label length prefix.
	assert_eq!(<Table as PinaPodCompact>::HEADER_SIZE, 17);
	assert_eq!(<Table as PinaPodCompact>::MIN_SIZE, 17);
	assert_eq!(<Table as PinaPodCompact>::MAX_SIZE, 17 + 8);
	assert_eq!(
		core::mem::align_of::<<Table as PinaPodCompact>::Header>(),
		1
	);
}

#[test]
fn compact_patch_accepts_native_and_pod_spelled_arrays() {
	let mut buf = [0u8; Table::MAX_SIZE];

	let patch = TablePatch::new().weights([5_u64, 6]).label("hi");
	let encoded_len = Table::initialize(&mut buf, &patch).unwrap();
	let table = Table::read_prefix(&buf[..encoded_len]).unwrap();

	assert_eq!(table.weights[0].get(), 5);
	assert_eq!(table.weights[1].get(), 6);
	assert_eq!(table.label(), "hi");

	let patch = TablePatch::new().weights([PodU64::from(7), PodU64::from(8)]);
	Table::update(&mut buf, &patch).unwrap();
	let table = Table::read_prefix(&buf).unwrap();

	assert_eq!(table.weights[0].get(), 7);
	assert_eq!(table.weights[1].get(), 8);
}

#[test]
fn compact_patch_validates_array_elements() {
	let buf = &mut [0u8; Table::MAX_SIZE];

	// `u64::MAX` values are valid; the label must fit its capacity instead.
	let oversized = TablePatch::new().weights([u64::MAX, 1]).label("123456789");
	assert!(Table::initialize(buf, &oversized).is_err());
}

/// A restricted-domain element: only `0` and `1` are valid.
///
/// This stands in for `PodBool`, whose array walk must never be skipped.
#[derive(Copy, Clone)]
struct RestrictedByte(u8);

impl pinapod::ZcValidate for RestrictedByte {
	fn validate_ref(value: &Self) -> Result<(), pinapod::PinaPodError> {
		if value.0 > 1 {
			return Err(pinapod::PinaPodError::InvalidBool);
		}

		Ok(())
	}
}

#[test]
fn byte_arrays_accept_every_bit_pattern() {
	// The whole point of the `[u8; N]` fast path: byte arrays carry no
	// restricted domain, so no byte value can make them invalid.
	let bytes = [0xFF_u8; 4];
	pinapod::ZcValidate::validate_ref(&bytes).unwrap();

	let nested = [[0xFF_u8; 4]; 2];
	pinapod::ZcValidate::validate_ref(&nested).unwrap();
}

#[test]
fn array_validation_still_walks_restricted_elements() {
	// A trivially valid element must not make a restricted element's array
	// skip its per-element gate.
	let valid = [RestrictedByte(0), RestrictedByte(1)];
	pinapod::ZcValidate::validate_ref(&valid).unwrap();

	let invalid = [RestrictedByte(0), RestrictedByte(2)];
	assert!(
		pinapod::ZcValidate::validate_ref(&invalid).is_err(),
		"a non-canonical element must still be rejected"
	);
}

#[test]
fn array_validation_walks_restricted_elements_inside_schemas() {
	// `Weights::flags` is `[bool; 2]`, stored as `[PodBool; 2]`. The invalid
	// byte sits in the second element, so only a real per-element walk finds it.
	let mut buf = [0u8; Weights::SIZE];
	buf[24] = 1;
	buf[25] = 2;

	assert!(Weights::read_exact(&buf).is_err());
}
