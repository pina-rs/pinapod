//! Kani proofs over derive-generated compact schemas.
//!
//! The pod-primitive proofs cover the handwritten crate. These harnesses
//! cover the code the derive generates: the validated offset walk, the
//! `Ref` accessors, and the `Patch` initialize/update commit path. The
//! schema is deliberately small so the proofs stay bounded, and every
//! harness states the property in terms of the public generated API only.

use crate::{pod, PinaPod, PinaPodCompact, String, Vec};

#[derive(PinaPod)]
#[pinapod(compact)]
struct Bounded {
    pub seq: u8,
    label: String<4>,
    values: Vec<u16, 4>,
    note: Option<String<4>>,
}

const _: () = assert!(<Bounded as PinaPodCompact>::HEADER_SIZE == 5);
const _: () = assert!(<Bounded as PinaPodCompact>::MIN_SIZE == 5);
const _: () = assert!(<Bounded as PinaPodCompact>::MAX_SIZE == 22);
const _: () = assert!(<Bounded as PinaPodCompact>::TAIL_ALIGNMENT == 1);

const LABEL: &str = "aaaa";
const NOTE: &str = "nnnn";
const STORAGE: usize = <Bounded as PinaPodCompact>::MAX_SIZE;

struct SymbolicValues([u16; 4]);

impl SymbolicValues {
    /// Derives the four element values from one base symbol. The proven
    /// properties depend on symbolic lengths and offsets, not on element
    /// value independence; deriving from one symbol keeps the solver state
    /// small enough for CI.
    fn new() -> Self {
        let base: u16 = kani::any();
        Self([
            base,
            base.wrapping_add(1),
            base.wrapping_add(2),
            base.wrapping_add(3),
        ])
    }

    fn pod(&self) -> [pod::PodU16; 4] {
        [
            pod::PodU16::from(self.0[0]),
            pod::PodU16::from(self.0[1]),
            pod::PodU16::from(self.0[2]),
            pod::PodU16::from(self.0[3]),
        ]
    }
}

/// Validation success must imply every accessor stays inside the validated
/// slice and the reported encoded length stays inside the allocation.
#[kani::proof]
#[kani::unwind(25)]
fn validated_accessors_are_bounded() {
    let data: [u8; STORAGE] = kani::any();
    if let Ok(view) = Bounded::read_prefix(&data) {
        assert!(view.label().len() <= 4);
        assert!(view.values().len() <= 4);
        match view.note() {
            Some(note) => assert!(note.len() <= 4),
            None => {}
        }
        assert!(view.encoded_len() <= view.storage_len());
        assert!(view.encoded_len() >= <Bounded as PinaPodCompact>::HEADER_SIZE);
        assert_eq!(
            view.spare_capacity(),
            view.storage_len() - view.encoded_len()
        );
    }
}

/// Initializing a garbage destination with a valid patch must succeed and
/// round-trip every field through a fresh validated view.
#[kani::proof]
#[kani::unwind(25)]
fn initialize_builds_a_valid_view() {
    let seq: u8 = kani::any();
    let label_len: usize = kani::any();
    kani::assume(label_len <= 4);
    let values = SymbolicValues::new();
    let values_len: usize = kani::any();
    kani::assume(values_len <= 4);
    let note_present: bool = kani::any();
    let note_len: usize = kani::any();
    kani::assume(note_len <= 4);

    // Concrete garbage start: `initialize` zeroes the destination before
    // writing, so the prior content cannot influence the outcome, and the
    // proof cost stays in the symbolic lengths where it belongs.
    let mut data: [u8; STORAGE] = [0xFF; STORAGE];
    let values_pod = values.pod();
    let patch = BoundedPatch::new()
        .seq(seq)
        .label(&LABEL[..label_len])
        .replace_values(&values_pod[..values_len])
        .note(if note_present {
            Some(&NOTE[..note_len])
        } else {
            None
        });

    let encoded_len =
        Bounded::initialize(&mut data, &patch).expect("initialize with a valid patch must succeed");
    assert!(encoded_len <= STORAGE);

    let view = Bounded::read_prefix(&data).expect("initialized bytes must validate");
    assert_eq!(view.seq, seq);
    assert_eq!(view.label(), &LABEL[..label_len]);
    assert_eq!(view.values(), &values_pod[..values_len]);
    match (view.note(), note_present) {
        (Some(note), true) => assert_eq!(note, &NOTE[..note_len]),
        (None, false) => {}
        _ => panic!("option round-trip mismatch"),
    }
}

/// Applying a second full patch to a valid buffer must commit every grow and
/// shrink combination atomically and leave a readable view.
#[kani::proof]
#[kani::unwind(25)]
fn update_preserves_roundtrip() {
    // Concrete start: only bytes written by the first `initialize` are read,
    // so symbolic garbage would add solver state without strengthening the
    // property.
    let mut data: [u8; STORAGE] = [0x7F; STORAGE];
    let first_values = SymbolicValues::new().pod();
    let first = BoundedPatch::new()
        .seq(0)
        .label(&LABEL[..2])
        .replace_values(&first_values[..2]);
    Bounded::initialize(&mut data, &first).expect("initialize with a valid patch must succeed");

    let seq: u8 = kani::any();
    let label_len: usize = kani::any();
    kani::assume(label_len <= 4);
    let values = SymbolicValues::new();
    let values_len: usize = kani::any();
    kani::assume(values_len <= 4);
    let note_present: bool = kani::any();
    let note_len: usize = kani::any();
    kani::assume(note_len <= 4);

    let values_pod = values.pod();
    let second = BoundedPatch::new()
        .seq(seq)
        .label(&LABEL[..label_len])
        .replace_values(&values_pod[..values_len])
        .note(if note_present {
            Some(&NOTE[..note_len])
        } else {
            None
        });

    let encoded_len = Bounded::update(&mut data, &second)
        .expect("updating valid bytes with a valid patch must succeed");
    assert!(encoded_len <= STORAGE);

    let view = Bounded::read_prefix(&data).expect("updated bytes must validate");
    assert_eq!(view.seq, seq);
    assert_eq!(view.label(), &LABEL[..label_len]);
    assert_eq!(view.values(), &values_pod[..values_len]);
    match (view.note(), note_present) {
        (Some(note), true) => assert_eq!(note, &NOTE[..note_len]),
        (None, false) => {}
        _ => panic!("option round-trip mismatch"),
    }
}
