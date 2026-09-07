//! Container storage remains initialized when copied into account bytes.
#![forbid(unsafe_code)]

use pinapod::{
    pod::{PodOption, PodString, PodU16, PodVec},
    PinaPod, PinaPodError,
};

#[allow(dead_code)]
#[derive(PinaPod)]
struct FixedContainers {
    pub text: PodString<8>,
    pub values: PodVec<PodU16, 3>,
    pub optional_text: PodOption<PodString<4>>,
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct CompactContainers {
    pub values: pinapod::Vec<pinapod::String<4>, 2>,
}

#[allow(dead_code)]
#[derive(PinaPod)]
struct OptionalText {
    pub value: PodOption<PodString<4>>,
}

#[allow(dead_code)]
#[derive(PinaPod)]
struct VectorBytes {
    pub values: PodVec<u16, 4>,
}

#[allow(dead_code)]
#[derive(PinaPod)]
struct StringBytes {
    pub value: PodString<8>,
}

#[test]
fn assigning_default_containers_keeps_every_account_byte_initialized() {
    let mut bytes = [0; FixedContainers::SIZE];

    {
        let view = FixedContainers::read_exact_mut(&mut bytes).unwrap();
        view.text = PodString::default();
        view.values = PodVec::<u16, 3>::default();
        view.optional_text = PodOption::some(PodString::default());
    }

    let mut expected = [0; FixedContainers::SIZE];
    expected[17] = 1;

    // Reading the complete byte slice also checks inactive capacity under Miri.
    assert_eq!(bytes, expected);
}

#[test]
fn compact_copy_initializes_inactive_capacity_in_nested_containers() {
    let mut bytes = [0; 7];
    let mut text = PodString::<4>::default();
    text.try_set("hi").unwrap();
    let values = [text];

    let patch = CompactContainersPatch::new().replace_values(&values);
    assert_eq!(
        CompactContainers::initialize(&mut bytes, &patch).unwrap(),
        bytes.len()
    );

    assert_eq!(bytes, [1, 0, 2, b'h', b'i', 0, 0]);
    let view = CompactContainers::read_prefix(&bytes).unwrap();
    assert_eq!(view.values()[0].as_str(), "hi");
}

#[test]
fn absent_options_do_not_expose_or_validate_inactive_payloads() {
    let bytes = [0, 255, 255, 255, 255, 255];
    let view = OptionalText::read_exact(&bytes).unwrap();

    assert!(view.value.get().is_none());
    assert!(view.value.get_ref().is_none());
    assert_eq!(format!("{:?}", view.value), "None");
}

#[test]
fn replacing_an_absent_payload_restores_a_valid_active_value() {
    let mut bytes = [0, 255, 255, 255, 255, 255];
    let mut text = PodString::<4>::default();
    text.try_set("ok").unwrap();

    {
        let view = OptionalText::read_exact_mut(&mut bytes).unwrap();
        view.value.set(Some(text));
        assert_eq!(view.value.get_ref().unwrap().as_str(), "ok");
    }

    assert_eq!(bytes, [1, 2, b'o', b'k', 0, 0]);
    let view = OptionalText::read_exact(&bytes).unwrap();
    assert_eq!(view.value.get_ref().unwrap().as_str(), "ok");
}

#[test]
fn active_options_still_reject_invalid_payloads() {
    let invalid_length = [1, 255, 255, 255, 255, 255];
    let invalid_utf8 = [1, 1, 255, 0, 0, 0];

    assert!(matches!(
        OptionalText::read_exact(&invalid_length),
        Err(PinaPodError::InvalidLength)
    ));
    assert!(matches!(
        OptionalText::read_exact(&invalid_utf8),
        Err(PinaPodError::InvalidUtf8)
    ));
}

#[test]
fn shortening_or_clearing_a_vector_zeroes_removed_values() {
    let mut bytes = [0; VectorBytes::SIZE];
    let values = &mut VectorBytes::read_exact_mut(&mut bytes).unwrap().values;
    values
        .try_set_from_slice(&[1u16.into(), 2u16.into(), 3u16.into(), 4u16.into()])
        .unwrap();
    values
        .try_set_from_slice(&[9u16.into(), 8u16.into()])
        .unwrap();
    assert_eq!(bytes, [2, 0, 9, 0, 8, 0, 0, 0, 0, 0]);

    let values = &mut VectorBytes::read_exact_mut(&mut bytes).unwrap().values;
    values.clear();
    assert_eq!(bytes, [0; 10]);
}

#[test]
fn shortening_or_clearing_a_string_zeroes_removed_bytes() {
    let mut bytes = [0; StringBytes::SIZE];
    let value = &mut StringBytes::read_exact_mut(&mut bytes).unwrap().value;
    value.try_set("private!").unwrap();
    value.try_set("ok").unwrap();
    assert_eq!(bytes, [2, b'o', b'k', 0, 0, 0, 0, 0, 0]);

    let value = &mut StringBytes::read_exact_mut(&mut bytes).unwrap().value;
    value.clear();
    assert_eq!(bytes, [0; 9]);
}
