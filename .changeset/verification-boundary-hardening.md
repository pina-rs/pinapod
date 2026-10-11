---
pinapod: fix
pinapod-derive: fix
---

# harden the verification boundary: fuzz lock, CI checks

The security-audit follow-ups that live around the crates rather than inside them, applied together because they close one boundary:

- `fuzz/Cargo.lock` still recorded pinapod 0.4.1 while the workspace was at 0.4.6, and the fuzz workflow ran `cargo fuzz` without a lock check, so the fuzz suite — part of the declared security boundary — silently rewrote its registry dependencies on every CI run. The lock is refreshed, the workflow verifies it up front with `cargo metadata --locked` and fails closed, the release flow refreshes it through monochange's `lockfile_commands`, dependabot tracks `/fuzz`, and `security:audit` now audits the fuzz lock alongside the root lock.
- `cargo-deny`'s `[advisories]` section was configured but never checked; `security:deny` now runs `cargo-deny check advisories bans licenses sources`.
- `gitleaks` shipped in devenv but nothing ran it. A `security:secrets` script now scans the commit history, and the CI security job runs it with a full-depth checkout.
- The fuzz workflow now also triggers on merges to main with the smoke budget, so direct merges are fuzzed before the weekly run, and crash artifacts are uploaded as workflow artifacts in addition to the hexdump.
- The coverage workflow's `id-token: write` moved to a dedicated upload job that runs no repository code — no checkout and no repo-local composite action — so the OIDC-holding job executes only the pinned download and codecov actions over the artifact the build job produced.

The fuzz targets also gained the coverage the audit mapped as missing: a mutable-read pass over attacker bytes in `fixed_validate`, post-condition assertions and four- and eight-byte prefixes through the wincode reader in `wincode_read`, and a third compact schema in `compact_validate` covering the nested tail grammar (`Option<Vec<u64, 4>>` and `Vec<String<4>, 2>`, whose per-element walk owns the nested UTF-8 checks). `compact_backend` additionally pins present-but-empty optional payloads (`Some("")`, `Some(&[])`) through initialize and update: their staged pointers are non-null with zero length, which is the discriminating case for the null-means-clear sentinel, and were previously untested.
