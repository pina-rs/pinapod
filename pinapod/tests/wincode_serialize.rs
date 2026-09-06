//! Regression tests for canonical, fully initialized wincode serialization.
#![cfg(feature = "wincode")]
#![allow(
    unsafe_code,
    reason = "the derive macro emits audited zero-copy validation implementations"
)]

use pinapod::{
    pod::{PodOption, PodString, PodU16, PodVec},
    ZeroPodError,
};

#[repr(transparent)]
#[derive(Clone, Copy)]
struct DynamicByte(u8);

impl pinapod::ZcValidate for DynamicByte {
    fn validate_ref(_value: &Self) -> Result<(), ZeroPodError> {
        Ok(())
    }
}

// SAFETY: DynamicByte is transparent over u8, has alignment one and no invalid bit patterns.
unsafe impl pinapod::ZcElem for DynamicByte {}

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
    assert!(value.set("hi"));

    let bytes = serialize::<9, _>(&value);
    assert_eq!(bytes, [2, b'h', b'i', 0, 0, 0, 0, 0, 0]);

    let decoded = wincode::deserialize::<PodString<8>>(&bytes).unwrap();
    assert_eq!(decoded.as_str(), "hi");
}

#[test]
fn partial_vec_zero_pads_capacity_and_roundtrips() {
    let mut value = PodVec::<PodU16, 4, 2>::default();
    assert!(value.push(PodU16::from(5)));
    assert!(value.push(PodU16::from(7)));

    let bytes = serialize::<10, _>(&value);
    assert_eq!(bytes, [2, 0, 5, 0, 7, 0, 0, 0, 0, 0]);

    let decoded = wincode::deserialize::<PodVec<PodU16, 4, 2>>(&bytes).unwrap();
    assert_eq!(decoded.as_slice(), &[PodU16::from(5), PodU16::from(7)]);
}

#[test]
fn serialization_does_not_disclose_truncated_capacity() {
    let mut truncated = PodString::<16>::default();
    assert!(truncated.set("abcdefgh"));
    truncated.truncate(3);

    let mut fresh = PodString::<16>::default();
    assert!(fresh.set("abc"));

    let truncated_bytes = serialize::<17, _>(&truncated);
    let fresh_bytes = serialize::<17, _>(&fresh);
    assert_eq!(truncated_bytes, fresh_bytes);
    assert!(truncated_bytes[4..].iter().all(|byte| *byte == 0));
}

#[test]
fn nested_vec_serializes_each_active_value_canonically() {
    let mut string = PodString::<8>::default();
    assert!(string.set("abcdefgh"));
    string.truncate(3);

    let mut value = PodVec::<PodString<8>, 2>::default();
    assert!(value.push(string));

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
    assert!(string.set("abcdefgh"));
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
fn option_writer_rejects_an_invalid_tag() {
    let bytes = [2u8, 0];
    // SAFETY: PodOption<u8> permits every initialized byte pattern; validation
    // deliberately happens when its safe API or serializer interprets the tag.
    let value = unsafe { core::ptr::read_unaligned(bytes.as_ptr().cast::<PodOption<u8>>()) };
    let mut output = [0u8; 2];
    let error = wincode::serialize_into(output.as_mut_slice(), &value).unwrap_err();
    assert!(matches!(
        error,
        wincode::WriteError::Custom("Pinapod option has an invalid tag")
    ));
}
