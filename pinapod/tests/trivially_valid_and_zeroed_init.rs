//! Adversarial and contract tests for `ZcValidate::TRIVIALLY_VALID` and
//! `PinaPodFixed::initialize_zeroed`.
#![allow(dead_code, missing_docs)]

use pinapod::PinaPod;
use pinapod::PinaPodError;
use pinapod::PinaPodFixed;
use pinapod::ZcValidate;
use pinapod::pod::PodBool;
use pinapod::pod::PodI16;
use pinapod::pod::PodI32;
use pinapod::pod::PodI64;
use pinapod::pod::PodI128;
use pinapod::pod::PodOption;
use pinapod::pod::PodString;
use pinapod::pod::PodU16;
use pinapod::pod::PodU32;
use pinapod::pod::PodU64;
use pinapod::pod::PodU128;
use pinapod::pod::PodVecRepr;

#[derive(PinaPod)]
struct AllPodLeaf {
	tag: u8,
	signed: i8,
	amount: u64,
	wide: u128,
	words: [u32; 3],
}

#[derive(PinaPod)]
struct AllPodComposite {
	leaf: AllPodLeaf,
	entries: [AllPodLeaf; 4],
}

#[derive(PinaPod)]
#[repr(u8)]
enum NonZeroStatus {
	Active = 1,
	Paused = 2,
}

#[derive(PinaPod)]
struct ContainsBool {
	id: u64,
	active: bool,
}

#[derive(PinaPod)]
struct DeepNestedBool {
	header: AllPodLeaf,
	inner: [ContainsBool; 3],
}

#[derive(PinaPod)]
struct ContainsEnum {
	amount: u64,
	status: NonZeroStatus,
}

#[derive(PinaPod)]
struct ContainsOption {
	value: Option<u64>,
}

#[derive(PinaPod)]
struct ContainsString {
	label: pinapod::String<8>,
}

#[derive(PinaPod)]
struct ContainsPodVec {
	items: pinapod::Vec<AllPodLeaf, 4>,
}

#[derive(PinaPod)]
struct ContainsRestrictedPodVec {
	items: pinapod::Vec<ContainsBool, 4>,
}

#[derive(PinaPod)]
struct ContainsEnumPodVec {
	items: pinapod::Vec<ContainsEnum, 4>,
}

#[derive(PinaPod)]
#[pinapod(compact)]
struct CompactTrivialHeader {
	pub owner_id: u64,
	pub entries: pinapod::Vec<AllPodLeaf, 4>,
}

#[derive(PinaPod)]
#[pinapod(compact)]
struct CompactRestrictedHeader {
	enabled: bool,
	entries: pinapod::Vec<ContainsBool, 4>,
}

// Compile-time proofs: every trivially valid primitive, array, and all-POD
// composite reports `TRIVIALLY_VALID == true`.
const _: () = {
	assert!(<u8 as ZcValidate>::TRIVIALLY_VALID);
	assert!(<i8 as ZcValidate>::TRIVIALLY_VALID);
	assert!(<PodU16 as ZcValidate>::TRIVIALLY_VALID);
	assert!(<PodU32 as ZcValidate>::TRIVIALLY_VALID);
	assert!(<PodU64 as ZcValidate>::TRIVIALLY_VALID);
	assert!(<PodU128 as ZcValidate>::TRIVIALLY_VALID);
	assert!(<PodI16 as ZcValidate>::TRIVIALLY_VALID);
	assert!(<PodI32 as ZcValidate>::TRIVIALLY_VALID);
	assert!(<PodI64 as ZcValidate>::TRIVIALLY_VALID);
	assert!(<PodI128 as ZcValidate>::TRIVIALLY_VALID);
	assert!(<[[PodU64; 2]; 3] as ZcValidate>::TRIVIALLY_VALID);
	assert!(<AllPodLeafZc as ZcValidate>::TRIVIALLY_VALID);
	assert!(<AllPodCompositeZc as ZcValidate>::TRIVIALLY_VALID);
	assert!(<[AllPodCompositeZc; 2] as ZcValidate>::TRIVIALLY_VALID);
	assert!(
		<<CompactTrivialHeader as pinapod::PinaPodCompact>::Header as ZcValidate>::TRIVIALLY_VALID
	);
};

// Compile-time proofs: EVERY restricted-domain type and EVERY struct/array
// containing one at any nesting depth MUST report `TRIVIALLY_VALID == false`.
const _: () = {
	assert!(!<PodBool as ZcValidate>::TRIVIALLY_VALID);
	assert!(!<PodString<8, 1> as ZcValidate>::TRIVIALLY_VALID);
	assert!(!<PodVecRepr<u8, 8, 1> as ZcValidate>::TRIVIALLY_VALID);
	assert!(!<PodOption<PodU64, 1> as ZcValidate>::TRIVIALLY_VALID);
	assert!(!<NonZeroStatusZc as ZcValidate>::TRIVIALLY_VALID);
	assert!(!<ContainsBoolZc as ZcValidate>::TRIVIALLY_VALID);
	assert!(!<DeepNestedBoolZc as ZcValidate>::TRIVIALLY_VALID);
	assert!(!<[DeepNestedBoolZc; 2] as ZcValidate>::TRIVIALLY_VALID);
	assert!(!<ContainsEnumZc as ZcValidate>::TRIVIALLY_VALID);
	assert!(!<ContainsOptionZc as ZcValidate>::TRIVIALLY_VALID);
	assert!(!<ContainsStringZc as ZcValidate>::TRIVIALLY_VALID);
	assert!(!<ContainsPodVecZc as ZcValidate>::TRIVIALLY_VALID);
	assert!(!<ContainsRestrictedPodVecZc as ZcValidate>::TRIVIALLY_VALID);
	assert!(!<ContainsEnumPodVecZc as ZcValidate>::TRIVIALLY_VALID);
	type RestrictedHeader = <CompactRestrictedHeader as pinapod::PinaPodCompact>::Header;
	assert!(!<RestrictedHeader as ZcValidate>::TRIVIALLY_VALID);
};

#[cfg(feature = "floats")]
const _: () = {
	assert!(<pinapod::pod::PodF32 as ZcValidate>::TRIVIALLY_VALID);
	assert!(<pinapod::pod::PodF64 as ZcValidate>::TRIVIALLY_VALID);
};

#[cfg(feature = "solana-address")]
const _: () = {
	assert!(<solana_address::Address as ZcValidate>::TRIVIALLY_VALID);
};

#[test]
fn all_pod_arrays_and_slices_accept_arbitrary_bit_patterns() {
	let bytes = [0xFFu8; AllPodComposite::SIZE];
	let composite = AllPodComposite::read_exact(&bytes)
		.unwrap_or_else(|error| panic!("all-POD composite should accept any bytes: {error:?}"));
	assert_eq!(composite.leaf.tag, 0xFF);
	assert!(<AllPodLeafZc as ZcValidate>::validate_array(&composite.entries).is_ok());
	assert!(<AllPodLeafZc as ZcValidate>::validate_slice(&composite.entries).is_ok());
}

#[test]
fn adversarial_corrupt_bool_in_deep_nested_array_is_rejected() {
	let mut bytes = [0u8; DeepNestedBool::SIZE];
	assert!(DeepNestedBool::read_exact(&bytes).is_ok());

	// Corrupt the `active` bool byte inside the last element of `inner`.
	let last_bool_offset = DeepNestedBool::SIZE - 1;
	for bad in [2u8, 3, 127, 255] {
		bytes[last_bool_offset] = bad;
		assert_eq!(
			DeepNestedBool::read_exact(&bytes).unwrap_err(),
			PinaPodError::InvalidBool
		);
	}
}

#[test]
fn adversarial_corrupt_element_in_pod_vec_is_rejected() {
	let mut bytes = [0u8; ContainsRestrictedPodVec::SIZE];
	// Length prefix = 3 active elements (little-endian).
	bytes[0] = 3;
	assert!(ContainsRestrictedPodVec::read_exact(&bytes).is_ok());

	// Corrupt element index 2's `active` bool byte.
	let elem_size = size_of::<ContainsBoolZc>();
	let prefix_len = ContainsRestrictedPodVec::SIZE - 4 * elem_size;
	let bad_offset = prefix_len + 2 * elem_size + (elem_size - 1);
	bytes[bad_offset] = 2;
	assert_eq!(
		ContainsRestrictedPodVec::read_exact(&bytes).unwrap_err(),
		PinaPodError::InvalidBool
	);
}

#[test]
fn adversarial_corrupt_enum_in_pod_vec_is_rejected() {
	let mut bytes = [0u8; ContainsEnumPodVec::SIZE];
	bytes[0] = 2;
	let elem_size = size_of::<ContainsEnumZc>();
	let prefix_len = ContainsEnumPodVec::SIZE - 4 * elem_size;
	// Set element 0 valid (status = 1), leave element 1 invalid (status = 0).
	bytes[prefix_len + elem_size - 1] = 1;
	assert_eq!(
		ContainsEnumPodVec::read_exact(&bytes).unwrap_err(),
		PinaPodError::InvalidDiscriminant
	);

	// Set element 1 to valid (status = 2).
	bytes[prefix_len + 2 * elem_size - 1] = 2;
	assert!(ContainsEnumPodVec::read_exact(&bytes).is_ok());

	// Corrupt element 1 to unknown discriminant 99.
	bytes[prefix_len + 2 * elem_size - 1] = 99;
	assert_eq!(
		ContainsEnumPodVec::read_exact(&bytes).unwrap_err(),
		PinaPodError::InvalidDiscriminant
	);
}

#[test]
fn adversarial_corrupt_option_and_string_in_structs_are_rejected() {
	let mut opt_bytes = [0u8; ContainsOption::SIZE];
	opt_bytes[0] = 2;
	assert_eq!(
		ContainsOption::read_exact(&opt_bytes).unwrap_err(),
		PinaPodError::InvalidTag
	);

	let mut str_bytes = [0u8; ContainsString::SIZE];
	str_bytes[0] = 2;
	str_bytes[1] = 0xFF;
	str_bytes[2] = 0xFE;
	assert_eq!(
		ContainsString::read_exact(&str_bytes).unwrap_err(),
		PinaPodError::InvalidUtf8
	);
}

#[test]
fn initialize_zeroed_validates_lengths_and_preserves_zeroing_on_failure() {
	let mut short = [0xAAu8; ContainsEnum::SIZE - 1];
	assert_eq!(
		<ContainsEnum as PinaPodFixed>::initialize_zeroed(&mut short, |_| Ok(())).unwrap_err(),
		PinaPodError::BufferTooSmall
	);
	assert!(short.iter().all(|&b| b == 0xAA));

	let mut long = [0xBBu8; ContainsEnum::SIZE + 1];
	assert_eq!(
		<ContainsEnum as PinaPodFixed>::initialize_zeroed(&mut long, |_| Ok(())).unwrap_err(),
		PinaPodError::InvalidLength
	);
	assert!(long.iter().all(|&b| b == 0xBB));

	// Failing closure after writing sensitive bytes MUST zero the whole buffer.
	let mut buf = [0u8; ContainsEnum::SIZE];
	let err = <ContainsEnum as PinaPodFixed>::initialize_zeroed(&mut buf, |zc| {
		zc.amount.set(0xDEAD_BEEF_CAFE_BABE);
		zc.status = NonZeroStatus::Active.into();
		Err(PinaPodError::InvalidLength)
	})
	.unwrap_err();
	assert_eq!(err, PinaPodError::InvalidLength);
	assert!(buf.iter().all(|&b| b == 0));

	// Leaving a non-zero-valid enum unset fails validation AND zeroes all written fields.
	let err = <ContainsEnum as PinaPodFixed>::initialize_zeroed(&mut buf, |zc| {
		zc.amount.set(42);
		Ok(())
	})
	.unwrap_err();
	assert_eq!(err, PinaPodError::InvalidDiscriminant);
	assert!(buf.iter().all(|&b| b == 0));

	// Adversarial dirty buffer on restricted schema: untouched invalid byte MUST
	// still fail validation and wipe the entire buffer to zero.
	let mut dirty = [0x7Fu8; ContainsBool::SIZE];
	let err = <ContainsBool as PinaPodFixed>::initialize_zeroed(&mut dirty, |zc| {
		zc.id.set(7);
		Ok(())
	})
	.unwrap_err();
	assert_eq!(err, PinaPodError::InvalidBool);
	assert!(dirty.iter().all(|&b| b == 0));

	// Valid initialization on both restricted and trivially valid types succeeds.
	let mut valid = [0u8; ContainsEnum::SIZE];
	let view = <ContainsEnum as PinaPodFixed>::initialize_zeroed(&mut valid, |zc| {
		zc.amount.set(123);
		zc.status = NonZeroStatus::Paused.into();
		Ok(())
	})
	.unwrap_or_else(|error| panic!("valid initialize_zeroed failed: {error:?}"));
	assert_eq!(view.amount(), 123);

	let mut leaf_buf = [0u8; AllPodLeaf::SIZE];
	let leaf = <AllPodLeaf as PinaPodFixed>::initialize_zeroed(&mut leaf_buf, |zc| {
		zc.tag = 9;
		zc.amount.set(999);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("valid leaf initialize_zeroed failed: {error:?}"));
	assert_eq!(leaf.tag, 9);
	assert_eq!(leaf.amount(), 999);
}

#[test]
fn initialize_rejects_short_and_long_buffers() {
	let mut short = [0u8; AllPodLeaf::SIZE - 1];
	assert_eq!(
		<AllPodLeaf as PinaPodFixed>::initialize(&mut short, |_| Ok(())).unwrap_err(),
		PinaPodError::BufferTooSmall
	);

	let mut long = [0u8; AllPodLeaf::SIZE + 1];
	assert_eq!(
		<AllPodLeaf as PinaPodFixed>::initialize(&mut long, |_| Ok(())).unwrap_err(),
		PinaPodError::InvalidLength
	);
}

#[derive(PinaPod)]
#[pinapod(compact)]
#[repr(u8)]
enum CompactMessage {
	Empty = 0,
	Values(pinapod::Vec<u16, 4>) = 1,
}

#[test]
fn compact_struct_and_enum_single_pass_update_and_initialize_zeroed() {
	use pinapod::PinaPodPatch;

	// Compact struct: initialize_zeroed matches initialize and zeroes on failure.
	let leaf = AllPodLeafZc {
		tag: 7,
		signed: -2,
		amount: PodU64::from(42),
		wide: PodU128::from(99),
		words: [PodU32::from(1), PodU32::from(2), PodU32::from(3)],
	};
	let one_leaf = [leaf];
	let init_patch = CompactTrivialHeaderPatch::new()
		.owner_id(11u64)
		.replace_entries(&one_leaf);

	let size_one = CompactTrivialHeader::HEADER_SIZE + AllPodLeaf::SIZE;
	let mut a = vec![0u8; size_one];
	let mut b = vec![0u8; size_one];
	let len_a = init_patch.initialize(&mut a).unwrap();
	let len_b = init_patch.initialize_zeroed(&mut b).unwrap();
	assert_eq!(len_a, size_one);
	assert_eq!(len_b, size_one);
	assert_eq!(a, b);

	// Failure in initialize_zeroed wipes buffer cleanly.
	let mut too_small = vec![0xAAu8; CompactTrivialHeader::HEADER_SIZE];
	assert_eq!(
		init_patch.initialize_zeroed(&mut too_small).unwrap_err(),
		PinaPodError::BufferTooSmall
	);
	assert!(too_small.iter().all(|&byte| byte == 0));

	// Grow needed: try_update_or_len returns Ok(Err(required_len)) without mutating `b`.
	let two_leaves = [leaf, leaf];
	let grow_patch = CompactTrivialHeaderPatch::new()
		.owner_id(22u64)
		.replace_entries(&two_leaves);
	let snapshot = b.clone();
	let needed = CompactTrivialHeader::HEADER_SIZE + 2 * AllPodLeaf::SIZE;
	assert_eq!(
		PinaPodPatch::<CompactTrivialHeader>::try_update_or_len(&&grow_patch, &mut b),
		Ok(Err(needed))
	);
	assert_eq!(b, snapshot);
	assert_eq!(grow_patch.update(&mut b), Err(PinaPodError::BufferTooSmall));

	// Resize then apply in place, then shrink in one pass.
	b.resize(needed, 0);
	assert_eq!(grow_patch.try_update_or_len(&mut b), Ok(Ok(needed)));
	let shrink_patch = CompactTrivialHeaderPatch::new()
		.owner_id(33u64)
		.replace_entries(&[]);
	assert_eq!(
		shrink_patch.try_update_or_len(&mut b),
		Ok(Ok(CompactTrivialHeader::HEADER_SIZE))
	);
	let view = CompactTrivialHeader::read_prefix(&b).unwrap();
	assert_eq!(view.owner_id.get(), 33);
	assert_eq!(view.entries(), []);

	// Blanket &P forwarding for initialize_zeroed.
	let mut c = vec![0u8; size_one];
	assert_eq!(
		PinaPodPatch::<CompactTrivialHeader>::initialize_zeroed(&&init_patch, &mut c),
		Ok(size_one)
	);
	assert_eq!(c, a);

	// Compact enum: initialize_zeroed, grow-needed Ok(Err), in-place shrink Ok(Ok), rollback.
	let empty_patch = CompactMessagePatch::Empty;
	let mut ebuf = vec![0u8; CompactMessage::MIN_SIZE];
	assert_eq!(
		PinaPodPatch::<CompactMessage>::initialize_zeroed(&&empty_patch, &mut ebuf),
		Ok(CompactMessage::MIN_SIZE)
	);

	let vals = [PodU16::from(10), PodU16::from(20)];
	let vals_patch = CompactMessagePatch::Values(&vals);
	let vals_needed = 1 + 2 + 4; // tag(1) + len(2) + 2*u16(4)
	let esnap = ebuf.clone();
	assert_eq!(
		PinaPodPatch::<CompactMessage>::try_update_or_len(&&vals_patch, &mut ebuf),
		Ok(Err(vals_needed))
	);
	assert_eq!(ebuf, esnap);
	assert_eq!(
		vals_patch.update(&mut ebuf),
		Err(PinaPodError::BufferTooSmall)
	);

	// Failure in enum initialize_zeroed zeroes destination.
	let mut bad_ebuf = vec![0xFFu8; CompactMessage::MIN_SIZE];
	assert_eq!(
		vals_patch.initialize_zeroed(&mut bad_ebuf),
		Err(PinaPodError::BufferTooSmall)
	);
	assert!(bad_ebuf.iter().all(|&byte| byte == 0));

	ebuf.resize(vals_needed, 0);
	assert_eq!(vals_patch.try_update_or_len(&mut ebuf), Ok(Ok(vals_needed)));
	assert_eq!(
		empty_patch.try_update_or_len(&mut ebuf),
		Ok(Ok(CompactMessage::MIN_SIZE))
	);
	// Trailing bytes past shrunk enum variant are cleared.
	assert!(
		ebuf[CompactMessage::MIN_SIZE..]
			.iter()
			.all(|&byte| byte == 0)
	);

	// Custom PinaPodPatch exercising the trait's default try_update_or_len & initialize_zeroed.
	struct ManualWrapper<'a>(&'a CompactTrivialHeaderPatch<'a>);
	impl PinaPodPatch<CompactTrivialHeader> for ManualWrapper<'_> {
		fn updated_len(&self, data: &[u8]) -> Result<usize, PinaPodError> {
			self.0.updated_len(data)
		}

		fn update(&self, data: &mut [u8]) -> Result<usize, PinaPodError> {
			self.0.update(data)
		}

		fn initialize(&self, data: &mut [u8]) -> Result<usize, PinaPodError> {
			self.0.initialize(data)
		}
	}
	let mut d = vec![0u8; size_one];
	assert_eq!(
		ManualWrapper(&init_patch).initialize_zeroed(&mut d),
		Ok(size_one)
	);
	assert_eq!(
		ManualWrapper(&grow_patch).try_update_or_len(&mut d),
		Ok(Err(needed))
	);
	assert_eq!(
		ManualWrapper(&shrink_patch).try_update_or_len(&mut d),
		Ok(Ok(CompactTrivialHeader::HEADER_SIZE))
	);
}
