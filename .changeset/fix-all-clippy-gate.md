---
pinapod: none
pinapod-derive: none
---

# fail on every Clippy warning and add `fix:all`

Clippy was run without `--all-targets`, so lint findings in tests, benchmarks, and compile fixtures never reached either `lint:clippy` or CI's `lint` job; only the library and binary targets were covered. `lint:clippy` now lints every target with `-D warnings`, so the local gate and CI agree and neither can hide a warning in a test file. Every finding the wider sweep surfaced is fixed rather than suppressed — one targeted `#[allow(clippy::clone_on_copy)]` is the only new suppression, on a test whose purpose is to exercise `clone` on a `Copy` type. The fixes are mechanical: deprecated `criterion::black_box` imports become `std::hint::black_box`, boolean `assert!` comparisons become `assert_eq!` / `assert_ne!`, and raw-pointer `as` casts become `.cast()` or `ptr::from_ref`. The manifest lints are resolved by inheriting `keywords` and dropping the `homepage` key that Cargo reports as redundant with `repository`, rather than deleting the metadata.

One of those findings sits in the derive: an `impl Schema` block had been left after the `#[cfg(test)] mod tests` block, which trips `clippy::items_after_test_module` now that the lint reaches lib targets. The impl moved above the tests; it is a pure reordering with no behavior change.

A new `fix:all` devenv task applies the whole fix suite in one command: `fix:clippy` across every target, then `docs:sync`, then `fix:format`. The explicit `docs:sync` is what makes the ordering correct rather than incidental — `fix:format` runs dprint before its own `docs:sync`, so an mdt rewrite performed inside it would land after formatting and never be formatted itself. Syncing ahead of the formatting pass means the final `fix:format` formats the blocks mdt produces, and the tree is left clean for `lint:*`. Running it twice produces no further changes.

`CARGO_BUILD_JOBS` is capped at 4 in the shell environment. `clippy --fix` re-runs the compiler to a fixpoint and the docs and test tasks each rebuild the workspace, so letting every invocation size its job pool to all 12 cores made a local `fix:all` saturate the machine. The cap is deliberately not expressed through `RUSTFLAGS`, which would invalidate the whole build cache and force a full rebuild.

The version bumps that introduced this work had also silently dropped the `zeropod` git pin: the workspace table declared a crates.io version with no `git`/`rev`, so Cargo ignored the member's pin (with an "unused manifest key" warning) and the benchmark comparison baseline had drifted from the pinned revision to crates.io 0.3.6. The git source now lives in the workspace table, which is the only place it can live for a member that consumes it via `workspace = true`. The 14 trybuild UI snapshots are re-blessed for column-number shifts caused by the `syn` 2 to 3 bump; the diffs are location lines only, with no change to any diagnostic message.
