#![cfg(feature = "floats")]

use {
    core::mem::{align_of, size_of},
    pinapod::{
        pod::{PodF32, PodF64},
        PinaPod, ZcElem, ZcField, ZcValidate,
    },
    std::vec::Vec as StdVec,
};

fn assert_mapping<T, Pod>()
where
    T: ZcField<Pod = Pod>,
    Pod: ZcElem,
{
    assert_eq!(size_of::<T::Pod>(), size_of::<Pod>());
    assert_eq!(align_of::<Pod>(), 1);
}

#[test]
fn float_primitives_map_to_alignment_one_pods() {
    assert_mapping::<f32, PodF32>();
    assert_mapping::<f64, PodF64>();

    // The native spelling is accepted anywhere `ZcField` is required, which is
    // what lets a schema declare `f32`/`f64` directly.
    fn assert_zc_elem<T: ZcElem>() {}
    assert_zc_elem::<PodF32>();
    assert_zc_elem::<PodF64>();
}

#[test]
fn float_pods_roundtrip_bit_patterns_exactly() {
    for value in [
        0.0_f32,
        -0.0,
        -1.5,
        3.141_592_7,
        f32::MIN_POSITIVE,
        f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::from_bits(0x7f80_0001), // signaling NaN payload
        f32::from_bits(0xffc0_0000), // negative quiet NaN
    ] {
        let pod = PodF32::from(value);
        assert_eq!(pod.to_bits(), value.to_bits(), "bits changed for {value}");
        assert_eq!(pod.get().to_bits(), value.to_bits());
        assert_eq!(f32::from(pod).to_bits(), value.to_bits());
    }

    for value in [0.0_f64, -2.25, f64::MAX, f64::NEG_INFINITY] {
        let pod = PodF64::from(value);
        assert_eq!(pod.to_bits(), value.to_bits());
        assert_eq!(pod.get(), value);
    }
}

#[test]
fn float_storage_is_little_endian() {
    let mut pod = PodF32::ZERO;
    pod.set(1.5);
    assert_eq!(pod.as_ref(), &1.5_f32.to_bits().to_le_bytes());

    pod.set_bits(0x4049_0fdb);
    assert_eq!(pod.get().to_bits(), 0x4049_0fdb);
    assert_eq!(pod.to_bits(), 0x4049_0fdb);

    let mut wide = PodF64::ZERO;
    wide.set(3.125);
    assert_eq!(wide.as_ref(), &3.125_f64.to_bits().to_le_bytes());
    assert_eq!(
        PodF64::new_from_array(3.125_f64.to_bits().to_le_bytes()),
        wide
    );
}

#[test]
fn equality_is_bitwise_not_float_valued() {
    assert_eq!(PodF32::from(1.5), PodF32::from(1.5));
    // `+0.0 == -0.0` under float semantics, but stored bits differ.
    assert_ne!(PodF32::from(0.0), PodF32::from(-0.0));

    let quiet_nan = PodF32::from(f32::NAN);
    assert_eq!(quiet_nan, quiet_nan, "NaN payloads compare bitwise");

    assert!(PodF32::ZERO.is_zero());
    assert!(!PodF32::from(-0.0).is_zero());

    // Ordering is intentionally absent: bitwise `Eq` and float ordering
    // cannot both hold (NaN payloads, `+0.0` vs `-0.0`). Decode with `get`
    // and compare the natives instead.
    assert_eq!(
        PodF32::from(1.0)
            .get()
            .partial_cmp(&PodF32::from(2.0).get()),
        Some(core::cmp::Ordering::Less)
    );
}

#[test]
fn every_bit_pattern_is_a_valid_stored_value() {
    // Validation cannot fail for any bit pattern, including NaN payloads and
    // the negative zero the float semantics would treat as equal to zero.
    for bits in [0u32, 1, 0x7f80_0001, 0xffc0_0000, u32::MAX] {
        let mut pod = PodF32::ZERO;
        pod.set_bits(bits);
        assert!(PodF32::validate_ref(&pod).is_ok());
        assert_eq!(pod.to_bits(), bits);
    }

    let mut wide = PodF64::ZERO;
    wide.set(f64::NAN);
    assert!(PodF64::validate_ref(&wide).is_ok());
}

#[allow(dead_code)]
#[derive(PinaPod)]
struct FloatAccount {
    pub temperature: f32,
    pub depth: f64,
    pub previous_depth: Option<f64>,
    pub bias: PodF32,
}

#[test]
fn fixed_schema_roundtrips_native_float_fields() {
    let mut bytes = [0u8; FloatAccount::SIZE];
    assert_eq!(size_of::<PodF32>(), 4);
    assert_eq!(size_of::<PodF64>(), 8);

    {
        let account = FloatAccount::read_exact_mut(&mut bytes).unwrap();
        account.temperature.set(-12.5);
        account.depth.set(3.125);
        account.previous_depth.set(Some(PodF64::from(2.0)));
        account.bias.set(0.25);
    }

    assert_eq!(&bytes[..4], &(-12.5_f32).to_bits().to_le_bytes());
    assert_eq!(&bytes[4..12], &3.125_f64.to_bits().to_le_bytes());

    let account = FloatAccount::read_exact(&bytes).unwrap();
    // The generated accessor decodes back to the native float.
    assert_eq!(account.temperature(), -12.5);
    assert_eq!(account.depth(), 3.125);
    assert_eq!(
        account.previous_depth.get().map(|value| value.get()),
        Some(2.0)
    );
    assert_eq!(account.bias.get(), 0.25);
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct FloatBook {
    pub mark_price: f32,
    pub venues: pinapod::Vec<f32, 4>,
    pub name: pinapod::String<8>,
}

#[test]
fn compact_schema_roundtrips_float_header_and_tail() {
    let mut bytes = [0u8; FloatBook::MAX_SIZE];
    let prices = [1.0_f32, 2.5];
    let mapped = prices.map(PodF32::from);

    let patch = FloatBookPatch::new()
        .mark_price(1.75)
        .replace_venues(&mapped)
        .name("float");
    let encoded_size = FloatBook::initialize(&mut bytes, &patch).unwrap();

    assert_eq!(
        encoded_size,
        FloatBook::HEADER_SIZE + mapped.len() * size_of::<PodF32>() + "float".len(),
    );

    let book = FloatBook::read_prefix(&bytes[..encoded_size]).unwrap();
    assert_eq!(book.mark_price.get(), 1.75);
    assert_eq!(
        book.venues()
            .iter()
            .map(|value| value.get())
            .collect::<StdVec<_>>(),
        prices,
    );
    assert_eq!(book.name(), "float");
}

#[test]
fn nan_payloads_survive_a_schema_round_trip() {
    let mut bytes = [0u8; FloatAccount::SIZE];
    {
        let account = FloatAccount::read_exact_mut(&mut bytes).unwrap();
        account.temperature.set_bits(0x7fc0_0001);
        account.depth.set(f64::NAN);
    }

    let account = FloatAccount::read_exact(&bytes).unwrap();
    assert_eq!(account.temperature.get().to_bits(), 0x7fc0_0001);
    assert!(account.depth.get().is_nan());
}

#[allow(dead_code)]
#[derive(PinaPod)]
struct FloatCollections {
    pub samples: Option<f32>,
    pub history: pinapod::Vec<f64, 3>,
}

#[test]
fn floats_compose_with_bounded_collections() {
    let mut bytes = [0u8; FloatCollections::SIZE];
    let history = [1.0_f64, -2.5];
    let mapped = history.map(PodF64::from);

    {
        let value = FloatCollections::read_exact_mut(&mut bytes).unwrap();
        value.samples.set(Some(PodF32::from(0.5)));
        value.history.try_set(&mapped).unwrap();
    }

    let value = FloatCollections::read_exact(&bytes).unwrap();
    assert_eq!(value.samples.get().map(|sample| sample.get()), Some(0.5));
    assert_eq!(
        value
            .history
            .iter()
            .map(|item| item.get())
            .collect::<StdVec<_>>(),
        history,
    );
}

#[test]
fn float_pods_expose_formatting_hashing_and_constants() {
    let pod = PodF32::from(1.5);
    assert_eq!(std::format!("{pod}"), "1.5");
    assert_eq!(std::format!("{pod:?}"), "1.5");
    assert_eq!(std::format!("{pod:x}"), "3fc00000");
    assert_eq!(std::format!("{pod:X}"), "3FC00000");
    assert_eq!(std::format!("{pod:b}"), "111111110000000000000000000000");
    assert_eq!(std::format!("{}", PodF64::from(3.125)), "3.125");

    // Bitwise hashing agrees for equal patterns.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&pod, &mut hasher);
    let first = std::hash::Hasher::finish(&hasher);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&PodF32::from(1.5), &mut hasher);
    assert_eq!(first, std::hash::Hasher::finish(&hasher));

    assert_eq!(PodF32::default(), PodF32::ZERO);
    assert_eq!(PodF32::MIN_POSITIVE.to_bits(), f32::MIN_POSITIVE.to_bits());
    assert_eq!(PodF32::MAX.to_bits(), f32::MAX.to_bits());
    assert_eq!(PodF64::MAX.to_bits(), f64::MAX.to_bits());

    // No `Ord`/`PartialOrd`/`max` on the pods themselves.
    assert!(PodF32::from(1.0).get() < PodF32::from(2.0).get());
}

#[test]
fn float_pods_copy_and_convert_like_byte_containers() {
    let pod = PodF32::from(-0.75);
    let copied = pod; // Copy
    let cloned = pod.clone();
    assert_eq!(copied, cloned);
    assert_eq!(f32::from(copied), -0.75);
    assert_eq!(
        PodF64::from(f64::from(PodF64::from(1.25))),
        PodF64::from(1.25)
    );
    assert_eq!(<StdVec<_>>::from([pod]).len(), 1);
}
