# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

### Breaking changes

- Renamed the derive and public schema contracts to `PinaPod`, `PinaPodFixed`, `PinaPodCompact`, and `PinaPodError`.
- Split fixed reads into exact and prefix contracts.
- Removed integer pod arithmetic, remainder, bitwise, shift, assignment, negation, and native-left comparison operators. Explicit checked, wrapping, and saturating methods remain.
- Replaced direct compact-header mutation and staged commits with generated, atomic patch types.
- Moved explicit string and vector prefix widths into const generic arguments, such as `PodVec<u64, 1024, 2>`.
- Made manual fixed and compact schema implementations unsafe. Removed the fixed trait size constant; derived schemas expose an inherent size calculated from the mapped representation.

### Added

- Added bounded strings, vectors, options, and recursively bounded containers to fixed schemas.
- Added the first compact nesting set: optional dynamic tails and fixed-footprint `Vec<String<M>, N>` elements.
- Added fixed and compact initialization APIs that validate after configuration.
- Added compact schema bounds and allocation-granularity validation to `PinaPodCompact`.
- Added native, Miri, compile-fail, and Kani regressions for the unsoundness paths found during the v0.2 review.
- Added a three-way Criterion comparison against PinaPod v0.1 and the pinned upstream ZeroPod baseline.
- Added an mdBook with GitHub Pages publication and warning-denying Rust API docs.

### Safety

- Compact readers and updates use checked length and offset arithmetic.
- Generated compact internals no longer expose mutable length metadata.
- Container defaults initialize inactive capacity, and shortening operations zero removed bytes.
- `PodOption` no longer exposes an inactive payload through a safe unchecked accessor.
- Wincode serialization recursively writes canonical initialized bytes.

The wire format remains compatible with PinaPod v0.1. See the [v0.2 migration guide](docs/src/migration-v0.2.md) for source changes and the coordinated Pina release order.

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

## [0.2.0](https://github.com/pina-rs/pinapod/releases/tag/pinapod/v0.2.0) (2026-09-07)

Grouped release for `pinapod-workspace`.

### Breaking Changes

#### redesign the safe PinaPod account API

_Packages:_ _pinapod_, _pinapod-derive_

Rename the public API to PinaPod, add safe fixed container initialization and compact Ref/Patch updates, preserve wire compatibility, and close the documented soundness gaps.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #8](https://github.com/pina-rs/pinapod/pull/8)
