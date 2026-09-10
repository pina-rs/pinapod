# Supported toolchains and Solana versions

PinaPod exists for Solana account data, so its supported-compiler window follows the Solana ecosystem rather than the newest stable Rust.

## MSRV policy

- The minimum supported Rust version (MSRV) is the oldest Rust pinned by an Agave line that Pina targets. The repository pins the exact MSRV in `rust-version` and runs the full test suite against it in CI.
- PinaPod does not chase every Agave toolchain bump. The MSRV moves only when the supported Solana window moves, and never sooner than needed.
- An MSRV bump is announced as a breaking change with a migration note in the changelog. The [Agave table](#agave-rust-versions) below records the current alignment.
- Development happens on the pinned nightly in `rust-toolchain.toml` (for Miri), but nightly is never _required_: CI runs the full suite on that nightly, on current stable, and on the MSRV. Compile-fail UI snapshots are recorded on the pinned nightly; other toolchain jobs skip them with `PINAPOD_UI=skip`.

## Agave Rust versions

Each row gives the Rust channel pinned by `rust-toolchain.toml` at that Agave release tag. PinaPod's MSRV of **1.89** covers every Agave line from the late v3.x era onward; older lines predate the supported window.

| Agave line | Pinned Rust | PinaPod MSRV 1.89 covers |
| ---------- | ----------- | ------------------------ |
| v2.1       | 1.81.0      | no (EOL)                 |
| v2.2       | 1.84.1      | no (EOL)                 |
| v2.3–v3.1  | 1.86.0      | no (EOL)                 |
| v3.2 era   | 1.89.0      | yes (floor)              |
| v3.3 era   | 1.90.0      | yes                      |
| v4.0       | 1.93.1      | yes                      |
| v4.1       | 1.95.0      | yes                      |
| v4.2       | 1.96.1      | yes                      |
| v4.3       | 1.97.1      | yes                      |

When Agave retires the last line that needs a given Rust version, the next PinaPod breaking release may raise the MSRV to the new floor. The table is refreshed from `anza-xyz/agave` history at that time.

## Solana dependency ranges

The optional `solana-address` and `solana-program-error` integrations accept `solana-address >= 2.2, < 2.7` and `solana-program-error >= 3.0,
< 4.0`. These ranges describe the SDK **type-compatibility window** for the optional features, not the supported toolchain window. Building those features still requires a compiler that satisfies the PinaPod MSRV. The cap is deliberate: major and minor Agave SDK lines are only widened after their representatives are tested, and the range is revisited together with the table above.

## Versioning

Versions and changelogs are managed by [MonoChange](https://github.com/MonoChange/monochange) through `.changeset/*.md` intent files. MonoChange also provides the project's semantic-version check in place of `cargo-semver-checks`: every pull request runs the `changeset-policy` workflow, which enforces changeset coverage and posts a semantic change classification with the proposed bump per package, and the release preview derives compatibility evidence from the semantic diff. Never edit `CHANGELOG.md` by hand; it is rendered from changesets during the release flow.
