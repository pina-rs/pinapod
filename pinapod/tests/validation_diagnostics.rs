//! Attribution tests for the derived `diagnose` walks.
//!
//! `diagnose` answers the follow-up question a flat rejection cannot: which
//! field. These tests pin each attribution site and, most importantly, the
//! acceptance contract — `diagnose` accepts and rejects exactly what the
//! corresponding reader validation accepts and rejects, with the same error
//! variant, including across pseudo-randomly corrupted buffers.

#![allow(
	dead_code,
	missing_docs,
	reason = "schemas and their generated surface exist to be exercised here, not published"
)]

use pinapod::Diagnostic;
use pinapod::PinaPod;
use pinapod::PinaPodCompact;
use pinapod::PinaPodError;

#[derive(PinaPod)]
#[repr(u8)]
enum Status {
	Active = 1,
	Retired = 2,
}

#[allow(dead_code)]
#[derive(PinaPod)]
struct Record {
	amount: u64,
	label: pinapod::String<8>,
	active: bool,
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct Journal {
	sequence: u64,
	approved: bool,
	label: pinapod::String<8>,
	entries: pinapod::Vec<u64, 8>,
	note: Option<pinapod::String<8>>,
	flags: Option<pinapod::Vec<u32, 8>>,
}

fn encode_record() -> Vec<u8> {
	let mut data = vec![0_u8; Record::SIZE];

	Record::initialize(&mut data, |record| {
		record.amount = 5_u64.into();
		record.label.try_set("ifi")?;
		record.active = true.into();
		Ok(())
	})
	.unwrap();

	data
}

fn encode_journal() -> Vec<u8> {
	let mut data = vec![0_u8; <Journal as PinaPodCompact>::MAX_SIZE];
	let entries: [pinapod::pod::PodU64; 8] = core::array::from_fn(|index| (index as u64).into());
	let flags: [pinapod::pod::PodU32; 8] = core::array::from_fn(|index| (index as u32).into());

	let patch = JournalPatch::new()
		.sequence(42_u64)
		.approved(true)
		.label("journal")
		.replace_entries(&entries[..3])
		.note(Some("note"))
		.replace_flags(Some(&flags[..2]));

	let used = Journal::initialize(&mut data, &patch).unwrap();

	data.truncate(used);
	data
}

#[test]
fn fixed_diagnose_attributes_each_rejection_site() {
	// A valid encoding diagnoses clean.
	assert_eq!(Record::diagnose(&encode_record()), Ok(()));

	// Non-UTF-8 active bytes are attributed to the string field.
	let mut corrupted = encode_record();
	corrupted[8] = 3; // label prefix: three active bytes
	corrupted[10] = 0xFF; // the third active byte is not UTF-8

	assert_eq!(
		Record::diagnose(&corrupted),
		Err(Diagnostic::new("label", PinaPodError::InvalidUtf8))
	);

	// An out-of-domain boolean byte is attributed to the boolean field.
	let mut corrupted = encode_record();
	corrupted[Record::SIZE - 1] = 7;

	assert_eq!(
		Record::diagnose(&corrupted),
		Err(Diagnostic::new("active", PinaPodError::InvalidBool))
	);

	// A short buffer is an allocation failure, so the schema owns it.
	let short = encode_record();
	assert_eq!(
		Record::diagnose(&short[..Record::SIZE - 1]),
		Err(Diagnostic::new("Record", PinaPodError::BufferTooSmall))
	);
}

#[test]
fn fixed_diagnose_agrees_with_validate_prefix() {
	let data = encode_record();

	assert!(Record::diagnose(&data).is_ok());
	assert!(Record::validate_prefix(&data).is_ok());

	let mut corrupted = encode_record();
	corrupted[8] = 200; // label prefix above capacity

	assert_eq!(
		Record::diagnose(&corrupted),
		Err(Diagnostic::new("label", PinaPodError::InvalidLength))
	);
	assert_eq!(
		Record::validate_prefix(&corrupted),
		Err(PinaPodError::InvalidLength)
	);
}

#[test]
fn enum_diagnose_attributes_unknown_discriminants() {
	assert_eq!(Status::diagnose(&[1]), Ok(()));
	assert_eq!(
		Status::diagnose(&[9]),
		Err(Diagnostic::new("Status", PinaPodError::InvalidDiscriminant))
	);
	assert_eq!(
		Status::diagnose(&[]),
		Err(Diagnostic::new("Status", PinaPodError::BufferTooSmall))
	);
}

#[test]
fn compact_diagnose_attributes_each_rejection_site() {
	let data = encode_journal();
	let header = <Journal as PinaPodCompact>::HEADER_SIZE;

	assert_eq!(Journal::diagnose(&data), Ok(()));

	// A tail-granularity of one makes any allocation at or above the header a
	// legal size, so a truncated buffer is attributed to the first tail whose
	// bounds leave it — here the last option's payload.
	let mut short = encode_journal();
	short.truncate(short.len() - 1);

	let diagnostic = Journal::diagnose(&short).unwrap_err();

	assert_eq!(diagnostic.field, "flags");
	assert_eq!(diagnostic.error, PinaPodError::BufferTooSmall);

	// An allocation above the schema's maximum is the schema's own failure.
	let mut oversized = encode_journal();
	oversized.resize(<Journal as PinaPodCompact>::MAX_SIZE + 1, 0);

	let diagnostic = Journal::diagnose(&oversized).unwrap_err();

	assert_eq!(diagnostic.field, "Journal");
	assert_eq!(diagnostic.error, PinaPodError::InvalidLength);

	// An out-of-domain inline boolean byte is attributed inline.
	let mut corrupted = encode_journal();
	corrupted[8] = 5; // `approved` sits at the ninth header byte

	assert_eq!(
		Journal::diagnose(&corrupted),
		Err(Diagnostic::new("approved", PinaPodError::InvalidBool))
	);

	// The tail prefixes and tags live in the header: sequence (0..8),
	// approved (8), label prefix (9), entries prefix (10..12), note tag (12),
	// flags tag (13). The tails proper start after the header with the label
	// payload, then entries prefix + payload, then the present options.
	// A vector tail's count prefix also lives in the header, like every tail's
	// length and tag; the tail proper holds only the payload.
	let entries_prefix = 10;
	let label_prefix = 9;
	let note_tag = 12;
	let label_payload = header;

	// A string tail prefix above its capacity names that tail.
	let mut corrupted = encode_journal();
	corrupted[label_prefix] = 9; // capacity is 8

	assert_eq!(
		Journal::diagnose(&corrupted),
		Err(Diagnostic::new("label", PinaPodError::InvalidLength))
	);

	// Non-UTF-8 active string bytes name the string tail.
	let mut corrupted = encode_journal();
	corrupted[label_payload] = 0xFF; // first active label byte

	assert_eq!(
		Journal::diagnose(&corrupted),
		Err(Diagnostic::new("label", PinaPodError::InvalidUtf8))
	);

	// A vector count above its capacity names the vector tail.
	let mut corrupted = encode_journal();
	corrupted[entries_prefix] = 0xFF;
	corrupted[entries_prefix + 1] = 0xFF; // count 65535, capacity is 8

	let diagnostic = Journal::diagnose(&corrupted).unwrap_err();

	assert_eq!(diagnostic.field, "entries");
	assert_eq!(diagnostic.error, PinaPodError::InvalidLength);

	// A corrupt option tag names the option tail.
	let mut corrupted = encode_journal();
	corrupted[note_tag] = 2;

	assert_eq!(
		Journal::diagnose(&corrupted),
		Err(Diagnostic::new("note", PinaPodError::InvalidTag))
	);
}

#[test]
fn compact_diagnose_agrees_with_validate_across_corrupted_buffers() {
	let seed = encode_journal();
	let mut state = 0x2545_F491_4F6C_DD1D_u64;

	let mut next = move || {
		// A deterministic xorshift keeps the corpus reproducible.
		state ^= state << 13;
		state ^= state >> 7;
		state ^= state << 17;
		state
	};

	for _ in 0..2_000 {
		let mut corrupted = seed.clone();

		// Flip one to three pseudo-random bytes; small mutations reach the
		// deep tails far more often than uniform rewrites.
		for _ in 0..=(next() % 3) {
			let index = (next() as usize) % corrupted.len();
			corrupted[index] = (next() % 256) as u8;
		}

		let validate = <Journal as PinaPodCompact>::validate(&corrupted);
		let diagnose = Journal::diagnose(&corrupted);

		assert_eq!(
			validate.is_ok(),
			diagnose.is_ok(),
			"acceptance disagreement on {corrupted:?}"
		);

		if let (Err(expected), Err(actual)) = (validate, diagnose) {
			assert_eq!(
				expected, actual.error,
				"variant disagreement on {corrupted:?}"
			);
		}
	}
}

#[test]
fn diagnostic_displays_the_field_and_carries_the_error() {
	let diagnostic = Diagnostic::new("label", PinaPodError::InvalidUtf8);

	assert_eq!(
		diagnostic.to_string(),
		"field `label`: invalid UTF-8 in string field"
	);
	assert_eq!(PinaPodError::from(diagnostic), PinaPodError::InvalidUtf8);

	let source = core::error::Error::source(&diagnostic).unwrap();

	assert_eq!(source.to_string(), PinaPodError::InvalidUtf8.to_string());
}
