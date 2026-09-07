#![deny(warnings)]

use pinapod::{PinaPod, PinaPodCompact};

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct GenericTail<T, const CAPACITY: usize>
where
    T: pinapod::ZcField,
    <T as pinapod::ZcField>::Pod: pinapod::ZcElem,
{
    values: pinapod::Vec<T, CAPACITY>,
}

#[allow(dead_code)]
#[derive(PinaPod)]
struct ScalarPayload {
    value: u64,
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
#[repr(u8)]
enum ScalarEvent {
    Empty = 0,
    Value(ScalarPayload) = 1,
}

#[test]
fn generic_compact_views_preserve_type_parameters_without_changing_layout() {
    assert_eq!(<GenericTail<u16, 4> as PinaPodCompact>::HEADER_SIZE, 2);

    let mut data = [0_u8; 2];
    let patch = GenericTailPatch::<u16, 4>::new();
    assert_eq!(patch.updated_len(&data).unwrap(), 2);
    assert_eq!(GenericTail::<u16, 4>::update(&mut data, &patch).unwrap(), 2);

    let reader = GenericTail::<u16, 4>::read_prefix(&data).unwrap();
    assert!(reader.values().is_empty());
}

#[test]
fn scalar_compact_enum_compiles_cleanly_when_warnings_are_denied() {
    let mut data = [0_u8; 1];
    let patch = ScalarEventPatch::Empty;

    assert_eq!(patch.updated_len(&data).unwrap(), 1);
    assert_eq!(ScalarEvent::initialize(&mut data, &patch).unwrap(), 1);
}
