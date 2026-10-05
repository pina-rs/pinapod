//! Adversarial coverage for dynamic containers nested inside array fields.
//!
//! Array fields map element-wise, so a `PodString` or `PodVec` inside an array
//! lands in a fixed or compact representation through the same rewrite as a
//! top-level declaration. The compile-fail fixtures `array_nested_prefix_width.rs`
//! and `option_array_nested_prefix_width.rs` pin the declaration-side contract:
//! an unsupported prefix width inside an array must be a compile error, not a
//! silent re-encode with a one-byte prefix. The tests here attack the runtime
//! side of the same surface: a malicious account buffer whose forged inner
//! prefixes, tags, and payload bytes must be rejected before any reference is
//! formed.

#![allow(
	dead_code,
	missing_docs,
	unused_qualifications,
	reason = "these adversarial fixtures are not a published surface, the schemas exist to be \
	          attacked through their generated readers rather than their own fields, and layout \
	          offsets are spelled through fully qualified paths on purpose"
)]

use pinapod::PinaPod;
use pinapod::PinaPodCompact;
use pinapod::PinaPodError;
use pinapod::PodString;

// The size claim behind the compile-time contract: an eight-byte payload with a
// one-byte prefix occupies nine bytes. Before the derive rejected
// `[PodString<8, 3>; N]`, it silently re-encoded the field with this one-byte
// prefix, so the account held a different layout than the declaration spelled
// and any client generated from the declared schema would disagree with the
// on-chain bytes. The fixtures above are what fail the derivation now.
const _: () = assert!(core::mem::size_of::<PodString<8, 1>>() == 9);

const OWNER: usize = core::mem::size_of::<[u8; 32]>();
const NAME: usize = core::mem::size_of::<PodString<8>>();
const ALIASES_SPAN: usize = 1 + 2 * NAME;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct Registry {
	pub owner: [u8; 32],
	pub names: [PodString<8>; 4],
	pub aliases: Option<[PodString<8>; 2]>,
}

#[derive(PinaPod)]
struct Ledger {
	pub entries: [PodString<8>; 2],
}

/// A maximally sized, fully zeroed allocation: valid storage for the schema,
/// with every string empty and the option absent.
fn zeroed_allocation() -> Vec<u8> {
	vec![0_u8; Registry::MAX_SIZE]
}

/// A valid buffer with two populated names and one alias, built through the
/// generated patch so the starting bytes are known canonical.
fn valid_registry_buffer() -> Vec<u8> {
	let mut data = zeroed_allocation();
	let names = [
		PodString::<8>::try_from("alpha").unwrap(),
		PodString::<8>::try_from("beta").unwrap(),
		PodString::<8>::default(),
		PodString::<8>::default(),
	];
	let aliases = [
		PodString::<8>::try_from("aka").unwrap(),
		PodString::<8>::default(),
	];
	let patch = RegistryPatch::new().names(names).aliases(Some(aliases));
	Registry::initialize(&mut data, &patch).unwrap();
	data
}

/// The byte offset of `names[index]`'s one-byte length prefix.
fn name_prefix_offset(index: usize) -> usize {
	OWNER + index * NAME
}

/// The byte offset of the first active payload byte of `names[index]`.
fn name_payload_offset(index: usize) -> usize {
	name_prefix_offset(index) + 1
}

/// The byte offset of the one-byte option tag ahead of the alias elements.
fn option_tag_offset() -> usize {
	Registry::HEADER_SIZE - ALIASES_SPAN
}

#[test]
fn valid_array_nested_strings_round_trip() {
	let data = valid_registry_buffer();

	let registry = Registry::read_prefix(&data).unwrap();
	assert_eq!(registry.names[0].as_str(), "alpha");
	assert_eq!(registry.names[1].as_str(), "beta");
	assert_eq!(registry.names[2].as_str(), "");
	let aliases = registry.aliases.get().unwrap();
	assert_eq!(aliases[0].as_str(), "aka");
	assert_eq!(aliases[1].as_str(), "");
}

#[test]
fn zeroed_array_nested_storage_is_valid_and_empty() {
	let data = zeroed_allocation();
	let registry = Registry::read_prefix(&data).unwrap();

	for name in &registry.names {
		assert!(name.is_empty());
	}

	assert!(registry.aliases.get().is_none());
}

#[test]
fn forged_inner_length_prefix_is_rejected_by_the_full_walk() {
	let mut data = valid_registry_buffer();

	// Corrupt the second name's stored length to one past its capacity.
	data[name_prefix_offset(1)] = 9;

	assert_eq!(Registry::validate(&data), Err(PinaPodError::InvalidLength));
	assert!(
		Registry::read_prefix(&data).is_err(),
		"no reference may be formed over a forged inner prefix"
	);
}

#[test]
fn forged_inner_utf8_is_rejected_by_the_full_walk() {
	let mut data = valid_registry_buffer();

	// Overwrite the first name's active bytes with lone continuation bytes,
	// keeping the length prefix itself canonical so only UTF-8 is forged.
	for offset in 0..4 {
		data[name_payload_offset(0) + offset] = 0xFF;
	}

	assert_eq!(Registry::validate(&data), Err(PinaPodError::InvalidUtf8));
	assert!(Registry::read_prefix(&data).is_err());
}

#[test]
fn layout_walk_skips_inline_array_content_and_validate_interprets_it() {
	// An inline array lives in the header, and the relocation walk never
	// reads header content: it proves the allocation, the tail prefixes, and
	// the chained tail bounds, nothing else. Both forgeries below are
	// therefore invisible to `validate_layout` by design, while every
	// value-exposing boundary rejects them through the full walk. This
	// mirrors the documented split that
	// `validate_layout_accepts_a_semantically_invalid_but_readable_layout`
	// pins for tails.
	let mut utf8_forged = valid_registry_buffer();

	for offset in 0..4 {
		utf8_forged[name_payload_offset(0) + offset] = 0xFF;
	}

	assert!(Registry::validate_layout(&utf8_forged).is_ok());
	assert_eq!(
		Registry::validate(&utf8_forged),
		Err(PinaPodError::InvalidUtf8)
	);

	let mut length_forged = valid_registry_buffer();
	length_forged[name_prefix_offset(1)] = 9;
	assert!(Registry::validate_layout(&length_forged).is_ok());
	assert_eq!(
		Registry::validate(&length_forged),
		Err(PinaPodError::InvalidLength)
	);
}

#[test]
fn absent_option_hides_arbitrary_inactive_array_payload() {
	let mut data = valid_registry_buffer();

	// Force the option absent while leaving arbitrary bytes in the inactive
	// payload slots. Validation must accept the storage, and no safe accessor
	// may expose the stale payload.
	let option_tag = option_tag_offset();
	data[option_tag] = 0;

	for byte in &mut data[option_tag + 1..Registry::HEADER_SIZE] {
		*byte = 0xA5;
	}

	let registry = Registry::read_prefix(&data).unwrap();
	assert!(registry.aliases.get().is_none());
	assert!(Registry::validate(&data).is_ok());
}

#[test]
fn forged_option_tag_over_an_array_payload_is_rejected() {
	let mut data = valid_registry_buffer();

	data[option_tag_offset()] = 2;

	assert_eq!(Registry::validate(&data), Err(PinaPodError::InvalidTag));
}

#[test]
fn short_allocation_is_rejected_before_any_byte_is_read() {
	// Every byte of the header must be present before any prefix is decoded,
	// including the tag and payload bytes an inline array claims.
	let short = vec![0_u8; Registry::MIN_SIZE - 1];
	assert_eq!(Registry::validate(&short), Err(PinaPodError::InvalidLength));
	assert!(Registry::read_prefix(&short).is_err());
}

#[test]
fn fixed_layout_also_rejects_forged_inner_prefixes() {
	let mut data = vec![0_u8; Ledger::SIZE];
	let entries = [
		PodString::<8>::try_from("one").unwrap(),
		PodString::<8>::try_from("two").unwrap(),
	];
	Ledger::initialize(&mut data, |ledger| {
		ledger.entries = entries;
		Ok(())
	})
	.unwrap();

	// Forge the second element's prefix beyond its capacity.
	data[NAME] = 9;

	assert_eq!(
		Ledger::validate_exact(&data),
		Err(PinaPodError::InvalidLength)
	);
	assert!(Ledger::read_exact(&data).is_err());
}
