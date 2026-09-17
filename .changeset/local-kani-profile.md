---
pinapod: none
---

# make the local Kani profile actually run proofs

`devenv --profile kani shell` could not run a single harness on macOS. Two separate faults were stacked behind each other.

The profile linked Kani against `nightly-2025-11-21`, but the packaged Kani 0.68.0 is built against `nightly-2026-08-21`. `kani-compiler` aborted on startup with `dyld: Library not loaded: @rpath/librustc_driver-83914c2c3aa68f2a.dylib`, so no harness ever reached verification. The toolchain pin now matches the nightly named by Kani's own `rust-toolchain-version`, and a comment records how to read that value when the nixpkgs pin is next bumped.

With the toolchain corrected, CBMC's `goto-cc` failed next with `execvp gcc failed: No such file or directory`. It preprocesses each harness by invoking a native C compiler under the literal name `gcc`, which a Darwin machine does not have. The profile now ships a `gcc` shim that execs clang; `goto-cc` parses the preprocessed C itself and never links the result, so the substitution is safe. `--native-compiler clang` cannot be threaded through `cargo-kani`, which is why the shim supplies the name instead.

`cargo-kani` now verifies `bool::kani_proofs` (3 harnesses) and `u32_proofs` (10 harnesses) locally, so a red CI shard can be reproduced on a laptop. The shard scripts also reject a missing harness argument rather than silently running the whole proof suite, and the timing script no longer runs a stray `cargo metadata` whose output it discarded.
