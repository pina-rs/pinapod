//! Verifies a consumer reaches `fixed` types through `PinaPod`'s re-export.
//!
//! The fixture crate at `tests/fixed_reexport` depends on `pinapod` alone: it
//! has no `fixed` requirement of its own, so every `fixed` name it uses must
//! resolve through the re-export behind `PinaPod`'s `fixed` feature. Building
//! it is the assertion — a re-export that went missing, or that resolved to a
//! release other than the one `PinaPod` implements `ZcField` for, fails here.

#![cfg(not(miri))]

use std::path::PathBuf;
use std::process::Command;

#[test]
fn consumer_uses_fixed_through_the_pinapod_reexport() {
	let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	let fixture = manifest_dir.join("tests/fixed_reexport/Cargo.toml");
	let target = manifest_dir.join("../target/fixed-reexport");
	let output = Command::new(env!("CARGO"))
		.arg("run")
		.arg("--quiet")
		.arg("--manifest-path")
		.arg(fixture)
		.arg("--locked")
		.env("CARGO_TARGET_DIR", target)
		.output()
		.unwrap_or_else(|error| panic!("fixed re-export compile fixture must run: {error}"));

	if output.status.success() {
		return;
	}

	panic!(
		"fixed re-export fixture failed\nstdout:\n{}\nstderr:\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr),
	);
}
