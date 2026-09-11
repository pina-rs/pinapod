#![allow(
    unsafe_code,
    unused_qualifications,
    reason = "the invalid-element regression constructs an audited raw PodBool and layout assertions intentionally spell out trait paths"
)]

use pinapod::{PinaPod, PinaPodCompact};

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct Profile {
    pub authority: [u8; 32],
    pub level: u64,
    pub active: bool,
    pub bio: pinapod::String<64>,
    pub tags: pinapod::Vec<[u8; 32], 20>,
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct ConstTailProfile<const BIO_CAP: usize, const TAG_CAP: usize> {
    pub bio: pinapod::String<BIO_CAP>,
    pub tags: pinapod::Vec<[u8; 4], TAG_CAP>,
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct OptionalTailProfile {
    pub nickname: Option<pinapod::String<8>>,
    pub tags: Option<pinapod::Vec<[u8; 4], 3>>,
    pub note: pinapod::String<8>,
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct StringVectorProfile {
    pub names: pinapod::Vec<pinapod::String<8>, 3>,
    pub note: pinapod::String<8>,
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct ValidatedVectorProfile {
    pub flags: pinapod::Vec<pinapod::pod::PodBool, 3>,
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct WidePrefixProfile {
    pub values: pinapod::PodVec<u64, 4, 8>,
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct WidePrefixOptionalProfile {
    pub note: Option<pinapod::PodString<8, 8>>,
}

#[allow(dead_code)]
#[derive(PinaPod)]
struct FixedEventPayload {
    pub amount: u64,
    pub enabled: bool,
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct OptionalInlineProfile {
    pub revision: Option<u64>,
    pub event: Option<FixedEventPayload>,
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
#[repr(u8)]
enum CompactEvent {
    Empty = 0,
    Label(pinapod::String<8>) = 1,
    Points(pinapod::Vec<u16, 3>) = 2,
    Fixed(FixedEventPayload) = 3,
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
#[repr(u16)]
enum WideCompactEvent {
    Empty = 0,
    Label(pinapod::String<8>) = 300,
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
#[repr(u8)]
enum ValidatedCompactEvent {
    Empty = 0,
    Flags(pinapod::Vec<pinapod::pod::PodBool, 3>) = 1,
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
#[repr(u8)]
enum WidePrefixCompactEvent {
    Empty = 0,
    Values(pinapod::PodVec<u64, 4, 8>) = 1,
}

// --- Header tests ---

#[test]
fn compact_header_size() {
    // authority(32) + PodU64(8) + PodBool(1) + bio_len(1, PFX=1) + tags_len(2,
    // PFX=2) = 44
    assert_eq!(<Profile as pinapod::PinaPodCompact>::HEADER_SIZE, 44);
}

#[test]
fn compact_header_alignment() {
    assert_eq!(
        core::mem::align_of::<<Profile as pinapod::PinaPodCompact>::Header>(),
        1
    );
}

#[test]
fn compact_option_patch_accepts_native_options_and_stored_representations() {
    let mut buf = [0_u8; OptionalInlineProfile::MAX_SIZE];
    let event = FixedEventPayloadZc {
        amount: 21_u64.into(),
        enabled: true.into(),
    };
    let patch = OptionalInlineProfilePatch::new()
        .revision(Some(13_u64))
        .event(pinapod::pod::PodOption::some(event));

    let encoded_len = OptionalInlineProfile::initialize(&mut buf, &patch).unwrap();
    let profile = OptionalInlineProfile::read_prefix(&buf[..encoded_len]).unwrap();

    assert_eq!(profile.revision.get().map(|value| value.get()), Some(13));
    assert_eq!(
        profile
            .event
            .get_ref()
            .map(|value| (value.amount.get(), value.enabled.get())),
        Some((21, true)),
    );

    let patch = OptionalInlineProfilePatch::new().revision(None);
    OptionalInlineProfile::update(&mut buf, &patch).unwrap();
    let profile = OptionalInlineProfile::read_prefix(&buf).unwrap();

    assert!(profile.revision.is_none());
    assert!(profile.event.is_some());
}

#[test]
fn compact_const_generic_tail_capacity() {
    assert_eq!(
        <ConstTailProfile<5, 2> as pinapod::PinaPodCompact>::HEADER_SIZE,
        3
    );

    let mut buf = vec![0u8; ConstTailProfile::<5, 2>::MAX_SIZE];
    let tags = [[1; 4], [2; 4]];
    let patch = ConstTailProfilePatch::<5, 2>::new()
        .bio("hello")
        .replace_tags(&tags);
    let new_size = ConstTailProfile::<5, 2>::initialize(&mut buf, &patch).unwrap();
    assert_eq!(new_size, 3 + 5 + 8);

    {
        let profile = ConstTailProfile::<5, 2>::read_prefix(&buf).unwrap();
        assert_eq!(profile.bio(), "hello");
        assert_eq!(profile.tags(), &[[1; 4], [2; 4]]);
    }

    assert!(ConstTailProfilePatch::<5, 2>::new()
        .bio("too long")
        .updated_len(&buf)
        .is_err());
    assert!(ConstTailProfilePatch::<5, 2>::new()
        .replace_tags(&[[0; 4], [1; 4], [2; 4]])
        .updated_len(&buf)
        .is_err());
}

#[test]
fn compact_optional_dynamic_tails_store_only_active_payloads() {
    assert_eq!(
        <OptionalTailProfile as pinapod::PinaPodCompact>::HEADER_SIZE,
        3
    );

    let mut buf = vec![0u8; OptionalTailProfile::MAX_SIZE];
    {
        let tags = [[1; 4], [2; 4]];
        let patch = OptionalTailProfilePatch::new()
            .nickname(None)
            .replace_tags(Some(&tags))
            .note("ok");
        let new_size = OptionalTailProfile::initialize(&mut buf, &patch).unwrap();

        assert_eq!(new_size, 3 + 2 + 8 + 2);
        assert_eq!(&buf[..3], &[0, 1, 2]);
        assert_eq!(&buf[3..5], &[2, 0]);
        assert_eq!(&buf[5..13], &[1, 1, 1, 1, 2, 2, 2, 2]);
        assert_eq!(&buf[13..15], b"ok");
    }

    let profile = OptionalTailProfile::read_prefix(&buf[..15]).unwrap();
    assert_eq!(profile.nickname(), None);
    assert_eq!(profile.tags(), Some(&[[1; 4], [2; 4]][..]));
    assert_eq!(profile.note(), "ok");

    let mut none_only = vec![0u8; 3];
    {
        let patch = OptionalTailProfilePatch::new()
            .nickname(None)
            .replace_tags(None);
        let new_size = OptionalTailProfile::initialize(&mut none_only, &patch).unwrap();
        assert_eq!(new_size, 3);
        assert_eq!(&none_only, &[0, 0, 0]);
    }
}

#[test]
fn compact_optional_dynamic_tails_validate_tags_and_payloads() {
    let mut bad_tag = vec![0u8; 3];
    bad_tag[0] = 2;
    assert_eq!(
        OptionalTailProfile::validate(&bad_tag),
        Err(pinapod::PinaPodError::InvalidTag)
    );

    let missing_payload_prefix = [1u8, 0, 0];
    assert_eq!(
        OptionalTailProfile::validate(&missing_payload_prefix),
        Err(pinapod::PinaPodError::BufferTooSmall)
    );

    let mut overlong = vec![0u8; 4];
    overlong[0] = 1;
    overlong[3] = 9;
    assert_eq!(
        OptionalTailProfile::validate(&overlong),
        Err(pinapod::PinaPodError::InvalidLength)
    );
}

#[test]
fn compact_string_vectors_are_fixed_stride_and_validate_each_string() {
    let mut ada = pinapod::String::<8>::default();
    ada.try_set("Ada").unwrap();
    let mut grace = pinapod::String::<8>::default();
    grace.try_set("Grace").unwrap();
    let names = [ada, grace];
    let mut buffer = [0u8; StringVectorProfile::MAX_SIZE];

    let patch = StringVectorProfilePatch::new()
        .replace_names(&names)
        .note("ok");
    let encoded_len = StringVectorProfile::initialize(&mut buffer, &patch).unwrap();

    assert_eq!(
        encoded_len,
        3 + 2 * core::mem::size_of::<pinapod::String<8>>() + 2
    );
    let profile = StringVectorProfile::read_prefix(&buffer[..encoded_len]).unwrap();
    assert_eq!(profile.names()[0].as_str(), "Ada");
    assert_eq!(profile.names()[1].as_str(), "Grace");
    assert_eq!(profile.note(), "ok");

    // Header is names count (u16) + note length (u8). The first element
    // begins at byte 3 and retains its own one-byte string length prefix.
    buffer[3] = 9;
    assert_eq!(
        StringVectorProfile::validate(&buffer[..encoded_len]),
        Err(pinapod::PinaPodError::InvalidLength)
    );
}

#[test]
fn compact_patches_validate_elements_before_writing() {
    let invalid_byte = 2u8;
    let invalid = unsafe { &*((&raw const invalid_byte).cast::<pinapod::pod::PodBool>()) };
    let mut buffer = [0u8; ValidatedVectorProfile::MAX_SIZE];

    let patch = ValidatedVectorProfilePatch::new().replace_flags(core::slice::from_ref(invalid));
    assert_eq!(
        ValidatedVectorProfile::update(&mut buffer, &patch),
        Err(pinapod::PinaPodError::InvalidBool)
    );
    assert!(buffer.iter().all(|byte| *byte == 0));

    let mut event_buffer = [0u8; ValidatedCompactEvent::MAX_SIZE];
    let event_snapshot = event_buffer;
    let event_patch = ValidatedCompactEventPatch::Flags(core::slice::from_ref(invalid));

    assert_eq!(
        ValidatedCompactEvent::update(&mut event_buffer, &event_patch),
        Err(pinapod::PinaPodError::InvalidBool)
    );
    assert_eq!(event_buffer, event_snapshot);
}

#[test]
fn compact_tagged_union_stores_only_active_variant_payload() {
    assert_eq!(<CompactEvent as pinapod::PinaPodCompact>::HEADER_SIZE, 1);

    let label = [1u8, 2, b'o', b'k'];
    match CompactEvent::read_prefix(&label).unwrap() {
        CompactEventRef::Label(value) => assert_eq!(value, "ok"),
        _ => panic!("expected label variant"),
    }

    let points = [2u8, 2, 0, 5, 0, 7, 0];
    match CompactEvent::read_prefix(&points).unwrap() {
        CompactEventRef::Points(values) => {
            assert_eq!(values.len(), 2);
            assert_eq!(values[0].get(), 5);
            assert_eq!(values[1].get(), 7);
        }
        _ => panic!("expected points variant"),
    }

    let mut fixed = vec![3u8; 1 + FixedEventPayload::SIZE];
    fixed[1..9].copy_from_slice(&9u64.to_le_bytes());
    fixed[9] = 1;
    match CompactEvent::read_prefix(&fixed).unwrap() {
        CompactEventRef::Fixed(value) => {
            assert_eq!(value.amount.get(), 9);
            assert!(value.enabled.get());
        }
        _ => panic!("expected fixed variant"),
    }
}

#[test]
fn compact_tagged_union_patches_between_variant_shapes() {
    let mut buf = vec![0u8; CompactEvent::MAX_SIZE];
    let label_patch = CompactEventPatch::Label("hello");
    assert_eq!(label_patch.updated_len(&buf).unwrap(), 1 + 1 + 5);
    assert_eq!(
        CompactEvent::initialize(&mut buf, &label_patch).unwrap(),
        1 + 1 + 5
    );
    assert_eq!(&buf[..7], &[1, 5, b'h', b'e', b'l', b'l', b'o']);

    let empty_patch = CompactEventPatch::Empty;
    assert_eq!(empty_patch.updated_len(&buf).unwrap(), 1);
    assert_eq!(CompactEvent::update(&mut buf, &empty_patch).unwrap(), 1);
    assert_eq!(buf[0], 0);
    assert!(buf[1..7].iter().all(|byte| *byte == 0));
    assert!(matches!(
        CompactEvent::read_prefix(&buf[..1]).unwrap(),
        CompactEventRef::Empty
    ));

    let points = [5u16.into(), 7u16.into(), 9u16.into()];
    let points_patch = CompactEventPatch::Points(&points);
    assert_eq!(points_patch.updated_len(&buf).unwrap(), 1 + 2 + 6);
    assert_eq!(
        CompactEvent::update(&mut buf, &points_patch).unwrap(),
        1 + 2 + 6
    );

    match CompactEvent::read_prefix(&buf[..9]).unwrap() {
        CompactEventRef::Points(values) => {
            assert_eq!(values.len(), 3);
            assert_eq!(values[2].get(), 9);
        }
        _ => panic!("expected points variant"),
    }
}

#[test]
fn compact_tagged_union_patches_fixed_payloads() {
    let mut buf = vec![0u8; CompactEvent::MAX_SIZE];

    let mut fixed_buf = vec![0u8; FixedEventPayload::SIZE];
    let fixed = FixedEventPayload::read_exact_mut(&mut fixed_buf).unwrap();
    fixed.amount = 42u64.into();
    fixed.enabled = true.into();

    let fixed_patch = CompactEventPatch::Fixed(fixed);
    assert_eq!(
        fixed_patch.updated_len(&buf).unwrap(),
        1 + FixedEventPayload::SIZE
    );
    assert_eq!(
        CompactEvent::initialize(&mut buf, &fixed_patch).unwrap(),
        1 + FixedEventPayload::SIZE
    );

    match CompactEvent::read_prefix(&buf[..10]).unwrap() {
        CompactEventRef::Fixed(value) => {
            assert_eq!(value.amount.get(), 42);
            assert!(value.enabled.get());
        }
        _ => panic!("expected fixed variant"),
    }
}

#[test]
fn compact_tagged_union_patch_rejects_invalid_values_and_capacity_atomically() {
    let mut buf = vec![0u8; 4];
    let snapshot = buf.clone();

    assert_eq!(
        CompactEvent::update(&mut buf, &CompactEventPatch::Label("too-long!")),
        Err(pinapod::PinaPodError::Overflow)
    );
    assert_eq!(buf, snapshot);

    let too_many_points = [1u16.into(), 2u16.into(), 3u16.into(), 4u16.into()];
    assert_eq!(
        CompactEvent::update(&mut buf, &CompactEventPatch::Points(&too_many_points)),
        Err(pinapod::PinaPodError::Overflow)
    );
    assert_eq!(buf, snapshot);

    assert_eq!(
        CompactEvent::update(&mut buf, &CompactEventPatch::Label("abcd")),
        Err(pinapod::PinaPodError::BufferTooSmall)
    );
    assert_eq!(buf, snapshot);
}

#[test]
fn compact_tagged_union_honors_wide_tags() {
    assert_eq!(
        <WideCompactEvent as pinapod::PinaPodCompact>::HEADER_SIZE,
        2
    );

    let mut buf = vec![0u8; WideCompactEvent::MAX_SIZE];
    let patch = WideCompactEventPatch::Label("hi");
    assert_eq!(
        WideCompactEvent::initialize(&mut buf, &patch).unwrap(),
        2 + 1 + 2
    );

    assert_eq!(&buf[..5], &[44, 1, 2, b'h', b'i']);
    match WideCompactEvent::read_prefix(&buf[..5]).unwrap() {
        WideCompactEventRef::Label(value) => assert_eq!(value, "hi"),
        WideCompactEventRef::Empty => panic!("expected label variant"),
    }

    assert_eq!(
        WideCompactEvent::validate(&[1, 0]),
        Err(pinapod::PinaPodError::InvalidDiscriminant)
    );
}

#[test]
fn compact_tagged_union_rejects_invalid_tags_and_payloads() {
    assert_eq!(
        CompactEvent::validate(&[9]),
        Err(pinapod::PinaPodError::InvalidDiscriminant)
    );
    assert_eq!(
        CompactEvent::validate(&[1, 9, b'o', b'v', b'e', b'r']),
        Err(pinapod::PinaPodError::InvalidLength)
    );
    assert_eq!(
        CompactEvent::validate(&[2, 4, 0, 1, 0, 2, 0, 3, 0, 4, 0]),
        Err(pinapod::PinaPodError::InvalidLength)
    );
    assert_eq!(
        CompactEvent::validate(&[3, 0, 0, 0]),
        Err(pinapod::PinaPodError::BufferTooSmall)
    );
}

// --- Ref tests ---

#[test]
fn compact_ref_inline_via_deref() {
    let buf = vec![0u8; 100];
    let profile = Profile::read_prefix(&buf).unwrap();
    assert_eq!(profile.level.get(), 0);
    assert!(!profile.active.get());
}

#[test]
fn compact_ref_empty_tails() {
    let buf = vec![0u8; 100];
    let profile = Profile::read_prefix(&buf).unwrap();
    assert_eq!(profile.bio(), "");
    assert_eq!(profile.tags().len(), 0);
}

#[test]
fn compact_ref_bio_with_data() {
    let mut buf = vec![0u8; 100];
    // bio_len is at offset 41 (32+8+1), PFX=1
    buf[41] = 5;
    // bio data at offset 44 (header size)
    buf[44..49].copy_from_slice(b"hello");
    let profile = Profile::read_prefix(&buf).unwrap();
    assert_eq!(profile.bio(), "hello");
}

#[test]
fn compact_ref_tags_with_data() {
    let mut buf = vec![0u8; 200];
    // bio_len = 0 (offset 41)
    // tags_len at offset 42-43, PFX=2
    buf[42] = 1;
    buf[43] = 0; // 1 tag
                 // tags data at offset 44 (header) + 0 (bio empty) = 44
    buf[44..76].copy_from_slice(&[0xAA; 32]);
    let profile = Profile::read_prefix(&buf).unwrap();
    assert_eq!(profile.tags().len(), 1);
    assert_eq!(profile.tags()[0], [0xAA; 32]);
}

// --- Validation tests ---

#[test]
fn compact_validate_overlength_bio() {
    let mut buf = vec![0u8; 200];
    buf[41] = 65; // bio_len=65 > max 64
    assert!(Profile::validate(&buf).is_err());
}

#[test]
fn compact_validate_tail_overflow() {
    let mut buf = vec![0u8; 50]; // header(44) + only 6 bytes
    buf[41] = 10; // bio_len=10, needs 44+10=54
    assert!(Profile::validate(&buf).is_err());
}

#[test]
fn compact_struct_rejects_malicious_eight_byte_prefix() {
    let data = u64::MAX.to_le_bytes();

    assert_eq!(
        WidePrefixProfile::validate(&data),
        Err(pinapod::PinaPodError::InvalidLength)
    );
    assert!(matches!(
        WidePrefixProfile::read_prefix(&data),
        Err(pinapod::PinaPodError::InvalidLength)
    ));
}

#[test]
fn compact_enum_rejects_malicious_eight_byte_prefix() {
    let mut data = [0xFF; 9];
    data[0] = 1;

    assert_eq!(
        WidePrefixCompactEvent::validate(&data),
        Err(pinapod::PinaPodError::InvalidLength)
    );
    assert!(matches!(
        WidePrefixCompactEvent::read_prefix(&data),
        Err(pinapod::PinaPodError::InvalidLength)
    ));
}

#[test]
fn compact_optional_tail_with_eight_byte_prefix_roundtrips() {
    // tag (1) + length prefix (8) + string capacity (8)
    let mut buffer = vec![0u8; 17];
    let patch = WidePrefixOptionalProfilePatch::new().note(Some("hello"));
    let encoded = WidePrefixOptionalProfile::initialize(&mut buffer, &patch).unwrap();
    assert_eq!(encoded, 1 + 8 + 5);

    let view = WidePrefixOptionalProfile::read_prefix(&buffer).unwrap();
    assert_eq!(view.note(), Some("hello"));

    let cleared = WidePrefixOptionalProfilePatch::new().note(None);
    let encoded = WidePrefixOptionalProfile::update(&mut buffer, &cleared).unwrap();
    assert_eq!(encoded, 1);

    let view = WidePrefixOptionalProfile::read_prefix(&buffer).unwrap();
    assert_eq!(view.note(), None);
}

// --- Patch tests ---

#[test]
fn borrowed_compact_patch_uses_the_patch_trait() {
    fn initialize_with_patch<P>(data: &mut [u8], patch: P) -> Result<usize, pinapod::PinaPodError>
    where
        P: pinapod::PinaPodPatch<Profile>,
    {
        <P as pinapod::PinaPodPatch<Profile>>::initialize(&patch, data)
    }

    let mut buf = vec![0u8; Profile::MAX_SIZE];
    let patch = ProfilePatch::new().bio("borrowed");
    let encoded_len = initialize_with_patch(&mut buf, &patch).unwrap();

    assert_eq!(
        Profile::read_prefix(&buf[..encoded_len]).unwrap().bio(),
        "borrowed"
    );
}

#[test]
fn compact_patch_updates_inline_fields() {
    let mut buf = vec![0u8; 200];
    let patch = ProfilePatch::new().level(42u64).active(true);
    Profile::update(&mut buf, &patch).unwrap();

    let profile = Profile::read_prefix(&buf).unwrap();
    assert_eq!(profile.level.get(), 42);
    assert!(profile.active.get());
}

#[test]
fn compact_patch_sets_bio() {
    let mut buf = vec![0u8; 200];
    let patch = ProfilePatch::new().bio("hello world");
    let new_size = Profile::initialize(&mut buf, &patch).unwrap();
    assert_eq!(new_size, 44 + 11);

    let view = Profile::read_prefix(&buf[..new_size]).unwrap();
    assert_eq!(view.bio(), "hello world");
}

#[test]
fn compact_patch_sets_bio_and_tags() {
    let mut buf = vec![0u8; 200];
    let first_tag = [0xAA; 32];
    let second_tag = [0xBB; 32];

    let tags = [first_tag, second_tag];
    let patch = ProfilePatch::new().bio("test").replace_tags(&tags);
    let new_size = Profile::initialize(&mut buf, &patch).unwrap();
    assert_eq!(new_size, 44 + 4 + 64);

    let view = Profile::read_prefix(&buf[..new_size]).unwrap();
    assert_eq!(view.bio(), "test");
    assert_eq!(view.tags().len(), 2);
    assert_eq!(view.tags()[0], [0xAA; 32]);
    assert_eq!(view.tags()[1], [0xBB; 32]);
}

#[test]
fn compact_patch_reports_updated_len() {
    let buf = vec![0u8; 200];
    assert_eq!(ProfilePatch::new().updated_len(&buf).unwrap(), 44);
    assert_eq!(
        ProfilePatch::new().bio("hello").updated_len(&buf).unwrap(),
        44 + 5
    );
}

#[test]
fn compact_patch_overwrite_shorter() {
    let mut buf = vec![0u8; 200];
    Profile::initialize(&mut buf, &ProfilePatch::new().bio("hello world")).unwrap();
    let new_size = Profile::update(&mut buf, &ProfilePatch::new().bio("hi")).unwrap();
    assert_eq!(new_size, 44 + 2);

    let view = Profile::read_prefix(&buf[..46]).unwrap();
    assert_eq!(view.bio(), "hi");
    assert!(buf[46..55].iter().all(|byte| *byte == 0));
}

#[test]
fn compact_patch_overflow_rejected_atomically() {
    let mut buf = vec![0u8; 200];
    let snapshot = buf.clone();
    let long = "x".repeat(65);
    let patch = ProfilePatch::new().bio(&long);

    assert_eq!(
        Profile::update(&mut buf, &patch),
        Err(pinapod::PinaPodError::Overflow)
    );
    assert_eq!(buf, snapshot);
}

#[test]
fn compact_patch_rejects_an_invalid_buffer_before_unchecked_writing() {
    let mut buf = vec![0u8; Profile::MAX_SIZE];
    Profile::initialize(&mut buf, &ProfilePatch::new().bio("before")).unwrap();
    buf[40] = 2;
    let snapshot = buf.clone();

    assert_eq!(
        Profile::update(&mut buf, &ProfilePatch::new().bio("after")),
        Err(pinapod::PinaPodError::InvalidBool)
    );
    assert_eq!(buf, snapshot);
}

#[test]
fn compact_initialize_zeroes_the_destination_after_an_error() {
    let mut buf = vec![0xFF; Profile::MAX_SIZE];
    let long = "x".repeat(65);
    let patch = ProfilePatch::new().bio(&long);

    assert_eq!(
        Profile::initialize(&mut buf, &patch),
        Err(pinapod::PinaPodError::Overflow)
    );
    assert!(buf.iter().all(|byte| *byte == 0));
}

#[test]
fn compact_empty_patch_preserves_unedited_fields() {
    let mut buf = vec![0u8; 200];
    Profile::initialize(&mut buf, &ProfilePatch::new().bio("hello")).unwrap();
    let new_size = Profile::update(&mut buf, &ProfilePatch::new()).unwrap();
    assert_eq!(new_size, 44 + 5);

    let view = Profile::read_prefix(&buf[..49]).unwrap();
    assert_eq!(view.bio(), "hello");
}

#[test]
fn compact_patch_bio_shift_preserves_tags() {
    let mut buf = vec![0u8; 300];
    let tag = [0xCC; 32];

    let tags = [tag];
    let initial = ProfilePatch::new()
        .bio("long bio text here!")
        .replace_tags(&tags);
    Profile::initialize(&mut buf, &initial).unwrap();

    // Now shorten bio. Tags must move but preserve content.
    {
        let patch = ProfilePatch::new().bio("hi");
        let new_size = Profile::update(&mut buf, &patch).unwrap();

        let view = Profile::read_prefix(&buf[..new_size]).unwrap();
        assert_eq!(view.bio(), "hi");
        assert_eq!(view.tags().len(), 1);
        assert_eq!(view.tags()[0], [0xCC; 32]);
    }
}
