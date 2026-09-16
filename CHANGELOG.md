# Changelog

All notable changes to this project will be documented in this file.

## [0.4.0](https://github.com/pina-rs/pinapod/releases/tag/pinapod/v0.4.0) (2026-09-16)

Grouped release for `pinapod-workspace`.

### Breaking Changes

#### support typed fixed arrays `[T; N]` in every schema position

_Packages:_ _pinapod_, _pinapod-derive_

`ZcElem`, `ZcValidate`, and `ZcField` are now implemented for arrays of any pod element, not only `[u8; N]`. A field declared `[u64; 4]` stores `[PodU64; 4]` little-endian with no length prefix, validation recurses per element, and the identity mapping for `[u8; N]` is preserved exactly. Nested arrays such as `[[u8; 4]; 2]`, pod-spelled arrays such as `[PodU64; 4]`, arrays of `PodBool`, and `Option<[u64; N]>` all resolve through the same composition rules.

The derive now maps array fields element-wise (`[u64; 4]` emits `[PodU64; 4]` storage), recurses capacity checks into array elements, and compact patch builders accept both native and pod spellings through a new hidden `IntoPodArray` conversion trait — `.weights([5, 6])` and `.weights([PodU64::from(5), PodU64::from(6)])` both compile. Fixed-schema accessors return the pod array by reference, matching the existing `Vec<T, N>` accessor shape.

For example, the field type now carries the element type instead of forcing raw bytes, with no change to the wire size:

```rust
// Before: only `[u8; N]` was supported, so a fixed u64 array was raw bytes.
#[derive(PinaPod)]
#[pinapod(compact)]
struct Table {
	weights: [u8; 16],
}
```

```rust
// After: the native element type is accepted throughout, storing `[PodU64; 2]`.
#[derive(PinaPod)]
#[pinapod(compact)]
struct Table {
	weights: [u64; 2],
}

fn round_trip() {
	let mut buffer = [0u8; Table::MAX_SIZE];
	let patch = TablePatch::new().weights([5_u64, 6]);
	let encoded_len = Table::initialize(&mut buffer, &patch).unwrap();
	let table = Table::read_prefix(&buffer[..encoded_len]).unwrap();
	assert_eq!(table.weights[0].get(), 5);
}
```

Soundness of the generalized `ZcElem` rests on the documented array layout rules: `[T; N]` inherits alignment 1 from `T`, its size is exactly `N * size_of::<T>()` so no padding can exist between elements, and per-element bit validity composes. The `[u8; N]`-specific impls were replaced by the generic ones, so downstream crates that hand-wrote `ZcField`/`ZcValidate` impls for their own array types will now conflict with the blanket impls — the closed-world safety guidance already rules that pattern out, but it is the one theoretically breaking edge and the reason both crates bump together.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #22](https://github.com/pina-rs/pinapod/pull/22)

### Documentation

#### document the public API and enforce `missing_docs`

_Packages:_ _pinapod_, _pinapod-derive_

Every publicly reachable item in `pinapod` now carries documentation: the crate root, both public modules, each pod type and container method, the error variants, and every trait, associated type, constant, and method in `traits.rs`. The derive crate documents its crate root, the `PinaPod` macro, and every struct and field option it accepts.

The workspace lint policy denies `missing_docs` alongside `unsafe_code`, so an undocumented public item in either library fails the build instead of shipping an undocumented entry in the wire-format contract. Test, benchmark, and fixture targets opt out with a reasoned `#[allow(missing_docs, reason = "...")]`, matching how `unsafe_code` is scoped.

Shared wording lives in one place. `mdt` providers in `api-docs.t.md` (API contracts expanded into rustdoc) and `templates/` (tables and contracts expanded into the README and the book) replace the text that was previously repeated across the pod containers, the error table, the float bit-pattern rules, and the prefix-width rules. `devenv shell docs:sync` rewrites the consumers, and `verify:docs` runs `mdt check` so a stale block fails CI. No API, wire format, or runtime behavior changes.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #23](https://github.com/pina-rs/pinapod/pull/23) · _Related issues:_ [#23](https://github.com/pina-rs/pinapod/issues/23)

### Notes

#### run Kani proofs against pinned Kani and modern solvers

_Packages:_ _pinapod_

The CI job installed z3 with `apt-get`, which on ubuntu-24.04 resolves to a 2021 release whose wide bitvector reasoning is far too slow. The `kani (u128)` and `kani (i128)` shards took 31.7 and 50.7 minutes and had begun timing out. Two changes fix that.

Each arithmetic harness is split so every expensive operation gets its own verification condition instead of accumulating one large formula, and the resulting harnesses request `cvc5` rather than `z3`. On the same formulas `z3` could not finish the signed 128-bit division proof within 25 minutes, while `cvc5` verifies it in 14 seconds; `z3` 5.1.0 did not help, so this is not a solver-version problem. The u128 shard now verifies 10 harnesses in about 30 seconds and the i128 shard 11 harnesses in about 22 seconds.

CI installs both solvers from their GitHub releases through a new `kani-solvers` action, pinned by SHA-256 per platform so a moved or compromised release asset cannot silently change what the proofs ran against. The Kani version is pinned alongside them. Proofs no longer run through the `devenv` environment, which saves roughly ten minutes per shard and keeps the devenv setup for local runs, where the same profile pins `cvc5` and `z3` for investigation.

Every assertion is preserved: each operation is still proven equal to its native counterpart, including signed overflow and division-by-zero cases. Nothing was weakened, bounded, or removed to make the proofs fit. No public API changes — the split harnesses live behind `#[cfg(kani)]` and the shard scripts make proofs runnable locally for the first time.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #26](https://github.com/pina-rs/pinapod/pull/26) · _Related issues:_ [#22](https://github.com/pina-rs/pinapod/issues/22), [#8083](https://github.com/pina-rs/pinapod/issues/8083)

#### finish the tabs-to-spaces switch and fix style fallout

_Packages:_ _pinapod_, _pinapod-derive_

The `hard_tabs = false` switch left most of the workspace formatted under the previous tab style, so `lint:format` (`dprint check`) failed across benches, tests, and both crates. `dprint fmt` completes the conversion, including the `tests/ui` fixtures, and `mdt check` stays green because the mdt `indent:"    "` directives now match the enforced style.

The compile-fail snapshots are re-blessed for the resulting span shifts and for the richer const-eval diagnostics of the pinned nightly, which also renders the containers' forced capacity assertions differently.

Two new tool lints needed reasoned workspace-policy entries. `rustdoc::invalid_markdown_table` rejects mdt's block close markers when they sit inline after the final row of a generated table, but the marker is an invisible synchronization delimiter, so the lint is always a false positive here. `clippy::used_underscore_items` misreads the containers' `let _ = Self::_CAP_CHECK` idiom, which references an underscore-prefixed const on purpose to force its compile-time assertion. No public API, wire format, or runtime behavior changes.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #28](https://github.com/pina-rs/pinapod/pull/28)

## [0.3.3](https://github.com/pina-rs/pinapod/releases/tag/pinapod/v0.3.3) (2026-09-14)

Grouped release for `pinapod-workspace`.

### Features

#### add IEEE-754 float pods behind the `floats` feature

_Packages:_ _pinapod_, _pinapod-derive_

The new `floats` feature adds `PodF32` and `PodF64`, alignment-one storage for IEEE-754 `f32` and `f64` values, and implements `ZcField` for the native primitives so a schema can declare `f32` or `f64` fields directly instead of spelling a pod type. Generated accessors decode back to the native float, so `field.get()` and the generated accessor both return `f32`/`f64` the same way the integer pods return `u16`/`u64`.

Storage is the complete bit pattern of the value, little-endian: four bytes for `f32` and eight for `f64`. `get`/`set` convert through the bit pattern while `to_bits`/`set_bits` expose it directly. Every bit pattern is a valid stored value, so `ZcValidate` accepts any bytes, validation cannot reject a NaN, infinity, or the sign of zero, and an all-zero field decodes as `+0.0`. Pod equality is bitwise rather than float-valued, which keeps `Eq` sound with NaN payloads and preserves the distinction between `+0.0` and `-0.0`; decode with `get` to compare with float semantics. The pods are byte containers and provide no arithmetic operators.

The pods deliberately implement no `PartialOrd` or `Ord`. Bitwise equality and float ordering cannot both hold, because an ordering that satisfied `PartialOrd`'s consistency contract would have to rank NaN payloads and separate `+0.0` from `-0.0`. Decode with `get` and compare the natives when an ordering is needed.

`PodF32` and `PodF64` compose with every container: they work as `PodOption` payloads, `PodVec` elements, compact inline header fields, and compact tails, and they implement canonical `wincode` serialization under that feature. The new mapping is additive — no existing wire format, validation order, or error variant changes, and `f32`/`f64` remain unmapped when the feature is disabled.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #20](https://github.com/pina-rs/pinapod/pull/20)

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
