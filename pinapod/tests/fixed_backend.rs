#![allow(
    unsafe_code,
    unused_qualifications,
    reason = "the derive macro emits audited zero-copy code and these upstream layout assertions intentionally spell out trait paths"
)]

use pinapod::{pod::*, ZeroPod, ZeroPodFixed};

#[allow(dead_code)]
#[derive(ZeroPod)]
struct Simple {
    pub value: u64,
    pub flag: bool,
}

#[test]
fn fixed_size() {
    assert_eq!(<Simple as pinapod::ZeroPodFixed>::SIZE, 8 + 1);
}

#[test]
fn fixed_alignment() {
    assert_eq!(
        core::mem::align_of::<<Simple as pinapod::ZeroPodFixed>::Zc>(),
        1
    );
}

#[test]
fn fixed_read() {
    let mut buf = [0u8; 9];
    buf[0..8].copy_from_slice(&42u64.to_le_bytes());
    buf[8] = 1;
    let zc = Simple::from_bytes(&buf).unwrap();
    assert_eq!(zc.value.get(), 42);
    assert!(zc.flag.get());
}

#[test]
fn fixed_write() {
    let mut buf = [0u8; 9];
    let zc = Simple::from_bytes_mut(&mut buf).unwrap();
    zc.value = 100.into();
    zc.flag = true.into();
    assert_eq!(buf[0..8], 100u64.to_le_bytes());
    assert_eq!(buf[8], 1);
}

#[test]
fn fixed_validate_bad_bool() {
    let mut buf = [0u8; 9];
    buf[8] = 2;
    assert!(Simple::from_bytes(&buf).is_err());
}

#[test]
fn fixed_buffer_too_small() {
    let buf = [0u8; 4];
    assert!(Simple::from_bytes(&buf).is_err());
}

#[test]
fn fixed_unchecked() {
    let buf = [0u8; 9];
    let zc = unsafe { Simple::from_bytes_unchecked(&buf) };
    assert_eq!(zc.value.get(), 0);
}

#[test]
fn fixed_mut_unchecked() {
    let mut buf = [0u8; 9];
    let zc = unsafe { Simple::from_bytes_mut_unchecked(&mut buf) };
    zc.value = PodU64::from(42);
    assert_eq!(&buf[..8], &42u64.to_le_bytes());
}

#[allow(dead_code)]
#[derive(ZeroPod)]
struct WithCollections {
    pub authority: [u8; 32],
    pub name: pinapod::String<32>,
    pub scores: pinapod::Vec<u8, 10>,
    pub active: bool,
    pub maybe: Option<u64>,
}

#[allow(dead_code)]
#[derive(ZeroPod)]
struct WideOptions {
    pub authority: PodOption<[u8; 32], 4>,
    pub amount: PodOption<PodU64, 4>,
}

#[test]
fn fixed_collections_size() {
    // [u8;32](32) + PodString<32,1>(1+32=33) + PodVec<u8,10,2>(2+10=12) +
    // PodBool(1) + PodOption<PodU64>(1+8=9) = 87
    assert_eq!(<WithCollections as pinapod::ZeroPodFixed>::SIZE, 87);
}

#[test]
fn fixed_collections_alignment() {
    assert_eq!(
        core::mem::align_of::<<WithCollections as pinapod::ZeroPodFixed>::Zc>(),
        1
    );
}

#[test]
fn fixed_pfx4_pod_option_accessors_borrow() {
    let mut buf = vec![0u8; <WideOptions as pinapod::ZeroPodFixed>::SIZE];
    let authority = [7u8; 32];

    {
        let zc = WideOptions::from_bytes_mut(&mut buf).unwrap();
        zc.authority = PodOption::<[u8; 32], 4>::some(authority);
        zc.amount = PodOption::<PodU64, 4>::some(PodU64::from(123));
    }

    let zc = WideOptions::from_bytes(&buf).unwrap();
    assert_eq!(zc.authority(), Some(&authority));
    assert_eq!(zc.amount().map(|value| value.get()), Some(123));
}

#[test]
fn fixed_string_field() {
    let mut buf = [0u8; 87];
    let zc = WithCollections::from_bytes_mut(&mut buf).unwrap();
    let _ = zc.name.set("hello");
    assert_eq!(zc.name.as_str(), "hello");
}

#[test]
fn fixed_vec_field() {
    let mut buf = [0u8; 87];
    let zc = WithCollections::from_bytes_mut(&mut buf).unwrap();
    let _ = zc.scores.push(1);
    let _ = zc.scores.push(2);
    assert_eq!(zc.scores.as_slice(), &[1, 2]);
}

#[test]
fn fixed_option_field() {
    let mut buf = [0u8; 87];
    let zc = WithCollections::from_bytes_mut(&mut buf).unwrap();
    assert!(zc.maybe.is_none());
    zc.maybe.set(Some(PodU64::from(42u64)));
    assert_eq!(zc.maybe.get(), Some(PodU64::from(42u64)));
}

#[test]
fn fixed_zc_is_zc_elem() {
    fn assert_zc_elem<T: pinapod::ZcElem>() {}
    assert_zc_elem::<SimpleZc>();
}
