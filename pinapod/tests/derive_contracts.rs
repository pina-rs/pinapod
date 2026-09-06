#![deny(warnings)]

use pinapod::{ZeroPod, ZeroPodCompact};

#[allow(dead_code)]
#[derive(ZeroPod)]
#[pinapod(compact)]
struct GenericTail<T, const CAPACITY: usize>
where
    T: pinapod::ZcField,
    <T as pinapod::ZcField>::Pod: pinapod::ZcElem,
{
    values: pinapod::Vec<T, CAPACITY>,
}

#[allow(dead_code)]
#[derive(ZeroPod)]
struct ScalarPayload {
    value: u64,
}

#[allow(dead_code)]
#[derive(ZeroPod)]
#[pinapod(compact)]
#[repr(u8)]
enum ScalarEvent {
    Empty = 0,
    Value(ScalarPayload) = 1,
}

#[test]
fn generic_compact_views_preserve_type_parameters_without_changing_layout() {
    assert_eq!(<GenericTail<u16, 4> as ZeroPodCompact>::HEADER_SIZE, 2);

    let mut data = [0_u8; 2];
    let mut writer = GenericTailMut::<u16, 4>::new(&mut data).unwrap();
    assert_eq!(writer.projected_size(), 2);
    assert_eq!(writer.commit().unwrap(), 2);

    let reader = GenericTailRef::<u16, 4>::new(&data).unwrap();
    assert!(reader.values().is_empty());
}

#[test]
fn scalar_compact_enum_compiles_cleanly_when_warnings_are_denied() {
    let mut data = [0_u8; 1];
    let mut event = ScalarEventMut::new(&mut data).unwrap();

    assert_eq!(event.projected_size(), 1);
    assert_eq!(event.commit().unwrap(), 1);
}
