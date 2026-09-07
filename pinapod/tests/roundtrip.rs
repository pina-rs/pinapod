use pinapod::{pod::*, PinaPod};

// --- Fixed roundtrip ---

#[allow(dead_code)]
#[derive(PinaPod)]
struct RoundtripFixed {
    pub amount: u64,
    pub flag: bool,
    pub tag: u8,
    pub name: pinapod::String<16>,
    pub scores: pinapod::Vec<u8, 8>,
    pub maybe: Option<u64>,
}

// Layout: PodU64(8) + PodBool(1) + u8(1) + PodString<16,1>(17) +
// PodVec<u8,8,2>(10) + PodOption<PodU64>(9) = 46

#[test]
fn fixed_roundtrip_write_then_read() {
    let mut buf = [0u8; 46];

    // Write
    {
        let zc = RoundtripFixed::read_exact_mut(&mut buf).unwrap();
        zc.amount = 1_000_000u64.into();
        zc.flag = true.into();
        zc.tag = 42;
        zc.name.try_set("alice").unwrap();
        zc.scores.try_push(10).unwrap();
        zc.scores.try_push(20).unwrap();
        zc.scores.try_push(30).unwrap();
        zc.maybe.set(Some(PodU64::from(999u64)));
    }

    // Read back
    let zc = RoundtripFixed::read_exact(&buf).unwrap();
    assert_eq!(zc.amount.get(), 1_000_000);
    assert!(zc.flag.get());
    assert_eq!(zc.tag, 42);
    assert_eq!(zc.name.as_str(), "alice");
    assert_eq!(zc.scores.as_slice(), &[10, 20, 30]);
    assert_eq!(zc.maybe.get(), Some(PodU64::from(999u64)));
}

#[test]
fn fixed_byte_stability() {
    let mut buf1 = [0u8; 46];
    let mut buf2 = [0u8; 46];

    for buf in [&mut buf1, &mut buf2] {
        let zc = RoundtripFixed::read_exact_mut(buf).unwrap();
        zc.amount = 42u64.into();
        zc.flag = false.into();
        zc.tag = 7;
        zc.name.try_set("bob").unwrap();
        zc.scores.try_push(1).unwrap();
        zc.maybe.set(None);
    }

    assert_eq!(buf1, buf2, "identical writes must produce identical bytes");
}

// --- Compact roundtrip ---

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct RoundtripCompact {
    pub authority: [u8; 32],
    pub level: u64,
    pub bio: pinapod::String<64>,
    pub tags: pinapod::Vec<[u8; 4], 10>,
}

// Header: authority(32) + PodU64(8) + bio_len(1) + tags_len(2) = 43

#[test]
fn compact_roundtrip_write_then_read() {
    let mut buf = vec![0u8; RoundtripCompact::MAX_SIZE];

    let auth = [0xAA; 32];
    let tag1 = [1u8, 2, 3, 4];
    let tag2 = [5u8, 6, 7, 8];

    let tags = [tag1, tag2];
    let patch = RoundtripCompactPatch::new()
        .authority(auth)
        .level(50u64)
        .bio("hello world")
        .replace_tags(&tags);
    RoundtripCompact::initialize(&mut buf, &patch).unwrap();

    // Read back
    let r = RoundtripCompact::read_prefix(&buf).unwrap();
    assert_eq!(r.authority, [0xAA; 32]);
    assert_eq!(r.level.get(), 50);
    assert_eq!(r.bio(), "hello world");
    assert_eq!(r.tags().len(), 2);
    assert_eq!(r.tags()[0], [1, 2, 3, 4]);
    assert_eq!(r.tags()[1], [5, 6, 7, 8]);
}

#[test]
fn compact_overwrite_shorter_roundtrip() {
    let mut buf = vec![0u8; RoundtripCompact::MAX_SIZE];

    let initial = RoundtripCompactPatch::new().bio("a long biography text");
    RoundtripCompact::initialize(&mut buf, &initial).unwrap();

    let new_size =
        RoundtripCompact::update(&mut buf, &RoundtripCompactPatch::new().bio("hi")).unwrap();

    // Read back — must see the shorter value, not the old one
    let r = RoundtripCompact::read_prefix(&buf[..new_size]).unwrap();
    assert_eq!(r.bio(), "hi");
}
