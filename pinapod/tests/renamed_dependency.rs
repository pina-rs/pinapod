//! Verifies derive expansion when Cargo exposes PinaPod under another name.

#![cfg(not(miri))]

use std::{path::PathBuf, process::Command};

#[test]
fn renamed_direct_dependency_compiles() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = manifest_dir.join("tests/renamed_dependency/Cargo.toml");
    let target = manifest_dir.join("../target/renamed-dependency");
    let output = Command::new(env!("CARGO"))
        .arg("check")
        .arg("--manifest-path")
        .arg(fixture)
        .arg("--locked")
        .env("CARGO_TARGET_DIR", target)
        .output()
        .unwrap_or_else(|error| panic!("renamed-dependency compile fixture must run: {error}"));

    if output.status.success() {
        return;
    }

    panic!(
        "renamed-dependency fixture failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
