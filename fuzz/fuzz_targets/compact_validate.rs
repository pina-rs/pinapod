//! Fuzzes the compact reader: storage-length validation, header validation,
//! the tail walk, and the tail accessors over arbitrary bytes.
//!
//! The second schema pins four- and eight-byte length prefixes so the wide
//! decode paths (checked arithmetic, `usize::try_from` rejection of lengths
//! that do not fit the target) are exercised with attacker-chosen prefixes,
//! not only through unit fixtures.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pinapod::PinaPod;
use pinapod::PinaPodCompact;
use pinapod::String;
use pinapod::Vec;

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct Ledger {
	pub sequence: u64,
	pub revision: u32,
	label: String<64>,
	values: Vec<u64, 16>,
	note: Option<String<8>>,
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct WidePrefixLedger {
	pub sequence: u64,
	blob: pinapod::PodVec<u64, 96, 8>,
	tagline: pinapod::PodString<192, 4>,
}

fuzz_target!(|data: &[u8]| {
	let min = <Ledger as PinaPodCompact>::MIN_SIZE;
	let max = <Ledger as PinaPodCompact>::MAX_SIZE;
	let storage_ok = (min..=max).contains(&data.len());

	if let Ok(view) = Ledger::read_prefix(data) {
		assert!(storage_ok, "validated storage length must be in range");
		assert!(view.label().len() <= 64);
		assert!(view.values().len() <= 16);
		match view.note() {
			Some(note) => assert!(note.len() <= 8),
			None => {}
		}
		assert!(view.encoded_len() <= view.storage_len());
		assert!(view.spare_capacity() == view.storage_len() - view.encoded_len());
	}

	let min = <WidePrefixLedger as PinaPodCompact>::MIN_SIZE;
	let max = <WidePrefixLedger as PinaPodCompact>::MAX_SIZE;
	if let Ok(view) = WidePrefixLedger::read_prefix(data) {
		assert!((min..=max).contains(&data.len()));
		assert!(view.blob().len() <= 96);
		assert!(view.tagline().len() <= 192);
		assert!(view.encoded_len() <= view.storage_len());
	}
});
