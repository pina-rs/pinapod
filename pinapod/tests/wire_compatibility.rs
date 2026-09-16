//! Wire-compatibility checks against the two implementations used by the
//! Criterion comparison.

#![cfg(not(miri))]

const FIXED_SIZE: usize = 45;
const COMPACT_HEADER_SIZE: usize = 15;
const COMPACT_CAPACITY: usize = 207;
const MAX_LABEL: &str = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";

macro_rules! historical_fixture {
    ($module:ident, $package:ident, $attribute:ident) => {
        mod $module {
            use $package as pinapod;

            #[allow(dead_code)]
            #[derive(pinapod::ZeroPod)]
            pub struct Fixed {
                pub authority: [u8; 32],
                pub amount: u64,
                pub revision: u32,
                pub active: bool,
            }

            #[allow(dead_code)]
            #[derive(pinapod::ZeroPod)]
            #[$attribute(compact)]
            pub struct Compact {
                pub sequence: u64,
                pub revision: u32,
                pub label: pinapod::String<64>,
                pub values: pinapod::Vec<u64, 16>,
            }

            pub fn fixed() -> [u8; super::FIXED_SIZE] {
                let mut data = [0u8; super::FIXED_SIZE];
                let record = <Fixed as pinapod::ZeroPodFixed>::from_bytes_mut(&mut data).unwrap();

                record.authority = [0xA5; 32];
                record.amount = 1_000_000u64.into();
                record.revision = 42u32.into();
                record.active = true.into();
                data
            }

            pub fn compact(
                label: &str,
                value_count: usize,
            ) -> ([u8; super::COMPACT_CAPACITY], usize) {
                let mut data = [0u8; super::COMPACT_CAPACITY];
                let values: [pinapod::pod::PodU64; 16] =
                    core::array::from_fn(|index| (1 + index as u64).into());
                let mut record = CompactMut::new(&mut data).unwrap();

                record.sequence = 42u64.into();
                record.revision = 7u32.into();
                record.set_label(label).unwrap();
                record.set_values(&values[..value_count]).unwrap();
                let encoded_len = record.commit().unwrap();

                (data, encoded_len)
            }
        }
    };
}

mod current {
    use pinapod::PinaPod;

    #[allow(dead_code)]
    #[derive(PinaPod)]
    pub struct Fixed {
        pub authority: [u8; 32],
        pub amount: u64,
        pub revision: u32,
        pub active: bool,
    }

    #[allow(dead_code)]
    #[derive(PinaPod)]
    #[pinapod(compact)]
    pub struct Compact {
        pub sequence: u64,
        pub revision: u32,
        pub label: pinapod::String<64>,
        pub values: pinapod::Vec<u64, 16>,
    }

    pub fn fixed() -> [u8; super::FIXED_SIZE] {
        let mut data = [0u8; super::FIXED_SIZE];
        Fixed::initialize(&mut data, |record| {
            record.authority = [0xA5; 32];
            record.amount = 1_000_000u64.into();
            record.revision = 42u32.into();
            record.active = true.into();

            Ok(())
        })
        .unwrap();
        data
    }

    pub fn compact(label: &str, value_count: usize) -> ([u8; super::COMPACT_CAPACITY], usize) {
        let mut data = [0u8; super::COMPACT_CAPACITY];
        let values: [pinapod::pod::PodU64; 16] =
            core::array::from_fn(|index| (1 + index as u64).into());
        let patch = CompactPatch::new()
            .sequence(42u64)
            .revision(7u32)
            .label(label)
            .replace_values(&values[..value_count]);
        let encoded_len = Compact::initialize(&mut data, &patch).unwrap();

        (data, encoded_len)
    }
}

historical_fixture!(previous, pinapod_previous, pinapod);
historical_fixture!(upstream, zeropod, zeropod);

#[test]
fn fixed_wire_bytes_match_previous_pinapod_and_upstream_zeropod() {
    let current = current::fixed();

    assert_eq!(current, previous::fixed());
    assert_eq!(current, upstream::fixed());
}

#[test]
fn compact_wire_bytes_match_at_representative_tail_sizes() {
    for (label, value_count) in [
        ("small", 1),
        ("small", 2),
        ("small", 4),
        ("small", 8),
        ("small", 16),
        (MAX_LABEL, 16),
    ] {
        let (current, current_len) = current::compact(label, value_count);
        let (previous, previous_len) = previous::compact(label, value_count);
        let (upstream, upstream_len) = upstream::compact(label, value_count);
        let expected_len = COMPACT_HEADER_SIZE + label.len() + value_count * 8;

        assert_eq!(current_len, expected_len);
        assert_eq!(current_len, previous_len);
        assert_eq!(current_len, upstream_len);
        assert_eq!(&current[..current_len], &previous[..previous_len]);
        assert_eq!(&current[..current_len], &upstream[..upstream_len]);
    }
}
