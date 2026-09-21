//! Proves the `compact-commit-full-validation` feature selects the depth.
//!
//! Run both ways — the default and `--features compact-commit-full-validation`
//! — because the assertion's branch is chosen by `cfg!(feature = ...)` and a
//! single run only exercises one side.

#![allow(
	dead_code,
	missing_docs,
	reason = "the schema's fields exist to give the commit a tail to interpret; the test reads \
	          the encoded bytes, not the fields"
)]

use pinapod::PinaPod;
use pinapod::traits::commit_entry_validate;

#[derive(PinaPod)]
#[pinapod(compact)]
struct Doc {
	revision: u64,
	title: pinapod::String<8>,
}

#[test]
fn commit_entry_depth_follows_the_feature() {
	let mut buf = vec![0u8; Doc::MAX_SIZE];
	Doc::initialize(&mut buf, &DocPatch::new().title("hi")).unwrap();

	// Corrupt the active title bytes to non-UTF-8 while keeping the layout
	// intact: the prefix still decodes and the tail is still in bounds.
	let header = <Doc as pinapod::PinaPodCompact>::HEADER_SIZE;
	buf[header] = 0xFF;

	assert_eq!(
		<Doc as pinapod::PinaPodCompact>::validate_layout(&buf[..header + 2]),
		Ok(()),
		"layout is sound"
	);

	// The dispatch must match the feature: the layout walk by default
	// (accepts), the full semantic walk when enabled (rejects the non-UTF-8
	// content). Either way the check resolves against PinaPod's own features,
	// which is why the dispatch lives in the runtime crate.
	let result = commit_entry_validate::<Doc>(&buf[..header + 2]);

	if cfg!(feature = "compact-commit-full-validation") {
		assert_eq!(
			result,
			Err(pinapod::PinaPodError::InvalidUtf8),
			"the feature must widen the commit-entry check to the semantic walk"
		);
	} else {
		assert_eq!(
			result,
			Ok(()),
			"the default commit-entry check is the layout walk"
		);
	}
}
