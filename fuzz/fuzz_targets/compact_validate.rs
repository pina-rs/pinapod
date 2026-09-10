//! Fuzzes the compact reader: storage-length validation, header validation,
//! the tail walk, and the cached-offset accessors over arbitrary bytes.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pinapod::{PinaPod, PinaPodCompact, String, Vec};

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
});
