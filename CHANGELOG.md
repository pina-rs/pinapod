# Changelog

All notable changes to this project will be documented in this file.

## [0.3.2](https://github.com/pina-rs/pinapod/releases/tag/pinapod/v0.3.2) (2026-09-12)

Grouped release for `pinapod-workspace`.

### Fixes

- _Packages:_ _pinapod_, _pinapod-derive_ **cut compact validation to bounded plain arithmetic.** On-chain compute-unit measurement against an SBF test programme showed the compact reader's per-element arithmetic costing instructions that `validate` had already proved unnecessary. Generated vector validation now strides the payload once through the audited `from_raw_parts` slice pattern instead of re-deriving every element offset, stores each tail end with plain adds for one- and two-byte prefixes only (a compile-time division-form assertion proves `max <= isize::MAX / size_of::<T>()` for those tails), and reads one- and two-byte optional tail prefixes with one bounds compare plus a constant-width decode. Four- and eight-byte prefixes keep the checked arithmetic so lengths that saturate or exceed `usize` on 32-bit targets can never wrap a plain sum before the bounds check. Fixed-layout `validate_exact` now takes a single length compare on the happy path, and the compact `validate` drops a header-length branch that `validate_storage_len` already subsumed. Measured on-chain: compact `validate` with dense non-trivial elements (`PodBool` vector plus optional tails) drops 5 compute units and every other measured instruction is unchanged, with wire format, validation order, error variants, and error precedence unchanged. Schemas declaring a one- or two-byte-prefix vector whose `max * size_of::<T>()` exceeds `isize::MAX` now fail to compile instead of always failing validation at runtime. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #17](https://github.com/pina-rs/pinapod/pull/17) · _Related issues:_ [#11](https://github.com/pina-rs/pinapod/issues/11), [#15](https://github.com/pina-rs/pinapod/issues/15)

## [0.3.1](https://github.com/pina-rs/pinapod/releases/tag/pinapod/v0.3.1) (2026-09-12)

Grouped release for `pinapod-workspace`.

### Fixes

- **pinapod-derive**: **restore 0.2 compact update costs.** On-chain measurement against Pina's example programs showed 0.3.0's update preflight paying for the reader's offset cache and for checked arithmetic that `validate` had already proved unnecessary. Generated `updated_len` now walks the current tails once with plain locals and arithmetic. Measured on-chain versus 0.2: compact `write` uses 24 fewer compute units, `resize` 96 fewer, and `rename` 8 fewer, with every other instruction unchanged. Wire format, validation order, error mapping, canonical zeroing, and every Kani proof are unchanged. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #15](https://github.com/pina-rs/pinapod/pull/15)

### Notes

- **pinapod**: **restore 0.2 compact update costs.** On-chain measurement against Pina's example programs showed 0.3.0's update preflight paying for the reader's offset cache and for checked arithmetic that `validate` had already proved unnecessary. Generated `updated_len` now walks the current tails once with plain locals and arithmetic. Measured on-chain versus 0.2: compact `write` uses 24 fewer compute units, `resize` 96 fewer, and `rename` 8 fewer, with every other instruction unchanged. Wire format, validation order, error mapping, canonical zeroing, and every Kani proof are unchanged. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #15](https://github.com/pina-rs/pinapod/pull/15)

## [0.3.0](https://github.com/pina-rs/pinapod/releases/tag/pinapod/v0.3.0) (2026-09-11)

Grouped release for `pinapod-workspace`.

### Breaking Changes

#### harden the error type and option tag surface

_Packages:_ _pinapod_

`PinaPodError` now implements `core::error::Error` and is `#[non_exhaustive]`, so downstream matches need a wildcard arm to receive new validation variants in later releases. `PodOption::raw_tag` returns `u64` instead of `u32` so eight-byte tags never truncate into a valid value, and `PodString::is_empty`/`PodVec::is_empty` now use the capacity-clamped length, which only changes behavior for zero-capacity containers. The `decode_len` sentinel and hidden generated-code types gained documented stability policies.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #11](https://github.com/pina-rs/pinapod/pull/11)

### Features

- _Packages:_ _pinapod_, _pinapod-derive_ **cache compact tail offsets in the reader.** Generated compact `Ref` views compute every tail offset once during construction instead of re-decoding all preceding length prefixes on each accessor call. Reading all fields of a schema with k tails drops from O(k²) prefix decodes to O(k), and each accessor is constant time. Compact vector validation also strides over the proven-bounded payload directly instead of re-checking multiplication per element. A six-tail benchmark group (`compact/many-tail-fields`) guards the scaling. Generated views are two words larger; wire bytes and validation semantics are unchanged. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #11](https://github.com/pina-rs/pinapod/pull/11)
- **pinapod**: **accept eight-byte option tags.** `PodOption<T, PFX>` now supports the same prefix widths as strings and vectors: 1, 2, 4, or 8 bytes. Tags decode through `u64` without truncation, and the Kani option proofs cover every width. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #11](https://github.com/pina-rs/pinapod/pull/11)

### Fixes

- **pinapod**: **pin the derive to its exact released version.** The runtime crate now requires `pinapod-derive = "=x.y.z"` instead of a caret range. Generated code expands against the runtime crate's private contracts, so a mixed pinapod/pinapod-derive pair could emit code the runtime does not match. Both crates continue to release together through MonoChange, which updates the pin. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #11](https://github.com/pina-rs/pinapod/pull/11)
- **pinapod-derive**: **report unrepresentable stored lengths as invalid length.** Generated compact decode helpers now return `InvalidLength`, matching the handwritten runtime, when a stored length prefix is wider than the target `usize`. On 32-bit targets a hostile eight-byte prefix previously surfaced as `Overflow` from generated readers while fixed readers reported `InvalidLength`; both now agree. Sixty-four-bit targets are unaffected. The expanded i686 regression job caught the disagreement. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #11](https://github.com/pina-rs/pinapod/pull/11)

### Documentation

- _Packages:_ _pinapod_, _pinapod-derive_ **add fuzzing and broader toolchain verification.** A cargo-fuzz suite now covers every untrusted-input reader: fixed validation, compact validation, compact patch commits, compact enums, wincode reads, and container mutators, with smoke runs on relevant pull requests and a weekly scheduled run. Kani proofs cover a derive-generated compact schema (validated accessor bounds, initialize and update round-trips) in a new CI shard. The full test suite runs on current stable and on the MSRV, the 32-bit job runs the fixed and compact regression suites, and `cargo-deny` rejects yanked crates. New book pages document the MSRV/Agave support policy, derive type resolution, and the deliberate zero-fill costs. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #11](https://github.com/pina-rs/pinapod/pull/11)

### Notes

- _Packages:_ _pinapod_, _pinapod-derive_ **refresh the renamed-dependency fixture lock during releases.** The compile fixture at `pinapod/tests/renamed_dependency` carries its own `Cargo.lock` that pins the path dependency's version. The release flow now refreshes it alongside the workspace lock so the fixture's `--locked` compile check stays valid across version bumps. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #14](https://github.com/pina-rs/pinapod/pull/14) · _Related issues:_ [#13](https://github.com/pina-rs/pinapod/issues/13)
- **pinapod-derive**: **pin the derive to its exact released version.** The runtime crate now requires `pinapod-derive = "=x.y.z"` instead of a caret range. Generated code expands against the runtime crate's private contracts, so a mixed pinapod/pinapod-derive pair could emit code the runtime does not match. Both crates continue to release together through MonoChange, which updates the pin. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #11](https://github.com/pina-rs/pinapod/pull/11)
- **pinapod-derive**: **accept eight-byte option tags.** `PodOption<T, PFX>` now supports the same prefix widths as strings and vectors: 1, 2, 4, or 8 bytes. Tags decode through `u64` without truncation, and the Kani option proofs cover every width. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #11](https://github.com/pina-rs/pinapod/pull/11)
- **pinapod-derive**: **harden the error type and option tag surface.** `PinaPodError` now implements `core::error::Error` and is `#[non_exhaustive]`, so downstream matches need a wildcard arm to receive new validation variants in later releases. `PodOption::raw_tag` returns `u64` instead of `u32` so eight-byte tags never truncate into a valid value, and `PodString::is_empty`/`PodVec::is_empty` now use the capacity-clamped length, which only changes behavior for zero-capacity containers. The `decode_len` sentinel and hidden generated-code types gained documented stability policies. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #11](https://github.com/pina-rs/pinapod/pull/11)

## [0.2.0](https://github.com/pina-rs/pinapod/releases/tag/pinapod/v0.2.0) (2026-09-07)

Grouped release for `pinapod-workspace`.

### Breaking Changes

#### redesign the safe PinaPod account API

_Packages:_ _pinapod_, _pinapod-derive_

Rename the public API to PinaPod, add safe fixed container initialization and compact Ref/Patch updates, preserve wire compatibility, and close the documented soundness gaps.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #8](https://github.com/pina-rs/pinapod/pull/8)

## pina [0.1.0](https://github.com/pina-rs/pinapod/releases/tag/pina/v0.1.0) (2026-09-06)

Grouped release for `pina`.

### Features

#### Establish the PinaPod compatibility fork

_Packages:_ _pina_

Fork ZeroPod 0.3.5 into the independently versioned `pinapod` and `pinapod-derive` crates while retaining its Apache-2.0 history and on-chain byte representation.

This first release fixes compact accessors for multiple unequal vector tails, tightens the unsafe POD trait contracts, and replaces unsound wincode direct-borrow serialization with validated canonical encoding that does not read or disclose inactive capacity.

Generated compact views preserve generic type parameters without changing their wire layout, scalar-only compact enums compile cleanly under `deny(warnings)`, and unsupported length-prefix widths or generic compact enums now produce focused macro diagnostics.

It also adds optional `fixed` 1.30.0 integration for all signed and unsigned fixed-point widths. Fixed-point fields map to their alignment-one little-endian integer pods and work in fixed schemas, dynamic compact vectors, options, and schemas with multiple independently sized tails.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #1](https://github.com/pina-rs/pinapod/pull/1)
