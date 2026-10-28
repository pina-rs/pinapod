# Changelog

All notable changes to this project will be documented in this file.

## [0.4.8](https://github.com/pina-rs/pinapod/releases/tag/pinapod/v0.4.8) (2026-10-28)

Grouped release for `pinapod-workspace`.

### Features

#### value traits and owned materialization

_Packages:_ 🟠 _pinapod_, 🟠 _pinapod-derive_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #65](https://github.com/pina-rs/pinapod/pull/65)

Generated representations were compare-blind: a fixed schema's `Zc` companion had `Copy` and `Clone` but no `PartialEq`, `Hash`, or `Debug`, so `assert_eq!` between two snapshots of the same account did not compile, a snapshot could not key a map, and a test failure printed nothing. Compact views had none of them either.

Every fixed companion and compact view now implements `PartialEq`, `Eq`, `Hash`, and `Debug` over **active** content: two encodings of the same value are equal regardless of inactive capacity, hashing agrees with that equality, and `Debug` renders field names without disclosing vacated bytes. The impls are conditional on the mapped pods carrying the corresponding trait, so a custom `ZcField` mapping without `Hash` keeps compiling and simply has no `Hash` companion impl. The fixed-enum companions and `PodOption` gained the `Eq`/`Hash` pieces they were missing so enum-bearing and optional fields compose into all four.

The second half is materialization: a new `FromZc` trait is the return path for `ZcField`'s outbound mapping, and every fixed companion and compact view exposes `to_native()`, which builds the fully owned declared struct — snapshots for logging and tests, off-chain decoding, cross-thread transfer; the shapes a borrowed view cannot serve. Materialization is fallible for exactly one reason: a stored enum discriminant naming no variant, which cannot occur for a value a validating reader accepted. Compact materialization copies active tail content into the declared containers, so the result never borrows the account bytes.

One contract follows for custom representations: because the generated impls bound on the mapped pods, a schema field using a hand-written `ZcField` mapping now needs that mapping to implement the value vocabulary — `PartialEq`, `Eq`, `Hash`, and `Debug` for the companion traits, `FromZc` for materialization. Built-in and derived-schema mappings already do; `compact_ref_auto_traits.rs` extends its deliberate `!Sync`/`!Send` fixtures with the same impls, which leave their thread-safety semantics untouched.

#### volatile access and checked accessors

_Packages:_ 🟠 _pinapod_, ⚪ _pinapod-derive_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #64](https://github.com/pina-rs/pinapod/pull/64)

Two safe ways out of the two remaining sharp edges in the pod surface.

First, every integer pod gains `get_volatile` and `set_volatile`. A CPI callee can write the account a program has already formed its view over, and a plain `get` lets the compiler reuse a load from before the call, because nothing in the caller's source proves the bytes changed — the classic stale-read-after-CPI hazard. A volatile read forces the bytes to be re-read from memory, so a balance or counter updated by a callee is observed after the call returns; a volatile write is emitted as one store that cannot be sunk past a later call. These are the account-data analogue of a `Relaxed` atomic load and store on the single-threaded BPF target, and the docs say exactly that: they prevent compiler caching and provide no synchronization.

Second, `PodString::try_as_str`, `PodVec::try_as_slice`, and `PodOption::try_get_ref` validate before borrowing. `as_str` trusts that a reader validated the bytes before the reference was formed — sound, but it left a consumer holding an unchecked reference (cast from raw account bytes) with no safe exit: calling `as_str` on forged non-UTF-8 bytes is undefined behavior, and `get_ref` silently reports a corrupt option tag as absent. The checked counterparts reject exactly what the reader walk rejects — an over-capacity prefix, invalid UTF-8, an out-of-domain element, a corrupt tag, an invalid present payload — so casting raw bytes and calling them is always sound.

Both are pinned by `pinapod/tests/safe_accessors.rs`: volatile round trips across all eight integer widths, a volatile read observing bytes written through a raw pointer, and the checked accessors' accept and reject paths over forged length prefixes, tags, and payloads.

#### diagnose: attribute rejections to fields

_Packages:_ 🟠 _pinapod_, 🟠 _pinapod-derive_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #66](https://github.com/pina-rs/pinapod/pull/66)

A flat `PinaPodError` answers whether a buffer holds a valid value; it cannot answer which field rejected it. Debugging a corrupt account — in an explorer, a simulator, a test failure — was guesswork over the byte layout.

Every derived schema now exposes `Type::diagnose(data)`, which re-runs the validation walk with attribution and returns a `Diagnostic` carrying the rejecting field's name beside the underlying `PinaPodError`. Fixed schemas attribute each field's validation; compact schemas attribute the allocation checks to the schema's own name (no field owns them), the header's inline fields to their names, and every tail rejection — tag, capacity, bounds, UTF-8, element domain — to the tail's field. A nested schema field is attributed at its own name, and the nested `diagnose` can be called on the field's bytes for one level deeper.

The acceptance contract is the readers': `diagnose` accepts and rejects exactly what the corresponding validation accepts and rejects, with the same error variant. That contract is not a claim but a test — `validation_diagnostics.rs` pins it across 2,000 pseudo-randomly corrupted buffers in addition to every attribution site — and the compact walk uses checked arithmetic throughout, because unlike the reader accessors it deliberately runs on bytes that may be invalid.

`Diagnostic` implements `Display` (`field \`label\`: invalid UTF-8 in string field`),`core::error::Error`with the underlying error as its`source`, and`From<Diagnostic> for PinaPodError`, so it drops into existing error handling unchanged.

### Fixes

#### instruction-count performance gates on every pull request

_Packages:_ 🟢 _pinapod_, ⚪ _pinapod-derive_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #63](https://github.com/pina-rs/pinapod/pull/63)

Every pull request now runs an instruction-count gate over PinaPod's hot paths, in addition to the wall-clock API comparison. The new `performance` workflow measures the `iai_hot_paths` bench through Valgrind's Callgrind and compares the retired-instruction counts against a committed baseline with a +3% tolerance. Instruction counts are a property of the binary rather than the machine, so the comparison is exact on any Linux runner and no timing noise can hide a regression; that determinism is why the gate can run on every PR instead of only on performance-motivated ones.

The gate covers fixed parse, validate, read, write, and initialize; compact small and maximum parse, validate, access, and update; the six-tail accessors; and, new in this change, large-tail parse, validate, access, and grow-to-full update at a 64 KiB tail. The large-tail fixtures exist because accounts may allocate up to 10 MiB and the previous 207-byte fixture could not observe scaling at real account sizes. The fixtures are shared with the Criterion harness through `pinapod/benches/common`, so both harnesses measure the same workloads over the same bytes, and the Criterion harness also gained a `bytemuck-unchecked-cast` floor for the fixed parse, which prices the same read with validation removed.

The baseline lives in `pinapod/benches/iai-baseline.json` and changes only through `devenv shell bench:iai:update`, whose diff must be justified in the pull request. A tolerance-exceeding regression without a baseline update fails the check by design. The book documents the gate, the update policy, and the new workloads in `docs/benchmarks.md`, and a new cookbook page covers discriminators and schema versioning built on `read_prefix`.

#### widen the dependency bounds

_Packages:_ 🟢 _pinapod_, ⚪ _pinapod-derive_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #71](https://github.com/pina-rs/pinapod/pull/71)

Two bounds stopped a Pinapod consumer from moving to the current Agave and Surfpool trains, even though the code compiles against both.

`solana-address` was capped at `<2.7`. Agave 4.3, which Surfpool 1.7's embedded runtime ships, resolves `solana-address` 2.7 or newer, and Cargo cannot hold two `2.x` versions at once, so the workspace could not resolve. The bound is now `>=2.2, <3.0`.

`fixed` was pinned to `=1.30.0`. `surfpool-core` 1.7 pulls `phoenix-rise-math`, which pins `fixed =1.29.0`, so the two exact pins conflict in a workspace that carries both Pinapod and Surfpool. The requirement is now `^1.29.0`, which admits the 1.29 line the current Agave/Surfpool SDK resolves, the newer 1.30 and 1.31 releases, and future 1.x patches.

The workspace lockfile pins `fixed` 1.30.0 and `solana-address` 2.9.0, so the suite exercises the newest releases inside both ranges that still build on Pinapod's Rust 1.89 minimum; `fixed` 1.31.0 requires Rust 1.93 and stays available to consumers on newer toolchains. The lowest accepted `fixed` (1.29.0) is exercised by a downstream workspace that carries Pinapod and Surfpool 1.7 together.

- **Decode Wincode containers through their nested codecs.** `PodVec` and `PodOption` now require a matching fixed-size `SchemaRead` implementation for their elements and decode active values through it, preserving custom wire transformations and codec errors. All container readers reject nonzero inactive storage, including nested padding, so accepted bytes already match the canonical representation emitted by their writers. _Packages:_ 🟢 _pinapod_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #68](https://github.com/pina-rs/pinapod/pull/68)
- **Preserve custom PodOption representation mappings.** Compact derives now resolve types named `PodOption` through their `ZcField` contract instead of substituting PinaPod's built-in option storage. Custom qualified types retain their declared validation and layout, including vector elements in compact enums. Genuine PinaPod options retain their existing payload and tag widths through their identity mapping. _Packages:_ 🟢 _pinapod-derive_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #69](https://github.com/pina-rs/pinapod/pull/69)
- **Preserve thread-safety bounds on compact views.** Generated compact `Ref` types now implement `Send` and `Sync` only when their header and vector element pods allow shared access across threads. This prevents views from bypassing a custom pod's `!Sync` restriction while preserving thread-safe representation mappings. _Packages:_ 🟢 _pinapod-derive_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #61](https://github.com/pina-rs/pinapod/pull/61)

## [0.4.7](https://github.com/pina-rs/pinapod/releases/tag/pinapod/v0.4.7) (2026-10-11)

Grouped release for `pinapod-workspace`.

### Fixes

#### harden the verification boundary: fuzz lock, CI checks

_Packages:_ 🟢 _pinapod_, 🟢 _pinapod-derive_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #51](https://github.com/pina-rs/pinapod/pull/51)

The security-audit follow-ups that live around the crates rather than inside them, applied together because they close one boundary:

- `fuzz/Cargo.lock` still recorded pinapod 0.4.1 while the workspace was at 0.4.6, and the fuzz workflow ran `cargo fuzz` without a lock check, so the fuzz suite — part of the declared security boundary — silently rewrote its registry dependencies on every CI run. The lock is refreshed, the workflow verifies it up front with `cargo metadata --locked` and fails closed, the release flow refreshes it through monochange's `lockfile_commands`, dependabot tracks `/fuzz`, and `security:audit` now audits the fuzz lock alongside the root lock.
- `cargo-deny`'s `[advisories]` section was configured but never checked; `security:deny` now runs `cargo-deny check advisories bans licenses sources`.
- `gitleaks` shipped in devenv but nothing ran it. A `security:secrets` script now scans the commit history, and the CI security job runs it with a full-depth checkout.
- The fuzz workflow now also triggers on merges to main with the smoke budget, so direct merges are fuzzed before the weekly run, and crash artifacts are uploaded as workflow artifacts in addition to the hexdump.
- The coverage workflow's `id-token: write` moved to a dedicated upload job that runs no repository code — no checkout and no repo-local composite action — so the OIDC-holding job executes only the pinned download and codecov actions over the artifact the build job produced.

The fuzz targets also gained the coverage the audit mapped as missing: a mutable-read pass over attacker bytes in `fixed_validate`, post-condition assertions and four- and eight-byte prefixes through the wincode reader in `wincode_read`, and a third compact schema in `compact_validate` covering the nested tail grammar (`Option<Vec<u64, 4>>` and `Vec<String<4>, 2>`, whose per-element walk owns the nested UTF-8 checks). `compact_backend` additionally pins present-but-empty optional payloads (`Some("")`, `Some(&[])`) through initialize and update: their staged pointers are non-null with zero length, which is the discriminating case for the null-means-clear sentinel, and were previously untested.

#### wincode writers validate the content their readers judge

_Packages:_ 🟢 _pinapod_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #51](https://github.com/pina-rs/pinapod/pull/51)

The wincode writers rejected a corrupt length prefix and a corrupt option tag, but emitted corrupt _content_ verbatim: a `PodString` whose prefix was in range over non-UTF-8 bytes, a `PodVec<PodBool>` with an out-of-domain element, and a `PodOption` with a valid tag over an invalid payload all serialized successfully into bytes the crate's own reader refuses. The canonical-output contract therefore held only for already-valid values, which only an unchecked reference could violate — but that is exactly the tier the prefix and tag checks were added for, so the boundary was inconsistent.

Every writer now runs the `ZcValidate` walk its reader runs before the first byte is written: `PodBool`, `PodString`, `PodVecRepr`, and `PodOption` each reject invalid stored content with a focused message (`PinaPod string content failed validation`, and the matching variants), and the option writer checks its tag _before_ writing the prefix, so all three rejection orderings match and a rejected serialization leaves the destination untouched. Valid values serialize identically; the added walk is proportional to what the writer already emits and is a no-op for trivially valid elements through the existing `ZcValidate::validate_slice` fast path.

The regression is pinned from the forging side in `wincode_serialize.rs`: five tests hand-cast values whose metadata is in range and whose content is invalid, and assert serialization fails without touching the destination.

`PodString`'s documentation now also states the precondition its accessors trust: `as_str` forms its reference through `from_utf8_unchecked`, so the active bytes are UTF-8 because a safe constructor wrote them or a validating reader accepted them — not because the length clamp checks them. `as_bytes`, `chars`, `bytes`, and `truncate`'s boundary walk carry the same note, and a `validation.rs` regression pins the boundary by proving every validating reader rejects a forged non-UTF-8 string before a reference exists.

#### generated compact modules bind the prelude names they spell

_Packages:_ 🟢 _pinapod-derive_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #51](https://github.com/pina-rs/pinapod/pull/51)

The compact generators wrap their output in a child module that opens with `use super::*`, and a glob import outranks the prelude. A schema module that defines (or glob-imports) an item named `Ok` with a matching signature therefore rebound every validation return the generated code spelled: the generated commit path compiled the shadow in and called it at runtime. An `Ok`-shaped function that panics turned every generated read, update, and initialize into a panic; `Some`/`None`-shaped functions and a `usize`-named type instead broke compilation with type errors pointing inside the generated module. The fixed generator was already immune through fully qualified `::core` spellings, so only the compact generators were exposed.

The generated compact struct and enum modules now bind those names to `core` explicitly (`use ::core::option::Option::{self, None, Some}` and companions), which outranks the schema module's glob for every bare `Result`, `Option`, `Some`, `None`, `Ok`, `Err`, and primitive the generated body spells, without rewriting the hundreds of emission sites. Schema modules can shadow prelude names again without reaching generated code.

The regression is pinned from three directions: `compact_prelude_shadowing.rs` defines a panicking `Ok` shadow plus type-identical `Result`/`Option` aliases in the schema module and runs full initialize/update/read round trips for both a compact struct and a compact enum — before the fix the first generated `Ok(())` panicked — and the `compact_prelude_patterns` fixture covers the shapes that were compile errors before the fix (`Some`/`None` functions and a `usize`-named type) behind the compile-pass gate.

Zero-variant enums are also now a focused derive error rather than an opaque one: both layout paths previously emitted a degenerate representation for `enum Never {}` and let rustc surface `E0084`/`E0004` from the declaration and the generated module. The derive rejects the schema at the declaration site for both layouts, pinned by `compact_empty_enum` and `fixed_empty_enum` compile-fail fixtures.

## [0.4.6](https://github.com/pina-rs/pinapod/releases/tag/pinapod/v0.4.6) (2026-10-05)

Grouped release for `pinapod-workspace`.

### Fixes

- **Monostyle style pass.** Breathing room around control flow and returns, group splits, and collapsed blank runs, applied by `monostyle fix` and kept where rustfmt puts them. No behavior change. _Packages:_ 🟢 _pinapod_, 🟢 _pinapod-derive_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #44](https://github.com/pina-rs/pinapod/pull/44)

## [0.4.5](https://github.com/pina-rs/pinapod/releases/tag/pinapod/v0.4.5) (2026-09-29)

Grouped release for `pinapod-workspace`.

### Fixes

#### reject array-nested prefixes instead of rewriting them

_Packages:_ 🟢 _pinapod_, 🟢 _pinapod-derive_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #45](https://github.com/pina-rs/pinapod/pull/45)

A compact field declared as `[PodString<8, 3>; 4]` previously skipped the prefix-width check entirely: validation descended through generic arguments but never through array elements, so the element-wise mapping silently re-encoded the field as `PodString<8, 1>` and the account compiled with a different wire layout than the schema declared. The runtime still validated whatever bytes the rewritten type produced, so this was never memory-unsound, but a client generated from the declared schema would disagree with the on-chain bytes — exactly the divergence the crate's every-unsupported-declaration-is-a-compile-error contract exists to prevent.

The declaration is now a compile error with a focused diagnostic. The descent also covers `Option<[PodVec<u8, 8, 0>; 2]>` and parenthesized spellings, and a valid explicit width inside an array (such as `[PodString<300, 2>; 4]`) is still accepted. The regression is pinned from three directions: a derive unit test that fails on the pre-fix parser, two `ui/fail` fixtures with checked `.stderr` snapshots that compile successfully on the pre-fix parser, and a new adversarial suite (`tests/array_nested_prefixes.rs`) attacking the runtime side of the same surface — forged inner length prefixes, non-UTF-8 payloads, forged option tags over inactive array payloads, and short allocations, each rejected by the walk that owns it, with the layout-versus-semantics split asserted explicitly.

#### trim compact-enum checks and document the generated surface

_Packages:_ 🟢 _pinapod_, 🟢 _pinapod-derive_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #46](https://github.com/pina-rs/pinapod/pull/46) · _Related issues:_ [#45](https://github.com/pina-rs/pinapod/issues/45)

The compact-enum commit-entry check no longer interprets fixed payloads. `validate_layout` for a variant with a `Fixed` payload ran the nested `PinaPodFixed::validate_prefix` — a full semantic walk — although relocation consumes only the payload's compile-time range. The bounds half is now the pure range check, and the semantic half stays in `validate` and every value-exposing boundary, emitted from the same per-variant fragment so the two walks cannot drift. A derive contract test asserts the split, and the dead tag-size guard after `validate_storage_len` is gone with it: `MIN_SIZE` is always at least the tag size, so the storage check already rejected every short buffer and the branch was unreachable.

Generated code is cheaper and more consistent on SBF. The compact-enum support helpers carry `#[inline(always)]` like their compact-struct twins, the compact `Ref` accessors and `Mut` setters carry `#[inline]`, and staged vector setters validate through `ZcValidate::validate_slice` so a trivially valid element removes the walk exactly as it does in the runtime and patch paths. The offset walk and prefix decode are unchanged by design; their validate-before-use licenses are now stated once on `compute_offset_tokens` and `read_len_expr`, including the associativity argument and which edits would require re-deriving it.

The generated compact `Ref`, `Mut`, and `Patch` surface now emits rustdoc, matching the fixed generator's precedent, and the derive crate's internal public items explain why each exists. The mdt graph grows a `derive-contracts` template: the compact field grammar, the initialize-zeroing contract, the Pina `UpdateResizableAccount` snippet, and the prefix-attribute rejection are each defined once and consumed wherever they were previously hand-copied, including the derive README. The feature table now lists `compact-commit-full-validation` and `kani`, install snippets say `pinapod = "0.4"`, and the benchmark page names Criterion 0.8.2 and carries a current measured comparison, matching the lockfile.

## [0.4.4](https://github.com/pina-rs/pinapod/releases/tag/pinapod/v0.4.4) (2026-09-24)

Grouped release for `pinapod-workspace`.

### Features

#### re-export the pinned `fixed` crate from the `fixed` feature

_Packages:_ _pinapod_

The `fixed` feature now re-exports its dependency as `pinapod::fixed`, so a consumer names the mapped fixed-point types through PinaPod instead of declaring its own `fixed` requirement. `use pinapod::fixed::types::I16F16;` now compiles with `pinapod` as the only dependency, and the book's fixed-point section documents that form.

The re-export matters because the version is an exact pin. `fixed` is held at 1.30.0 since later releases require a newer compiler than PinaPod's Rust 1.89 baseline, and a schema field's `ZcField` mapping belongs to the types of whichever `fixed` release the program resolves. A consumer that declares its own `fixed` therefore owns a requirement it cannot get wrong safely: an incompatible spelling such as `fixed = "1.31"` fails Cargo's resolver outright, and a compatible one such as `fixed = "1"` compiles but obliges the consumer to keep tracking PinaPod's pin. Routing the import through the feature removes the question — `pinapod` supplies the `fixed` it maps, and a program declares `pinapod` and nothing else.

A compile fixture at `pinapod/tests/fixed_reexport` depends on `pinapod` and nothing else and exercises the re-exported path in a derived schema across the 16-, 32-, and 128-bit storage widths. It is run by a test alongside the `renamed_dependency` fixture and fails to compile if the re-export disappears, so the guarantee is checked rather than documented only. The `changeset-policy` release flow refreshes that fixture's lock file with every version bump, matching the existing renamed-dependency entry.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #42](https://github.com/pina-rs/pinapod/pull/42)

### Notes

#### re-export the pinned `fixed` crate from the `fixed` feature

_Packages:_ _pinapod-derive_

The `fixed` feature now re-exports its dependency as `pinapod::fixed`, so a consumer names the mapped fixed-point types through PinaPod instead of declaring its own `fixed` requirement. `use pinapod::fixed::types::I16F16;` now compiles with `pinapod` as the only dependency, and the book's fixed-point section documents that form.

The re-export matters because the version is an exact pin. `fixed` is held at 1.30.0 since later releases require a newer compiler than PinaPod's Rust 1.89 baseline, and a schema field's `ZcField` mapping belongs to the types of whichever `fixed` release the program resolves. A consumer that declares its own `fixed` therefore owns a requirement it cannot get wrong safely: an incompatible spelling such as `fixed = "1.31"` fails Cargo's resolver outright, and a compatible one such as `fixed = "1"` compiles but obliges the consumer to keep tracking PinaPod's pin. Routing the import through the feature removes the question — `pinapod` supplies the `fixed` it maps, and a program declares `pinapod` and nothing else.

A compile fixture at `pinapod/tests/fixed_reexport` depends on `pinapod` and nothing else and exercises the re-exported path in a derived schema across the 16-, 32-, and 128-bit storage widths. It is run by a test alongside the `renamed_dependency` fixture and fails to compile if the re-export disappears, so the guarantee is checked rather than documented only. The `changeset-policy` release flow refreshes that fixture's lock file with every version bump, matching the existing renamed-dependency entry.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #42](https://github.com/pina-rs/pinapod/pull/42)

## [0.4.3](https://github.com/pina-rs/pinapod/releases/tag/pinapod/v0.4.3) (2026-09-21)

Grouped release for `pinapod-workspace`.

### Fixes

#### keep commit-entry revalidation safe at layout cost

_Packages:_ _pinapod_, _pinapod-derive_

0.4.2 added a full `PinaPodCompact::validate` walk to the top of every generated `commit()`. The hardening was right, but the walk was stronger than the pointer arithmetic it guards. Relocation reads exactly three things from the buffer — `data.len()`, the header size, and the stored length prefixes — and then moves bytes between the offsets they imply. The only hazard is an offset or end computed past the buffer, which is a layout property. The full walk additionally visited every tail element and checked UTF-8, enum ranges, and `PodBool` tags: bytes `commit` never interprets, at a cost proportional to the element count rather than the prefix count.

On Pina's `compact_accounts_program` benchmark that measured **+308 CU on `write`, +292 on `resize`, +157 on `rename`, and +46 on `initialize`** against 0.4.1, tracking how many tails each instruction walks. Two changes remove it.

`ZcValidate::validate_slice` joins `validate_array` as an overridable element walk, and every trivially valid type overrides it to a no-op alongside its existing `validate_array` override. Generated tail loops call it instead of spelling the loop at each emission site, so a `Vec<u64>` or `PodVec<u8>` tail no longer burns CU proving `Ok(())` per element — a loop that is merely dead after inlining is not reliably removed at `-C opt-level=3` on SBF. The nested-container walk inside `PodVecRepr::validate_ref` routes through it as well.

`PinaPodCompact::validate_layout` is the commit entry now. It is a _provided_ method defaulting to the full `validate`, so a hand-written implementation cannot weaken itself by accident and existing ones keep compiling unchanged. The derive overrides it with the prefix decode, the capacity check, and the chained bounds, and emits it from the same per-field fragments as `validate` so a bound cannot be tightened in one walk and left stale in the other. The four semantic boundaries — the `Ref` and `Mut` constructors, `updated_len`, and `try_initialize`'s post-commit check — still run the full `validate`, as do the patch-input validations.

Measured against 0.4.1 on the same benchmark, the regression is recovered and three of four instructions are faster: −39 CU on `initialize`, −110 on `resize`, −104 on `write`, and +36 on `rename` — all inside the gate's noise band, which warns only at +250 CU and +5%.

One behavior refinement is deliberate and documented. A buffer that is layout-valid but semantically invalid — invalid UTF-8 in an untouched tail, say — is no longer rejected by `commit`. It is still rejected at the next read. On the generated update path the stronger invariant still holds by construction: `updated_len` runs the full `validate` over the same bytes the commit will see, because the staged edits are `(ptr, len)` intents that have not been written yet. `try_initialize` keeps its post-commit full `validate`, so initialization cannot persist such bytes either. The new `compact-commit-full-validation` feature restores the full walk at the commit entry for consumers with different CU budgets; it dispatches from the runtime crate rather than a `#[cfg]` in the generated body, because a `cfg` in an expansion resolves against the consumer's feature namespace, where the name is usually undefined and the feature would silently mean nothing.

The measurement, the rejected proof-token alternative, and the residual behavior change are recorded in the book's performance decision log.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #39](https://github.com/pina-rs/pinapod/pull/39) · _Closed issues:_ [#38](https://github.com/pina-rs/pinapod/issues/38) · _Related issues:_ [#485](https://github.com/pina-rs/pinapod/issues/485)

## [0.4.2](https://github.com/pina-rs/pinapod/releases/tag/pinapod/v0.4.2) (2026-09-19)

Grouped release for `pinapod-workspace`.

### Features

#### validate the buffer inside every generated compact commit

_Packages:_ _pinapod_, _pinapod-derive_

A generated compact writer held a construction-time proof that its buffer was a valid representation, but the proof was call-site discipline rather than something the commit itself re-established: `new_unchecked` exists for the patch paths, and a future construction site could have skipped validation and committed over stale or tampered bytes. Upstream ZeroPod's 0.3.6 safety release (PR #34) fixed the same class by revalidating in `commit`, and this ports that hardening.

Every generated `commit`, tail-bearing or tail-free, now runs the schema's `PinaPodCompact::validate` over the buffer as its first statement, before any offset arithmetic or pointer work, so a stale or tampered writer fails closed instead of relocating tails over bytes that were never validated. The cost is one additional validation walk per commit.

The preflight-consistency `debug_assert_eq!` in `Patch::update` and `Patch::try_initialize` is now a release-mode check as well: if the preflighted length and the committed length ever disagree — which can only indicate a defect in the generated code — the patch returns `InvalidLength` instead of trusting and persisting a length the bytes do not support. The `PinaPodPatch::update` documentation names that error path. The full upstream review, including every already-covered item, is recorded in the book's new upstream review log.

One ordering consequence surfaced in review and is covered by a regression: `try_initialize` now writes the patch's inline values before calling `commit`, because the destination starts zeroed and an inline field such as a one-based enum can have an invalid all-zero representation — validating before those writes would reject a legitimate initialize. `update` keeps the opposite order (commit, then inline writes) so a rejected update still leaves the account byte-identical.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #34](https://github.com/pina-rs/pinapod/pull/34) · _Related issues:_ [#34](https://github.com/pina-rs/pinapod/issues/34)

#### keep optional features no_std and prove it in CI

_Packages:_ _pinapod_, _pinapod-derive_

The runtime crate is documented as `no_std`, but the `fixed` and `solana-address` optional dependencies re-enabled their crates' default features, and `fixed`'s defaults pull in `std`. A dependent enabling either feature therefore silently left the no_std guarantee. Both member dependencies now keep `default-features = false`, matching the workspace posture already used for `wincode` and `solana-program-error`.

The `build:no-default` job now also checks every optional feature combination (`fixed`, `floats`, `solana-address`, `solana-program-error`, `wincode`) against the bare-metal `thumbv7em-none-eabihf` target, where `std` does not exist. A host-side check could never prove the promise, and a future dependency bump cannot quietly break it again. Alongside this, the fuzz suite grew failure-path coverage (rejected-commit atomicity, retry after rejection, updates over corrupted bytes, compact enum patches, four- and eight-byte length prefixes), CI-local coverage artifacts are untracked, `libfuzzer-sys` is pinned, and the devenv flake input that supplies CI tooling is pinned by branch with its trust story documented in SECURITY.md.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #34](https://github.com/pina-rs/pinapod/pull/34) · _Related issues:_ [#34](https://github.com/pina-rs/pinapod/issues/34)

### Fixes

- _Packages:_ _pinapod_, _pinapod-derive_ **reject corrupt length prefixes at the Wincode write boundary.** The Wincode `SchemaWrite` implementations for `PodString` and `PodVec` emitted the stored length prefix verbatim. A container holding a corrupt prefix — one that decodes above its capacity, or wider than `usize` on a 32-bit target — therefore serialized into a wire value that fails on re-read: silent corruption at a format boundary rather than an error at write time. The option writer already rejected invalid tags; the string and vector writers now apply the matching check and fail with a `WriteError` naming the container, so a value that cannot round-trip is rejected where it is written instead of where it is read back. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #34](https://github.com/pina-rs/pinapod/pull/34) · _Related issues:_ [#34](https://github.com/pina-rs/pinapod/issues/34)

## [0.4.1](https://github.com/pina-rs/pinapod/releases/tag/pinapod/v0.4.1) (2026-09-17)

Grouped release for `pinapod-workspace`.

### Fixes

#### restore the byte-array fast path for `[u8; N]` validation

_Packages:_ _pinapod_, _pinapod-derive_

0.4.0 generalized the `[u8; N]`-specific `ZcValidate` impl into a `[T; N]` impl whose body walks every element. That walk is correct but its per-element work no longer disappears for byte arrays: at `-C opt-level=3` on SBF the optimizer inlines a neighboring restricted-domain check and duplicates a byte load instead of folding the loop away. A program whose only arrays are byte arrays therefore spends compute units it does not need to. Measured against a downstream `pina` checkout, `migrations_program/update` moved 533 to 537 CU in the original report; reproduced in-repo against `pina` at `430a2e95`, the same instruction moved 758 to 761 CU, and 0.3.3 measures 758 with every other input held constant.

Validation now dispatches through a new provided method, `ZcValidate::validate_array`, whose default implementation is the per-element walk. Element types whose every initialized bit pattern is valid override it with a no-op, and `[T; N]` forwards to it. A trivially valid element therefore has no loop in its instantiation at all, while `PodBool`, length-prefix-bearing containers, and every other restricted-domain element keep the walk unchanged — that walk remains the only gate rejecting a non-canonical bool byte, an out-of-range prefix, or non-UTF-8 string bytes. `u8`, `i8`, the integer pods, the float pods, and `solana_address::Address` take the no-op override.

The `ZcElem` safety contract now states the matching obligation: overriding `validate_array` with a no-op asserts that the element's whole domain is valid, exactly as a trivial `validate_ref` does, so an array of a restricted-domain type must keep the default walk. Wire format, sizes, and every rejected-input behavior are unchanged; only the per-element cost of a trivially valid array moves.

`ZcValidate` gains a method with a default body, so downstream implementors keep compiling without changes. Both crates bump together because the runtime crate's exact `=x.y.z` derive pin is what keeps generated code matched to the private contracts it expands against.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #32](https://github.com/pina-rs/pinapod/pull/32) · _Closed issues:_ [#29](https://github.com/pina-rs/pinapod/issues/29)

### Notes

#### fail on every Clippy warning and add `fix:all`

_Packages:_ _pinapod_, _pinapod-derive_

Clippy was run without `--all-targets`, so lint findings in tests, benchmarks, and compile fixtures never reached either `lint:clippy` or CI's `lint` job; only the library and binary targets were covered. `lint:clippy` now lints every target with `-D warnings`, so the local gate and CI agree and neither can hide a warning in a test file. Every finding the wider sweep surfaced is fixed rather than suppressed — one targeted `#[allow(clippy::clone_on_copy)]` is the only new suppression, on a test whose purpose is to exercise `clone` on a `Copy` type. The fixes are mechanical: deprecated `criterion::black_box` imports become `std::hint::black_box`, boolean `assert!` comparisons become `assert_eq!` / `assert_ne!`, and raw-pointer `as` casts become `.cast()` or `ptr::from_ref`. The manifest lints are resolved by inheriting `keywords` and dropping the `homepage` key that Cargo reports as redundant with `repository`, rather than deleting the metadata.

One of those findings sits in the derive: an `impl Schema` block had been left after the `#[cfg(test)] mod tests` block, which trips `clippy::items_after_test_module` now that the lint reaches lib targets. The impl moved above the tests; it is a pure reordering with no behavior change.

A new `fix:all` devenv task applies the whole fix suite in one command: `fix:clippy` across every target, then `docs:sync`, then `fix:format`. The explicit `docs:sync` is what makes the ordering correct rather than incidental — `fix:format` runs dprint before its own `docs:sync`, so an mdt rewrite performed inside it would land after formatting and never be formatted itself. Syncing ahead of the formatting pass means the final `fix:format` formats the blocks mdt produces, and the tree is left clean for `lint:*`. Running it twice produces no further changes.

`CARGO_BUILD_JOBS` is capped at 4 in the shell environment. `clippy --fix` re-runs the compiler to a fixpoint and the docs and test tasks each rebuild the workspace, so letting every invocation size its job pool to all 12 cores made a local `fix:all` saturate the machine. The cap is deliberately not expressed through `RUSTFLAGS`, which would invalidate the whole build cache and force a full rebuild.

The version bumps that introduced this work had also silently dropped the `zeropod` git pin: the workspace table declared a crates.io version with no `git`/`rev`, so Cargo ignored the member's pin (with an "unused manifest key" warning) and the benchmark comparison baseline had drifted from the pinned revision to crates.io 0.3.6. The git source now lives in the workspace table, which is the only place it can live for a member that consumes it via `workspace = true`. The 14 trybuild UI snapshots are re-blessed for column-number shifts caused by the `syn` 2 to 3 bump; the diffs are location lines only, with no change to any diagnostic message.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #30](https://github.com/pina-rs/pinapod/pull/30)

#### make the local Kani profile actually run proofs

_Packages:_ _pinapod_

`devenv --profile kani shell` could not run a single harness on macOS. Two separate faults were stacked behind each other.

The profile linked Kani against `nightly-2025-11-21`, but the packaged Kani 0.68.0 is built against `nightly-2026-08-21`. `kani-compiler` aborted on startup with `dyld: Library not loaded: @rpath/librustc_driver-83914c2c3aa68f2a.dylib`, so no harness ever reached verification. The toolchain pin now matches the nightly named by Kani's own `rust-toolchain-version`, and a comment records how to read that value when the nixpkgs pin is next bumped.

With the toolchain corrected, CBMC's `goto-cc` failed next with `execvp gcc failed: No such file or directory`. It preprocesses each harness by invoking a native C compiler under the literal name `gcc`, which a Darwin machine does not have. The profile now ships a `gcc` shim that execs clang; `goto-cc` parses the preprocessed C itself and never links the result, so the substitution is safe. `--native-compiler clang` cannot be threaded through `cargo-kani`, which is why the shim supplies the name instead.

`cargo-kani` now verifies `bool::kani_proofs` (3 harnesses) and `u32_proofs` (10 harnesses) locally, so a red CI shard can be reproduced on a laptop. The shard scripts also reject a missing harness argument rather than silently running the whole proof suite, and the timing script no longer runs a stray `cargo metadata` whose output it discarded.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #30](https://github.com/pina-rs/pinapod/pull/30)

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
