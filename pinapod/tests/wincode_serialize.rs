//! Regression tests for canonical, fully initialized wincode serialization.
#![cfg(feature = "wincode")]
#![allow(
    unsafe_code,
    reason = "the derive macro emits audited zero-copy validation implementations"
)]

use pinapod::{
    pod::{PodBool, PodOption, PodString, PodU16, PodVec},
    PinaPodError,
};

#[test]
fn pod_bool_deserialization_validates_its_stored_byte() {
    let value = PodBool::from(true);
    assert_eq!(wincode::serialized_size(&value).unwrap(), 1);
    let stored = serialize::<1, _>(&value);
    assert_eq!(stored, [1]);
    assert!(!wincode::deserialize::<PodBool>(&[0]).unwrap().get());
    assert!(wincode::deserialize::<PodBool>(&stored).unwrap().get());

    let error = wincode::deserialize::<PodBool>(&[2]).unwrap_err();
    assert!(matches!(
        error,
        wincode::ReadError::InvalidValue("PodBool validation failed")
    ));
}

#[repr(transparent)]
#[derive(Clone, Copy)]
struct DynamicByte(u8);

impl pinapod::ZcValidate for DynamicByte {
    fn validate_ref(_value: &Self) -> Result<(), PinaPodError> {
        Ok(())
    }
}

// SAFETY: DynamicByte is transparent over u8, has alignment one and no invalid bit patterns.
unsafe impl pinapod::ZcElem for DynamicByte {}

// SAFETY: DynamicByte is already its complete alignment-one representation.
unsafe impl pinapod::ZcField for DynamicByte {
    type Pod = Self;
}

// SAFETY: The default dynamic metadata accurately describes this deliberately
// non-static test codec, and size_of matches the single byte written.
unsafe impl<C: wincode::config::ConfigCore> wincode::SchemaWrite<C> for DynamicByte {
    type Src = Self;

    fn size_of(_src: &Self) -> wincode::WriteResult<usize> {
        Ok(1)
    }

    fn write(mut writer: impl wincode::io::Writer, src: &Self) -> wincode::WriteResult<()> {
        writer.write(&[src.0])?;
        Ok(())
    }
}

fn serialize<const N: usize, T>(value: &T) -> [u8; N]
where
    T: wincode::SchemaWrite<wincode::config::DefaultConfig, Src = T>,
{
    let mut bytes = [0; N];
    wincode::serialize_into(bytes.as_mut_slice(), value).unwrap();
    bytes
}

#[test]
fn partial_string_zero_pads_capacity_and_roundtrips() {
    let mut value = PodString::<8>::default();
    value.try_set("hi").unwrap();
    assert_eq!(wincode::serialized_size(&value).unwrap(), 9);

    let bytes = serialize::<9, _>(&value);
    assert_eq!(bytes, [2, b'h', b'i', 0, 0, 0, 0, 0, 0]);

    let decoded = wincode::deserialize::<PodString<8>>(&bytes).unwrap();
    assert_eq!(decoded.as_str(), "hi");
}

#[test]
fn partial_vec_zero_pads_capacity_and_roundtrips() {
    let mut value = PodVec::<PodU16, 4, 2>::default();
    value.try_push(PodU16::from(5)).unwrap();
    value.try_push(PodU16::from(7)).unwrap();
    assert_eq!(wincode::serialized_size(&value).unwrap(), 10);

    let bytes = serialize::<10, _>(&value);
    assert_eq!(bytes, [2, 0, 5, 0, 7, 0, 0, 0, 0, 0]);

    let decoded = wincode::deserialize::<PodVec<PodU16, 4, 2>>(&bytes).unwrap();
    assert_eq!(decoded.as_slice(), &[PodU16::from(5), PodU16::from(7)]);
}

#[test]
fn serialization_does_not_disclose_truncated_capacity() {
    let mut truncated = PodString::<16>::default();
    truncated.try_set("abcdefgh").unwrap();
    truncated.truncate(3);

    let mut fresh = PodString::<16>::default();
    fresh.try_set("abc").unwrap();

    let truncated_bytes = serialize::<17, _>(&truncated);
    let fresh_bytes = serialize::<17, _>(&fresh);
    assert_eq!(truncated_bytes, fresh_bytes);
    assert!(truncated_bytes[4..].iter().all(|byte| *byte == 0));
}

#[test]
fn nested_vec_serializes_each_active_value_canonically() {
    let mut string = PodString::<8>::default();
    string.try_set("abcdefgh").unwrap();
    string.truncate(3);

    let mut value = PodVec::<PodString<8>, 2>::default();
    value.try_push(string).unwrap();

    let bytes = serialize::<20, _>(&value);
    assert_eq!(bytes.len(), 2 + 2 * 9);
    assert_eq!(&bytes[..6], &[1, 0, 3, b'a', b'b', b'c']);
    assert!(bytes[6..].iter().all(|byte| *byte == 0));

    let decoded = wincode::deserialize::<PodVec<PodString<8>, 2>>(&bytes).unwrap();
    assert_eq!(decoded[0].as_str(), "abc");
}

#[test]
fn nested_option_serializes_its_value_canonically() {
    let mut string = PodString::<8>::default();
    string.try_set("abcdefgh").unwrap();
    string.truncate(3);
    let value = PodOption::<_, 1>::some(string);

    let bytes = serialize::<10, _>(&value);
    assert_eq!(bytes, [1, 3, b'a', b'b', b'c', 0, 0, 0, 0, 0]);

    let decoded = wincode::deserialize::<PodOption<PodString<8>>>(&bytes).unwrap();
    assert_eq!(decoded.get_ref().unwrap().as_str(), "abc");
}

#[test]
fn none_option_zero_pads_its_value() {
    let value = PodOption::<PodString<8>>::none();
    assert_eq!(wincode::serialized_size(&value).unwrap(), 10);
    let bytes = serialize::<10, _>(&value);
    assert_eq!(bytes, [0; 10]);
}

#[test]
fn vec_of_options_zero_pads_capacity_and_roundtrips() {
    let mut value = PodVec::<PodOption<PodU16>, 3>::default();
    value
        .try_push(PodOption::some(PodU16::from(0x1234)))
        .unwrap();
    value.try_push(PodOption::none()).unwrap();
    assert_eq!(wincode::serialized_size(&value).unwrap(), 11);

    let bytes = serialize::<11, _>(&value);
    assert_eq!(bytes, [2, 0, 1, 0x34, 0x12, 0, 0, 0, 0, 0, 0]);

    let decoded = wincode::deserialize::<PodVec<PodOption<PodU16>, 3>>(&bytes).unwrap();
    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[0].get(), Some(PodU16::from(0x1234)));
    assert!(decoded[1].get().is_none());
}

#[test]
fn option_of_vec_zero_pads_truncated_capacity_and_roundtrips() {
    let mut values = PodVec::<PodU16, 3>::default();
    values.try_push(PodU16::from(0x1234)).unwrap();
    values.try_push(PodU16::from(0x5678)).unwrap();
    values.truncate(1);
    let value = PodOption::<_, 1>::some(values);
    assert_eq!(wincode::serialized_size(&value).unwrap(), 9);

    let bytes = serialize::<9, _>(&value);
    assert_eq!(bytes, [1, 1, 0, 0x34, 0x12, 0, 0, 0, 0]);

    let decoded = wincode::deserialize::<PodOption<PodVec<PodU16, 3>>>(&bytes).unwrap();
    assert_eq!(
        decoded.get_ref().unwrap().as_slice(),
        &[PodU16::from(0x1234)]
    );

    let absent = PodOption::<PodVec<PodU16, 3>>::none();
    let bytes = serialize::<9, _>(&absent);
    assert_eq!(bytes, [0; 9]);
    assert!(wincode::deserialize::<PodOption<PodVec<PodU16, 3>>>(&bytes)
        .unwrap()
        .get_ref()
        .is_none());
}

#[test]
fn vec_of_vecs_zero_pads_both_capacities_and_roundtrips() {
    let mut inner = PodVec::<PodU16, 2>::default();
    inner.try_push(PodU16::from(0x1234)).unwrap();
    inner.try_push(PodU16::from(0x5678)).unwrap();
    inner.truncate(1);
    let mut value = PodVec::<PodVec<PodU16, 2>, 2>::default();
    value.try_push(inner).unwrap();
    assert_eq!(wincode::serialized_size(&value).unwrap(), 14);

    let bytes = serialize::<14, _>(&value);
    assert_eq!(bytes, [1, 0, 1, 0, 0x34, 0x12, 0, 0, 0, 0, 0, 0, 0, 0]);

    let decoded = wincode::deserialize::<PodVec<PodVec<PodU16, 2>, 2>>(&bytes).unwrap();
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].as_slice(), &[PodU16::from(0x1234)]);
}

#[test]
fn option_of_option_preserves_both_tags_and_roundtrips() {
    let cases = [
        (None, [0, 0, 0, 0]),
        (Some(None), [1, 0, 0, 0]),
        (Some(Some(PodU16::from(0x1234))), [1, 1, 0x34, 0x12]),
    ];

    for (expected, expected_bytes) in cases {
        let value = match expected {
            Some(Some(inner)) => PodOption::some(PodOption::some(inner)),
            Some(None) => PodOption::some(PodOption::none()),
            None => PodOption::<PodOption<PodU16>>::none(),
        };
        assert_eq!(wincode::serialized_size(&value).unwrap(), 4);

        let bytes = serialize::<4, _>(&value);
        assert_eq!(bytes, expected_bytes);

        let decoded = wincode::deserialize::<PodOption<PodOption<PodU16>>>(&bytes).unwrap();
        assert_eq!(decoded.get().map(|inner| inner.get()), expected);
    }
}

#[test]
fn containers_reject_dynamic_element_encodings() {
    let value = PodVec::<DynamicByte, 1>::default();
    let error = wincode::serialized_size(&value).unwrap_err();
    assert!(matches!(
        error,
        wincode::WriteError::Custom(message)
            if message.contains("fixed wincode representation")
    ));
}

#[test]
fn nested_containers_reject_dynamic_element_encodings() {
    let vec = PodVec::<PodOption<DynamicByte>, 1>::default();
    let option = PodOption::<PodVec<DynamicByte, 1>>::none();

    for error in [
        wincode::serialized_size(&vec).unwrap_err(),
        wincode::serialized_size(&option).unwrap_err(),
    ] {
        assert!(matches!(
            error,
            wincode::WriteError::Custom(message)
                if message.contains("fixed wincode representation")
        ));
    }
}

#[test]
fn option_writer_rejects_an_invalid_tag() {
    let bytes = [2u8, 0];
    // SAFETY: PodOption<u8> permits every initialized byte pattern; validation
    // deliberately happens when its safe API or serializer interprets the tag.
    let value = unsafe { core::ptr::read_unaligned(bytes.as_ptr().cast::<PodOption<u8>>()) };
    let mut output = [0u8; 2];
    let error = wincode::serialize_into(output.as_mut_slice(), &value).unwrap_err();
    assert!(matches!(
        error,
        wincode::WriteError::Custom("PinaPod option has an invalid tag")
    ));
}
