//! Companion-surface tests: value traits on generated representations and
//! materialization of owned native values from views.
//!
//! Fixed companions and compact views gain `PartialEq`, `Eq`, `Hash`, and
//! `Debug` over active content, plus `to_native`, which builds a fully owned
//! value. These tests pin that equality follows the value rather than the raw
//! bytes, that hashing and rendering agree with it, and that materialization
//! round-trips every field kind — including the enum field whose stored
//! discriminant is the one fallible case.

#![allow(
	dead_code,
	missing_docs,
	reason = "schemas and their generated surface exist to be exercised here, not published"
)]
#![allow(
	unsafe_code,
	reason = "forging a stored enum discriminant over initialized bytes is the adversarial input 	          the fallible materialization case exists for"
)]

use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use pinapod::PinaPod;
use pinapod::PinaPodCompact;
use pinapod::PodString;
use pinapod::PodVec;
use pinapod::pod::PodU16;

#[derive(Debug, PartialEq, Eq, PinaPod)]
#[repr(u8)]
enum Status {
	Active = 1,
	Retired = 2,
}

#[derive(Debug, PartialEq, PinaPod)]
struct Inner {
	marker: u32,
}

#[derive(Debug, PartialEq)]
struct Snapshot {
	authority: [u8; 4],
	amount: u64,
	rate: f32,
	label: PodString<32>,
	tags: PodVec<u16, 8>,
	note: Option<PodString<8>>,
	state: Status,
	inner: Inner,
	count: i128,
	small: u8,
}

#[derive(Debug, PartialEq, PinaPod)]
struct Profile {
	authority: [u8; 4],
	amount: u64,
	rate: f32,
	label: pinapod::String<32>,
	tags: pinapod::Vec<u16, 8>,
	note: Option<pinapod::String<8>>,
	state: Status,
	inner: Inner,
	count: i128,
	small: u8,
	flag: pinapod::pod::PodBool,
}

#[allow(dead_code)]
#[derive(PinaPod)]
struct Hidden {
	visible: u64,
	#[pinapod(skip_accessor)]
	discriminator: [u8; 8],
}

#[allow(dead_code)]
#[derive(PinaPod)]
struct GenericBox<T, const WIDTH: usize>
where
	T: pinapod::ZcField,
	<T as pinapod::ZcField>::Pod: pinapod::ZcElem,
{
	value: T,
	padding: [u8; WIDTH],
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct Journal {
	sequence: u64,
	label: pinapod::String<64>,
	entries: pinapod::Vec<u64, 16>,
	note: Option<pinapod::String<32>>,
	flags: Option<pinapod::Vec<u32, 8>>,
}

fn encode_profile(amount: u64) -> Vec<u8> {
	let mut data = vec![0_u8; Profile::SIZE];

	Profile::initialize(&mut data, |profile| {
		profile.authority = [9, 8, 7, 6];
		profile.amount = amount.into();
		profile.rate.set(1.5);
		profile.label.try_set("ifi")?;
		profile.tags.try_push(7_u16)?;
		profile.tags.try_push(11_u16)?;
		profile.note.set(Some(PodString::<8>::try_from("hi")?));
		profile.state = Status::Active.into();
		profile.inner.marker = 44_u32.into();
		profile.count = (-5_i128).into();
		profile.small = 3;
		profile.flag = pinapod::pod::PodBool::from(true);
		Ok(())
	})
	.unwrap_or_else(|error| panic!("profile fixture must initialize: {error}"));

	data
}

#[test]
fn fixed_to_native_materializes_every_field_kind() {
	let data = encode_profile(1_000);
	let zc =
		Profile::read_exact(&data).unwrap_or_else(|error| panic!("encoding is valid: {error}"));
	let native = zc
		.to_native()
		.unwrap_or_else(|error| panic!("validated value materializes: {error}"));

	assert_eq!(native.authority, [9, 8, 7, 6]);
	assert_eq!(native.amount, 1_000);
	assert_eq!(native.rate, 1.5);
	assert_eq!(native.label.as_str(), "ifi");
	assert_eq!(
		native.tags.as_slice(),
		&[PodU16::from(7_u16), PodU16::from(11_u16)]
	);
	assert_eq!(native.note.as_ref().map(PodString::as_str), Some("hi"));
	assert_eq!(native.state, Status::Active);
	assert_eq!(native.inner.marker, 44);
	assert_eq!(native.count, -5);
	assert_eq!(native.small, 3);
	assert_eq!(native.flag, pinapod::pod::PodBool::from(true));

	// Materialization is pure: repeating it yields the same value.
	let again = zc
		.to_native()
		.unwrap_or_else(|error| panic!("materialization repeats: {error}"));
	assert_eq!(native, again);
}

#[test]
fn fixed_zc_value_traits_follow_active_content() {
	let first_data = encode_profile(1_000);
	let second_data = encode_profile(1_000);
	let different_data = encode_profile(2_000);

	let first = Profile::read_exact(&first_data).unwrap();
	let second = Profile::read_exact(&second_data).unwrap();
	let different = Profile::read_exact(&different_data).unwrap();

	// The gap this closes: assert_eq between two representations compiles.
	assert_eq!(first, second);
	assert_ne!(first, different);

	let first_hash = {
		let mut hasher = DefaultHasher::new();
		first.hash(&mut hasher);
		hasher.finish()
	};

	let second_hash = {
		let mut hasher = DefaultHasher::new();
		second.hash(&mut hasher);
		hasher.finish()
	};

	assert_eq!(first_hash, second_hash);

	let rendered = format!("{first:?}");
	assert!(rendered.contains("Profile"));
	assert!(rendered.contains("amount"));
	assert!(rendered.contains("label"));
}

#[test]
fn enum_materialization_is_the_one_fallible_case() {
	// A stored discriminant naming no variant can only reach `FromZc` through
	// an unchecked reference; validation would have rejected it first.
	let bytes = [9_u8];
	let zc: &StatusZc = unsafe { &*(bytes.as_ptr().cast()) };

	let error = <Status as pinapod::FromZc>::from_zc(zc).unwrap_err();

	assert_eq!(error, pinapod::PinaPodError::InvalidDiscriminant);
}

#[test]
fn skip_accessor_fields_still_materialize() {
	let mut data = vec![0_u8; Hidden::SIZE];

	Hidden::initialize(&mut data, |hidden| {
		hidden.visible = 7_u64.into();
		hidden.discriminator = *b"ACCOUNT1";
		Ok(())
	})
	.unwrap();

	let zc = Hidden::read_exact(&data).unwrap();
	let native = zc.to_native().unwrap();

	assert_eq!(native.visible, 7);
	assert_eq!(native.discriminator, *b"ACCOUNT1");
}

#[test]
fn generic_schemas_keep_the_value_traits() {
	let mut data = vec![0_u8; GenericBox::<u16, 4>::SIZE];

	GenericBox::<u16, 4>::initialize(&mut data, |value| {
		value.value = 21_u16.into();
		value.padding = [1, 2, 3, 4];
		Ok(())
	})
	.unwrap();

	let first = GenericBox::<u16, 4>::read_exact(&data).unwrap();
	let second = GenericBox::<u16, 4>::read_exact(&data).unwrap();

	assert_eq!(first, second);
	assert_eq!(format!("{first:?}"), format!("{second:?}"));

	let native = first.to_native().unwrap();

	assert_eq!(native.value, 21);
}

#[test]
fn fixed_point_fields_materialize_through_the_pinned_mapping() {
	use pinapod::fixed::types::I16F16;

	#[derive(PinaPod)]
	struct Priced {
		amount: I16F16,
	}

	let mut data = vec![0_u8; Priced::SIZE];

	Priced::initialize(&mut data, |priced| {
		priced.amount.set(3 << 16);
		Ok(())
	})
	.unwrap();

	let native = Priced::read_exact(&data).unwrap().to_native().unwrap();

	assert_eq!(native.amount, I16F16::from_bits(3 << 16));
}

fn encode_journal(label: &str, note: Option<&str>) -> Vec<u8> {
	let mut data = vec![0_u8; <Journal as PinaPodCompact>::MAX_SIZE];
	let entries: [pinapod::pod::PodU64; 16] = core::array::from_fn(|index| (index as u64).into());
	let flags: [pinapod::pod::PodU32; 8] = core::array::from_fn(|index| (index as u32).into());

	let patch = JournalPatch::new()
		.sequence(42_u64)
		.label(label)
		.replace_entries(&entries[..3])
		.note(note)
		.replace_flags(Some(&flags[..2]));

	let used = Journal::initialize(&mut data, &patch)
		.unwrap_or_else(|error| panic!("journal fixture must initialize: {error}"));

	data.truncate(used);
	data
}

#[test]
fn compact_to_native_materializes_inline_and_tail_fields() {
	let data = encode_journal("journal", Some("note"));
	let view = Journal::read_prefix(&data).unwrap();
	let native = view
		.to_native()
		.unwrap_or_else(|error| panic!("validated view materializes: {error}"));

	assert_eq!(native.sequence, 42);
	assert_eq!(native.label.as_str(), "journal");
	assert_eq!(native.entries.len(), 3);
	assert_eq!(native.entries[2].get(), 2);
	assert_eq!(native.note.as_ref().map(PodString::as_str), Some("note"));
	assert_eq!(
		native.flags.as_ref().map(pinapod::pod::PodVecRepr::len),
		Some(2)
	);
}

#[test]
fn compact_ref_value_traits_follow_active_content() {
	let first_data = encode_journal("journal", Some("note"));
	let second_data = encode_journal("journal", Some("note"));
	let different_data = encode_journal("diary", None);

	let first = Journal::read_prefix(&first_data).unwrap();
	let second = Journal::read_prefix(&second_data).unwrap();
	let different = Journal::read_prefix(&different_data).unwrap();

	assert_eq!(first, second);
	assert_ne!(first, different);

	let first_hash = {
		let mut hasher = DefaultHasher::new();
		first.hash(&mut hasher);
		hasher.finish()
	};

	let second_hash = {
		let mut hasher = DefaultHasher::new();
		second.hash(&mut hasher);
		hasher.finish()
	};

	assert_eq!(first_hash, second_hash);

	let rendered = format!("{first:?}");

	assert!(rendered.contains("Journal"));
	assert!(rendered.contains("label"));
	assert!(rendered.contains("entries"));
}
