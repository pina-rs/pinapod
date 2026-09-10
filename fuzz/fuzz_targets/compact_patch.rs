//! Fuzzes the compact patch commit path with structured inputs.
//!
//! Input bytes are decoded into two patches. Every successful `initialize`
//! or `update` must leave bytes that validate and round-trip exactly, and
//! `updated_len` must agree with the length returned by `update`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pinapod::{pod::PodU64, PinaPod, PinaPodCompact};

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct Ledger {
    pub sequence: u64,
    pub revision: u32,
    label: pinapod::String<64>,
    values: pinapod::Vec<u64, 16>,
    note: Option<pinapod::String<8>>,
}

/// A tiny cursor over the fuzz input that derives always-valid patch fields.
struct Plan<'a> {
    bytes: &'a [u8],
}

impl Plan<'_> {
    fn take(&mut self, count: usize) -> &[u8] {
        let split = self.bytes.len().min(count);
        let (head, tail) = self.bytes.split_at(split);
        self.bytes = tail;
        head
    }

    fn byte(&mut self) -> u8 {
        self.take(1).first().copied().unwrap_or(0)
    }
}

/// Maps arbitrary bytes onto ASCII so every generated string is valid UTF-8.
fn ascii(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| b'a' + byte % 26)
        .collect::<Vec<u8>>()
        .into_iter()
        .map(|byte| byte as char)
        .collect()
}

struct Fields {
    sequence: u64,
    pub revision: u32,
    label: String,
    values: Vec<PodU64>,
    note: Option<String>,
}

fn decode_fields(plan: &mut Plan<'_>) -> Fields {
    let label_len = plan.byte() as usize % 65;
    let values_len = plan.byte() as usize % 17;
    let note_present = plan.byte() % 2 == 1;
    let note_len = plan.byte() as usize % 9;
    let mut sequence = [0u8; 8];
    let chunk = plan.take(8);
    sequence[..chunk.len()].copy_from_slice(chunk);
    let mut revision = [0u8; 4];
    let chunk = plan.take(4);
    revision[..chunk.len()].copy_from_slice(chunk);

    Fields {
        sequence: u64::from_le_bytes(sequence),
        revision: u32::from_le_bytes(revision),
        label: ascii(plan.take(label_len)),
        values: plan
            .take(values_len * 8)
            .chunks(8)
            .filter(|chunk| chunk.len() == 8)
            .map(|chunk| {
                let mut buffer = [0u8; 8];
                buffer.copy_from_slice(chunk);
                PodU64::from(u64::from_le_bytes(buffer))
            })
            .collect(),
        note: note_present.then(|| ascii(plan.take(note_len))),
    }
}

fn build_patch(fields: &Fields) -> LedgerPatch<'_> {
    LedgerPatch::new()
        .sequence(fields.sequence)
        .revision(fields.revision)
        .label(&fields.label)
        .replace_values(&fields.values)
        .note(fields.note.as_deref())
}

fn assert_roundtrip(data: &[u8], fields: &Fields) {
    let view = Ledger::read_prefix(data).expect("committed bytes must validate");
    assert_eq!(view.sequence, fields.sequence);
    assert_eq!(view.revision, fields.revision);
    assert_eq!(view.label(), fields.label);
    assert_eq!(view.values(), fields.values.as_slice());
    match (&fields.note, view.note()) {
        (Some(expected), Some(actual)) => assert_eq!(actual, expected),
        (None, None) => {}
        _ => panic!("option round-trip mismatch"),
    }
}

fuzz_target!(|data: &[u8]| {
    let mut plan = Plan { bytes: data };
    let first = decode_fields(&mut plan);
    let second = decode_fields(&mut plan);
    let patch = build_patch(&first);

    let mut buffer = vec![0u8; <Ledger as PinaPodCompact>::MAX_SIZE];
    let encoded_len =
        Ledger::initialize(&mut buffer, &patch).expect("initialize with valid fields must succeed");
    assert!(encoded_len <= buffer.len());
    assert_roundtrip(&buffer, &first);

    let second_patch = build_patch(&second);
    let announced = Ledger::updated_len(&buffer, &second_patch)
        .expect("updated_len on valid bytes must succeed");
    let committed =
        Ledger::update(&mut buffer, &second_patch).expect("update with valid fields must succeed");
    assert_eq!(
        announced, committed,
        "preflight must match the commit length"
    );
    assert!(committed <= buffer.len());
    assert_roundtrip(&buffer, &second);
});
