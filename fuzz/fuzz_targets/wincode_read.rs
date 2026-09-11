//! Fuzzes the wincode deserialization path for pod storage types. Readers
//! must reject invalid stored bytes (bools, tags, lengths, UTF-8) instead of
//! constructing an invalid value, and valid round-trips must preserve the
//! active contents.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pinapod::{
    pod::{PodBool, PodI64, PodOption, PodString, PodU16, PodVec},
    PinaPodError, ZcValidate,
};

fuzz_target!(|data: &[u8]| {
    if let Ok(value) = wincode::deserialize::<PodBool>(data) {
        assert!(value.get() == (value.as_ref()[0] != 0));
    }
    if let Ok(value) = wincode::deserialize::<PodU16>(data) {
        assert_eq!(value.get(), u16::from_le_bytes([data[0], data[1]]));
    }
    if let Ok(value) = wincode::deserialize::<PodI64>(data) {
        let mut expected = [0u8; 8];
        expected.copy_from_slice(&data[..8]);
        assert_eq!(value.get(), i64::from_le_bytes(expected));
    }
    if let Ok(value) = wincode::deserialize::<PodString<8>>(data) {
        assert!(value.len() <= 8);
        assert!(ZcValidate::validate_ref(&value).is_ok());
        assert_eq!(value.as_bytes(), &data[1..1 + value.len()]);
    }
    if let Ok(value) = wincode::deserialize::<PodVec<PodU16, 4>>(data) {
        assert!(value.len() <= 4);
        assert!(ZcValidate::validate_ref(&value).is_ok());
    }
    if let Ok(value) = wincode::deserialize::<PodOption<PodBool>>(data) {
        match value.get() {
            Some(inner) => assert_eq!(value.raw_tag(), 1),
            None => assert!(value.raw_tag() != 1),
        }
    }
    let _ = wincode::deserialize::<PodOption<PodString<4>>>(data);
    let _ = wincode::deserialize::<PodVec<PodBool, 6>>(data);

    // Serialization is canonical: equivalent logical values encode equally,
    // and serialized bytes deserialize back to the same active contents.
    let mut first = PodString::<8>::default();
    first.try_set("hi").map_err(|e: PinaPodError| e).unwrap();
    let mut second = PodString::<8>::default();
    second.try_push_str("h").unwrap();
    second.try_push_str("i").unwrap();

    let size = wincode::serialized_size(&first).unwrap() as usize;
    let mut first_bytes = vec![0u8; size];
    wincode::serialize_into(first_bytes.as_mut_slice(), &first).unwrap();
    let mut second_bytes = vec![0u8; size];
    wincode::serialize_into(second_bytes.as_mut_slice(), &second).unwrap();

    assert_eq!(first_bytes, second_bytes);
    assert_eq!(
        wincode::deserialize::<PodString<8>>(&first_bytes)
            .unwrap()
            .as_str(),
        "hi"
    );
});
