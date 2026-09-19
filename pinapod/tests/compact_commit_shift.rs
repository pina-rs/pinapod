//! Regression tests for a data-corruption bug in the compact-layout
//! `Patch::update` path: when an edited tail field grew, the in-place write of
//! the new value clobbered the source bytes of later unedited tail fields
//! before they were relocated.

use pinapod::PinaPod;
use pinapod::PinaPodError;
use pinapod::pod::PodU16;
use pinapod::pod::PodU64;

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

#[test]
fn compact_patch_oversized_grow_returns_buffer_too_small_without_mutating() {
	let mut buf = vec![0u8; 300];
	let tag = [0xDDu8; 32];

	let tags = [tag];
	let patch = ProfilePatch::new().bio("hi").replace_tags(&tags);
	let committed_size = Profile::initialize(&mut buf, &patch).unwrap();

	buf.truncate(committed_size);
	let snapshot = buf.clone();

	{
		// `String<64>` caps UTF-8 length at 64; stay within that while still
		// requiring more total bytes than the minimally-sized account slice.
		let long_bio = "y".repeat(48);
		let patch = ProfilePatch::new().bio(&long_bio);

		assert_eq!(
			Profile::update(&mut buf, &patch),
			Err(PinaPodError::BufferTooSmall),
			"grow must not clobber the buffer when the account is too small"
		);
	}

	assert_eq!(
		buf, snapshot,
		"failed update must leave the serialized account unchanged"
	);

	let view = Profile::read_prefix(&buf).unwrap();
	assert_eq!(view.bio(), "hi");
	assert_eq!(view.tags().len(), 1);
	assert_eq!(view.tags()[0], tag);
}

#[test]
fn compact_patch_retry_after_a_rejected_update_still_commits() {
	let mut buf = vec![0u8; 300];
	let tag = [0xEEu8; 32];

	let tags = [tag];
	let initial = ProfilePatch::new().bio("seed").replace_tags(&tags);
	let committed_size = Profile::initialize(&mut buf, &initial).unwrap();
	buf.truncate(committed_size);
	let snapshot = buf.clone();

	let long_bio = "y".repeat(48);
	assert_eq!(
		Profile::update(&mut buf, &ProfilePatch::new().bio(&long_bio)),
		Err(PinaPodError::BufferTooSmall)
	);
	assert_eq!(
		buf, snapshot,
		"the rejected update must not mutate anything"
	);

	// The account must remain fully usable after the rejection: a patch that
	// fits commits cleanly and preserves every unedited tail.
	let new_size = Profile::update(&mut buf, &ProfilePatch::new().bio("ok")).unwrap();
	assert!(new_size <= buf.len());
	let view = Profile::read_prefix(&buf).unwrap();
	assert_eq!(view.bio(), "ok");
	assert_eq!(view.tags().len(), 1);
	assert_eq!(view.tags()[0], tag);
}

#[test]
fn compact_patch_bio_grow_preserves_tags() {
	let mut buf = vec![0u8; 300];
	let tag = [0xCCu8; 32];

	let tags = [tag];
	let initial = ProfilePatch::new().bio("hi").replace_tags(&tags);
	Profile::initialize(&mut buf, &initial).unwrap();

	{
		let patch = ProfilePatch::new().bio("much longer bio text here!");
		let new_size = Profile::update(&mut buf, &patch).unwrap();

		let view = Profile::read_prefix(&buf[..new_size]).unwrap();
		assert_eq!(view.bio(), "much longer bio text here!");
		assert_eq!(view.tags().len(), 1);
		assert_eq!(view.tags()[0], [0xCCu8; 32]);
	}
}

#[test]
fn compact_patch_bio_grow_preserves_multiple_tags() {
	let mut buf = vec![0u8; Profile::MAX_SIZE];
	let t1 = [0x11u8; 32];
	let t2 = [0x22u8; 32];
	let t3 = [0x33u8; 32];

	let tags = [t1, t2, t3];
	let initial = ProfilePatch::new().bio("x").replace_tags(&tags);
	Profile::initialize(&mut buf, &initial).unwrap();

	let long_bio = "z".repeat(60);
	{
		let patch = ProfilePatch::new().bio(&long_bio);
		let new_size = Profile::update(&mut buf, &patch).unwrap();

		let view = Profile::read_prefix(&buf[..new_size]).unwrap();
		assert_eq!(view.bio(), long_bio);
		assert_eq!(view.tags().len(), 3);
		assert_eq!(view.tags()[0], t1);
		assert_eq!(view.tags()[1], t2);
		assert_eq!(view.tags()[2], t3);
	}
}

#[test]
fn compact_empty_patch_preserves_all_tails() {
	let mut buf = vec![0u8; 300];
	let tag = [0xABu8; 32];

	let tags = [tag];
	let initial = ProfilePatch::new().bio("hello there").replace_tags(&tags);
	Profile::initialize(&mut buf, &initial).unwrap();

	{
		let new_size = Profile::update(&mut buf, &ProfilePatch::new()).unwrap();

		let view = Profile::read_prefix(&buf[..new_size]).unwrap();
		assert_eq!(view.bio(), "hello there");
		assert_eq!(view.tags().len(), 1);
		assert_eq!(view.tags()[0], tag);
	}
}

#[test]
fn compact_patch_grow_then_shrink_then_grow() {
	let mut buf = vec![0u8; Profile::MAX_SIZE];
	let tag = [0x7Fu8; 32];

	let tags = [tag];
	let initial = ProfilePatch::new().bio("a").replace_tags(&tags);
	Profile::initialize(&mut buf, &initial).unwrap();

	for bio in [
		"aaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		"b",
		"cccccccccccccccccccccccccccccccccccccccccc",
	] {
		Profile::update(&mut buf, &ProfilePatch::new().bio(bio)).unwrap();

		{
			let profile = Profile::read_prefix(&buf).unwrap();
			assert_eq!(profile.bio(), bio);
			assert_eq!(profile.tags().len(), 1);
			assert_eq!(profile.tags()[0], tag);
		}
	}
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct MultiTail {
	pub id: u32,
	pub a: pinapod::String<32>,
	pub b: pinapod::String<32>,
	pub c: pinapod::Vec<u8, 32>,
}

#[test]
fn compact_patch_grow_first_preserves_middle_and_last() {
	let mut buf = vec![0u8; MultiTail::MAX_SIZE];

	let initial = MultiTailPatch::new()
		.a("a")
		.b("bbbbb")
		.replace_c(&[1, 2, 3, 4, 5, 6, 7]);
	MultiTail::initialize(&mut buf, &initial).unwrap();

	{
		let patch = MultiTailPatch::new().a("aaaaaaaaaaaaaaaaaaaaaaaaaaaa");
		let new_size = MultiTail::update(&mut buf, &patch).unwrap();

		let view = MultiTail::read_prefix(&buf[..new_size]).unwrap();
		assert_eq!(view.a(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaa");
		assert_eq!(view.b(), "bbbbb");
		assert_eq!(view.c(), &[1, 2, 3, 4, 5, 6, 7]);
	}
}

#[test]
fn compact_patch_mixed_grow_and_shrink_preserves_unedited_last() {
	let mut buf = vec![0u8; MultiTail::MAX_SIZE];

	let initial = MultiTailPatch::new()
		.a("aaaaaaaaaa")
		.b("bbbbb")
		.replace_c(&[9, 8, 7, 6, 5]);
	MultiTail::initialize(&mut buf, &initial).unwrap();

	{
		let patch = MultiTailPatch::new()
			.a("aa")
			.b("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
		let new_size = MultiTail::update(&mut buf, &patch).unwrap();

		let view = MultiTail::read_prefix(&buf[..new_size]).unwrap();
		assert_eq!(view.a(), "aa");
		assert_eq!(view.b(), "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
		assert_eq!(view.c(), &[9, 8, 7, 6, 5]);
	}
}

#[test]
fn compact_patch_last_tail_edits_preserve_large_earlier_tails() {
	let mut buf = vec![0u8; MultiTail::MAX_SIZE];

	let a = "a".repeat(30);
	let b = "b".repeat(30);
	let initial = MultiTailPatch::new().a(&a).b(&b).replace_c(&[7; 32]);
	MultiTail::initialize(&mut buf, &initial).unwrap();

	// Grow, shrink, and empty the last tail while both earlier tails sit at
	// nearly full capacity: every relocation source lies behind the edit.
	for c in [&[9u8; 32][..], &[3u8; 4][..], &[][..]] {
		let new_size = MultiTail::update(&mut buf, &MultiTailPatch::new().replace_c(c)).unwrap();

		let view = MultiTail::read_prefix(&buf[..new_size]).unwrap();
		assert_eq!(view.a(), a);
		assert_eq!(view.b(), b);
		assert_eq!(view.c(), c);
	}
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct MultiVecTail {
	pub values: pinapod::Vec<u64, 8>,
	pub codes: pinapod::Vec<u16, 8>,
}

#[test]
fn compact_ref_uses_independent_vec_tail_lengths() {
	let mut buf = vec![0u8; MultiVecTail::MAX_SIZE];
	let values = [PodU64::from(11), PodU64::from(22), PodU64::from(33)];
	let codes = [PodU16::from(5), PodU16::from(8)];

	let patch = MultiVecTailPatch::new()
		.replace_values(&values)
		.replace_codes(&codes);
	let encoded_size = MultiVecTail::initialize(&mut buf, &patch).unwrap();

	let value = MultiVecTail::read_prefix(&buf[..encoded_size]).unwrap();
	assert_eq!(value.values().len(), values.len());
	assert_eq!(value.values(), values);
	assert_eq!(value.codes().len(), codes.len());
	assert_eq!(value.codes(), codes);
}
