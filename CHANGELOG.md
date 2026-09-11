# Changelog

All notable changes to this project will be documented in this file.

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
