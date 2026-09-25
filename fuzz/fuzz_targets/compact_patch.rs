//! Fuzzes the compact patch commit path with structured inputs.
//!
//! Input bytes are decoded into two patches whose lengths may exceed field
//! capacity, so both commit outcomes are reachable: a successful
//! `initialize` or `update` must leave bytes that validate and round-trip
//! exactly, `updated_len` must agree with the length returned by `update`,
//! and a rejected commit must leave the buffer untouched. A third phase
//! corrupts one byte of a committed buffer and proves an `update` over the
//! corrupted bytes either still commits a valid representation or refuses
//! without mutating anything.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pinapod::PinaPod;
use pinapod::PinaPodCompact;
use pinapod::pod::PodU64;

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

/// A tiny cursor over the fuzz input that derives patch fields, including
/// lengths beyond each field's capacity so rejected commits are reachable.
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

/// Lengths decode above capacity on purpose: `label` may reach 80 over a
/// capacity of 64, `values` may reach 20 over 16, and `note` may reach 12
/// over 8, so `Overflow` rejections are part of the fuzzed surface.
fn decode_fields(plan: &mut Plan<'_>) -> Fields {
	let label_len = plan.byte() as usize % 80;
	let values_len = plan.byte() as usize % 20;
	let note_present = plan.byte() % 2 == 1;
	let note_len = plan.byte() as usize % 12;
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
	match Ledger::initialize(&mut buffer, &patch) {
		Ok(encoded_len) => {
			assert!(encoded_len <= buffer.len());
			assert_roundtrip(&buffer, &first);
		}
		Err(_) => {
			assert!(
				buffer.iter().all(|byte| *byte == 0),
				"a rejected initialize must leave the destination zeroed"
			);
		}
	}

	// A rejected update must leave every byte untouched, whatever the reason:
	// an over-capacity patch value or a buffer the preflight refused.
	let snapshot = buffer.clone();
	let second_patch = build_patch(&second);
	match Ledger::updated_len(&buffer, &second_patch) {
		Ok(announced) => {
			let committed = Ledger::update(&mut buffer, &second_patch)
				.expect("update must succeed when the preflight announced a length");
			assert_eq!(
				announced, committed,
				"preflight must match the commit length"
			);
			assert!(committed <= buffer.len());
			assert_roundtrip(&buffer, &second);
		}
		Err(_) => {
			assert_eq!(
				Ledger::update(&mut buffer, &second_patch),
				Err(pinapod::PinaPodError::Overflow),
				"only an over-capacity patch can be rejected after a valid initialize"
			);
			assert_eq!(
				buffer, snapshot,
				"a rejected update must leave the buffer byte-identical"
			);

			// The buffer must still accept a fitting patch afterwards.
			let retry = LedgerPatch::new().label("retry");
			let committed = Ledger::update(&mut buffer, &retry)
				.expect("a valid patch must still commit after a rejected one");
			assert!(committed <= buffer.len());
			let view = Ledger::read_prefix(&buffer).expect("retried bytes must validate");
			assert_eq!(view.label(), "retry");
		}
	}

	// One corrupted byte: an update either still commits over a coincidentally
	// valid representation, or refuses without mutating anything.
	let index = plan.byte() as usize % buffer.len();
	let bit = 1 << (plan.byte() % 8);
	buffer[index] ^= bit;
	let corrupted = buffer.clone();
	let recovery = LedgerPatch::new().label("recover");
	match Ledger::update(&mut buffer, &recovery) {
		Ok(committed) => {
			assert!(committed <= buffer.len());
			let view = Ledger::read_prefix(&buffer).expect("recovered bytes must validate");
			assert_eq!(view.label(), "recover");
		}
		Err(_) => {
			assert_eq!(
				buffer, corrupted,
				"an update rejected over corrupted bytes must not mutate them"
			);
		}
	}
});
