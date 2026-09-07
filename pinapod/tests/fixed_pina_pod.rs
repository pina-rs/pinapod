#![allow(
    unsafe_code,
    reason = "the derive macro emits audited zero-copy representation implementations"
)]

use core::mem::size_of;
use pinapod::{
    pod::{PodOption, PodString},
    PinaPod, PinaPodError, PinaPodFixed,
};

type ExplicitWideVector = pinapod::PodVec<u64, 1024, 2>;

const _: () = assert!(size_of::<ExplicitWideVector>() == 2 + 1024 * 8);

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(no_inherent)]
struct EmbeddedSchema {
    value: u64,
}

impl EmbeddedSchema {
    const SIZE: usize = 99;

    fn initialize() {}
}

const _: () = assert!(size_of::<<EmbeddedSchema as PinaPodFixed>::Zc>() == 8);

#[test]
fn no_inherent_allows_framework_owned_helpers() {
    assert_eq!(EmbeddedSchema::SIZE, 99);
    EmbeddedSchema::initialize();
}

#[allow(dead_code)]
#[derive(PinaPod)]
struct NestedCollections {
    title: pinapod::String<8>,
    nickname: Option<pinapod::String<8>>,
    score: Option<u64>,
    samples: pinapod::Vec<u16, 3>,
    names: pinapod::Vec<pinapod::String<8>, 3>,
    aliases: pinapod::Vec<Option<pinapod::String<4>>, 2>,
    preferred: Option<pinapod::Vec<u16, 3>>,
    history: pinapod::PodVec<u64, 4, 1>,
    label: PodString<300, 2>,
}

#[test]
fn fixed_layout_supports_recursively_bounded_containers() {
    assert_eq!(NestedCollections::SIZE, 423);

    let mut bytes = vec![0; NestedCollections::SIZE];
    let value = NestedCollections::read_exact_mut(&mut bytes).unwrap();

    value.title.try_set("PinaPod").unwrap();
    value
        .nickname
        .set(Some(PodString::try_from("ifi").unwrap()));
    value.score.set(Some(99.into()));
    value.samples.try_set([3_u16, 5, 8]).unwrap();
    value
        .names
        .try_push(PodString::try_from("alice").unwrap())
        .unwrap();
    value
        .aliases
        .try_push(PodOption::some(PodString::try_from("ifi").unwrap()))
        .unwrap();

    let mut preferred = pinapod::PodVec::<u16, 3>::default();
    preferred.try_push(7_u16).unwrap();
    preferred.try_push(11_u16).unwrap();
    value.preferred.set(Some(preferred));
    value.history.try_push(42_u64).unwrap();
    value.label.try_set("a wider prefix").unwrap();

    let value = NestedCollections::read_exact(&bytes).unwrap();
    assert_eq!(value.title(), "PinaPod");
    assert_eq!(value.nickname(), Some("ifi"));
    assert_eq!(value.score(), Some(99));
    assert_eq!(
        value
            .samples()
            .iter()
            .map(|sample| sample.get())
            .collect::<Vec<_>>(),
        [3, 5, 8]
    );
    assert_eq!(value.names()[0].as_str(), "alice");
    assert_eq!(
        value.aliases()[0].get_ref().map(PodString::as_str),
        Some("ifi")
    );
    assert_eq!(value.preferred().map(|values| values[1].get()), Some(11));
    assert_eq!(value.history()[0].get(), 42);
    assert_eq!(value.label(), "a wider prefix");
}

#[test]
fn fixed_exact_reads_reject_trailing_bytes() {
    let exact = vec![0; NestedCollections::SIZE];
    let short = vec![0; NestedCollections::SIZE - 1];
    let mut with_trailing = vec![0; NestedCollections::SIZE + 1];

    assert!(NestedCollections::read_exact(&exact).is_ok());
    assert!(matches!(
        NestedCollections::read_exact(&short),
        Err(PinaPodError::BufferTooSmall)
    ));
    assert!(matches!(
        NestedCollections::read_exact(&with_trailing),
        Err(PinaPodError::InvalidLength)
    ));
    assert!(NestedCollections::validate_prefix(&with_trailing).is_ok());
    assert!(NestedCollections::read_prefix(&with_trailing).is_ok());
    assert!(NestedCollections::read_prefix_mut(&mut with_trailing).is_ok());
}

#[test]
fn fixed_validation_reaches_nested_active_strings() {
    let mut bytes = vec![0; NestedCollections::SIZE];

    // One active outer element whose first string byte is invalid UTF-8.
    let names_offset = 36;
    bytes[names_offset..names_offset + 2].copy_from_slice(&1_u16.to_le_bytes());
    bytes[names_offset + 2] = 1;
    bytes[names_offset + 3] = 0xff;

    assert!(matches!(
        NestedCollections::read_exact(&bytes),
        Err(PinaPodError::InvalidUtf8)
    ));
}

const BASE_DISCRIMINANT: u8 = 7;

#[derive(PinaPod, Debug, PartialEq)]
#[repr(u8)]
enum ExpressionDiscriminant {
    First = BASE_DISCRIMINANT,
    Second = Self::First as u8 + 1,
}

#[test]
fn enum_uses_compiler_evaluated_discriminants() {
    let first = ExpressionDiscriminant::read_exact(&[7]).unwrap();
    let second = ExpressionDiscriminant::read_exact(&[8]).unwrap();

    assert_eq!(first.try_to_enum(), Ok(ExpressionDiscriminant::First));
    assert_eq!(second.try_to_enum(), Ok(ExpressionDiscriminant::Second));
}

#[derive(PinaPod, Debug, PartialEq)]
#[repr(u8)]
enum NonZeroState {
    Ready = 1,
    Complete = 2,
}

#[allow(dead_code)]
#[derive(PinaPod)]
struct RequiresInitialization {
    state: NonZeroState,
    count: u64,
}

#[test]
fn initialization_configures_nonzero_discriminants_before_validation() {
    let mut bytes = [0xff; RequiresInitialization::SIZE];

    let value = RequiresInitialization::initialize(&mut bytes, |value| {
        value.state = NonZeroState::Ready.into();
        value.count = 9.into();
        Ok(())
    })
    .unwrap();

    assert_eq!(value.state.try_to_enum(), Ok(NonZeroState::Ready));
    assert_eq!(value.count(), 9);
}

#[test]
fn initialization_zeroes_the_destination_after_callback_errors() {
    let mut bytes = [0xff; RequiresInitialization::SIZE];

    let result = RequiresInitialization::initialize(&mut bytes, |value| {
        value.state = NonZeroState::Complete.into();
        value.count = 100.into();
        Err(PinaPodError::Overflow)
    });

    assert!(matches!(result, Err(PinaPodError::Overflow)));
    assert_eq!(bytes, [0; RequiresInitialization::SIZE]);
}

#[test]
fn initialization_zeroes_the_destination_after_validation_errors() {
    let mut bytes = [0xff; RequiresInitialization::SIZE];

    let result = RequiresInitialization::initialize(&mut bytes, |_value| Ok(()));

    assert!(matches!(result, Err(PinaPodError::InvalidDiscriminant)));
    assert_eq!(bytes, [0; RequiresInitialization::SIZE]);
}
