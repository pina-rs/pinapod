//! Fuzzes the compact reader: storage-length validation, header validation,
//! the tail walk, and the tail accessors over arbitrary bytes.
//!
//! The second schema pins four- and eight-byte length prefixes so the wide
//! decode paths (checked arithmetic, `usize::try_from` rejection of lengths
//! that do not fit the target) are exercised with attacker-chosen prefixes,
//! not only through unit fixtures. The third schema covers the nested tail
//! grammar the plain schemas omit: an optional vector (`Option<Vec<T, N>>`,
//! whose cleared tail carries a forged tag plus a stale region) and a vector
//! of strings (`Vec<String<M>, N>`, whose per-element walk owns the nested
//! UTF-8 and per-element prefix checks).

#![no_main]

use libfuzzer_sys::fuzz_target;
use pinapod::PinaPod;
use pinapod::PinaPodCompact;
use pinapod::String;
use pinapod::Vec;
use pinapod::ZcValidate;

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

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct NestedTailLedger {
	pub sequence: u64,
	pairs: Option<Vec<u64, 4>>,
	labels: Vec<String<4>, 2>,
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

	if let Ok(view) = NestedTailLedger::read_prefix(data) {
		match view.pairs() {
			Some(pairs) => {
				assert!(
					pairs.len() <= 4,
					"an accepted optional vector stays in capacity"
				)
			}
			None => {}
		}
		assert!(
			view.labels().len() <= 2,
			"an accepted string vector stays in capacity"
		);
		for label in view.labels() {
			assert!(
				label.len() <= 4,
				"an accepted nested string stays in capacity"
			);
			assert!(
				ZcValidate::validate_ref(label).is_ok(),
				"a view exposes only validated nested strings"
			);
		}
		assert!(view.encoded_len() <= view.storage_len());
	}
});
