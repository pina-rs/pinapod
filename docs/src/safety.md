# Safety model and regressions

PinaPod uses unsafe code to form typed references over account bytes. The public safe API must prove every condition required by those casts. Validation is part of memory safety, not an optional data-quality check.

## Safe readers establish the boundary

A safe fixed reader checks the slice length and recursively validates the mapped representation. A safe compact reader first checks the physical allocation against the schema's minimum, maximum, and tail granularity. It then checks the header, option tags, length prefixes, field capacities, arithmetic, element values, UTF-8, and every source range before it exposes a field.

The reader owns the proof for the lifetime of its borrow. Code cannot construct a generated compact reader from fields or mutate its stored length metadata.

PinaPod separates two facts:

- Fully initialized bytes can still contain a semantically invalid value.
- A semantically valid active value does not prove that inactive capacity was initialized.

Constructors and updates address both facts. They initialize all destination bytes, then validate active values before returning success.

## Unsafe traits carry representation contracts

`ZcElem`, `ZcField`, `PinaPodFixed`, and `PinaPodCompact` are unsafe extension points. Their contracts cover alignment, padding, bit validity, size, and validation. Prefer `#[derive(PinaPod)]` for schema types.

`PinaPodFixed` has no size constant. Direct derives expose inherent `Type::SIZE`, calculated from `size_of::<TypeZc>()`. Generic code derives the size from `size_of::<T::Zc>()`. `ZcField` has no separate size constant. An implementation cannot claim that a one-byte object contains two readable bytes.

`PinaPodCompact` owns the physical storage contract through `MIN_SIZE`, `MAX_SIZE`, `TAIL_ALIGNMENT`, and `validate_storage_len`. Its validator must enforce that contract before it inspects the representation.

Unchecked readers remain unsafe. Their caller must prove all documented slice and representation requirements. They are not a faster substitute at an account boundary.

## Writes either finish or leave canonical bytes

Fixed `initialize` zeros the destination before configuration. If configuration or validation fails, it zeros the destination again.

A compact patch performs all capacity, arithmetic, and supplied-value checks before it changes account data. When an update shortens the encoded value, PinaPod zeros the old suffix. Container operations also zero removed values and inactive option payloads.

This policy prevents stale payload disclosure and makes repeated writes produce the same bytes.

## Every reported unsoundness path has a regression

The v0.2 review converted each confirmed path into a native test, a Miri test, a compile-fail test, or a Kani proof. The table points to the closest permanent check.

| Reviewed failure                                                                            | Permanent check                                                                                                                                                                                                                        | What it proves                                                                                                                  |
| ------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| A default string or vector copied uninitialized capacity                                    | `container_initialization::assigning_default_containers_keeps_every_account_byte_initialized` and `compact_copy_initializes_inactive_capacity_in_nested_containers`                                                                    | Fresh and nested container representations contain initialized bytes                                                            |
| An absent option exposed unchecked payload bytes as a valid `T`                             | `container_initialization::absent_options_do_not_expose_or_validate_inactive_payloads` and the `PodOption::value_unchecked` compile-fail example                                                                                       | Safe code cannot borrow an inactive semantic value                                                                              |
| A shorter string or vector retained the removed value                                       | `container_initialization::shortening_or_clearing_a_vector_zeroes_removed_values` and `shortening_or_clearing_a_string_zeroes_removed_bytes`                                                                                           | Removed fixed-container bytes become zero                                                                                       |
| Wincode serialized inactive or nested capacity inconsistently                               | `wincode_serialize::serialization_does_not_disclose_truncated_capacity` and its nested-container round trips                                                                                                                           | Serialization is fixed-size, recursive, and canonical                                                                           |
| A fixed reader silently accepted trailing data                                              | `fixed_pina_pod::fixed_exact_reads_reject_trailing_bytes`                                                                                                                                                                              | Exact and prefix reads have different contracts                                                                                 |
| Zero was not a valid enum value during initialization                                       | The three `fixed_pina_pod::initialization_*` tests                                                                                                                                                                                     | Configuration runs before validation, and failures zero the destination                                                         |
| A derive replayed enum discriminant expressions in an invalid context                       | `fixed_pina_pod::enum_uses_compiler_evaluated_discriminants`, `ui/pass/enum_discriminant_expressions.rs`, and `ui/pass/compact_enum_discriminant_expressions.rs`                                                                       | The generated code uses compiler-evaluated discriminants                                                                        |
| A caller-local type named like a Rust primitive received a built-in mapping                 | `ui/fail/primitive_name_shadowing.rs`                                                                                                                                                                                                  | A type name alone cannot grant a representation contract                                                                        |
| A malformed compact tail escaped its slice through offset or length overflow                | Compact overflow and capacity regressions in `compact_backend.rs`                                                                                                                                                                      | Checked arithmetic rejects the bytes before a pointer operation                                                                 |
| A compact writer accepted an invalid supplied element                                       | Compact mutation validation regressions in `compact_backend.rs`                                                                                                                                                                        | Patch preflight rejects invalid values without changing the destination                                                         |
| Compact initialization failed after receiving nonzero destination bytes                     | `compact_backend::compact_initialize_zeroes_the_destination_after_an_error`                                                                                                                                                            | Failed initialization leaves the complete destination zeroed                                                                    |
| Safe code forged a generated compact view, patch, or removed mutable view                   | `ui/fail/compact_ref_construction_private.rs`, `compact_header_construction_private.rs`, `compact_patch_construction_private.rs`, `compact_patch_metadata_private.rs`, and `compact_mut_not_exported.rs`                               | Only generated constructors create views, patch metadata stays private, and the mutable view is not exported                    |
| A framework-owned field leaked into a generated patch                                       | `ui/pass/compact_skip_patch.rs`, `ui/fail/compact_skip_patch_builder.rs`, and `ui/pass/compact_inline_only_patch.rs`                                                                                                                   | `skip_patch` omits its builder while patches remain usable for dynamic and inline-only schemas                                  |
| A compact shrink retained an old tail suffix                                                | Compact shortening regressions in `compact_backend.rs`                                                                                                                                                                                 | Bytes from the old encoded suffix become zero                                                                                   |
| An eight-byte fixed prefix truncated on a 32-bit target                                     | `validation::validate_rejects_eight_byte_lengths_that_do_not_fit_usize` and the i686 CI job                                                                                                                                            | Length conversion fails instead of accepting a truncated prefix                                                                 |
| A dishonest exact-size iterator partially changed an existing vector                        | `pod_types::pod_vec_slice_like_setters_are_atomic_on_overflow`                                                                                                                                                                         | Slice-like setters know the complete length and leave bytes unchanged on overflow                                               |
| A prefix width or capacity could not fit its representation                                 | The seven `ui/fail/prefix_*.rs` and `ui/fail/fixed_prefix_*.rs` fixtures                                                                                                                                                               | Invalid prefixes fail during compilation                                                                                        |
| A compact vector used a zero-sized or dynamically nested element                            | `ui/fail/zero_sized_compact_vec.rs` and `ui/fail/unsupported_compact_nesting.rs`                                                                                                                                                       | Unsupported layouts fail with schema-specific guidance                                                                          |
| Generated names collided with a schema lifetime or a renamed dependency                     | `ui/pass/compact_lifetime_a.rs` and `renamed_dependency.rs`                                                                                                                                                                            | Macro hygiene does not depend on one lifetime or Cargo dependency spelling                                                      |
| A generated commit trusted its construction-time validation instead of re-establishing it   | `compact::tests::generated_commits_revalidate_the_buffer_before_relocating_tails` (a derive unit test over the emitted tokens)                                                                                                         | Every generated `commit`, tail-bearing or tail-free, validates the buffer before its offset walk                                |
| Commit-time revalidation rejected a one-based inline enum during initialize                 | `compact_backend::compact_initialize_writes_inline_enums_before_commit_validates`                                                                                                                                                      | A patch writes its inline values before `commit` validates, so the zeroed-start rule for restricted-domain fields still holds   |
| A patch preflight and its commit could disagree about the new length in release builds      | `compact::tests::generated_updates_fail_closed_when_preflight_and_commit_lengths_diverge`, plus the preflight-equals-commit assertions in the `compact_patch` and `compact_enum_patch` fuzz targets                                    | Divergence is a release-mode `InvalidLength` error rather than a silently wrong length                                          |
| A Wincode writer copied a corrupt length prefix verbatim into the wire format               | `wincode_serialize::string_writer_rejects_a_length_prefix_above_its_capacity` and `vec_writer_rejects_a_length_prefix_above_its_capacity`                                                                                              | Serialization rejects a prefix that cannot round-trip, matching the option writer's tag check                                   |
| A rejected compact update could leave the account unusable for the next patch               | `compact_commit_shift::compact_patch_retry_after_a_rejected_update_still_commits` and `compact_backend::compact_wide_tagged_union_patch_rejects_over_capacity_atomically`                                                              | A fitting patch still commits over the exact bytes a rejected patch left behind                                                 |
| Editing only the last tail was untested while earlier tails sat near capacity               | `compact_commit_shift::compact_patch_last_tail_edits_preserve_large_earlier_tails`                                                                                                                                                     | Grow, shrink, and empty edits of the final tail preserve large earlier tails                                                    |
| Reinitializing an existing valid account was untested                                       | `compact_backend::compact_initialize_over_an_existing_valid_account_rewrites_every_byte`                                                                                                                                               | A second initialize produces exactly the new value and zeroes the vacated suffix                                                |
| Zero-capacity containers were untested                                                      | `pod_types::zero_capacity_containers_accept_only_empty_values`                                                                                                                                                                         | `String<0>` and `Vec<_, 0>` hold only empty values and still validate                                                           |
| A layout-only commit entry could have dropped a bound it must keep                          | `compact::tests::generated_layout_walk_keeps_every_bound_and_drops_every_semantic_check`, plus the derive's `validate_layout`/`validate` pair being emitted from one set of per-field fragments                                        | Every chained tail bound and tag rejection survives in the layout walk, and no semantic check leaks back in                     |
| A hand-written `PinaPodCompact` impl could have weakened itself through the new method      | `validation::validate_layout_defaults_to_the_full_walk_for_hand_written_impls`                                                                                                                                                         | The provided `validate_layout` defaults to the full `validate`, so an impl that does not override it keeps every semantic check |
| A corrupt prefix at commit entry could have relocated tails over unchecked bytes            | `compact_backend::compact_commit_fails_closed_on_a_corrupt_prefix_at_entry`, `validate_layout_rejects_a_prefix_above_its_capacity`, and `validate_layout_rejects_a_tail_end_past_the_buffer`                                           | `commit` still fails closed on a corrupt prefix, a short buffer, and a tail end past the allocation                             |
| The documented layout-versus-semantics split could have drifted from the reader's behavior  | `compact_backend::validate_layout_accepts_a_semantically_invalid_but_readable_layout`, `compact_enum_validate_layout_accepts_a_readable_but_invalid_payload`, and `validate_layout_matches_the_layout_half_of_validate_on_valid_bytes` | Relocation accepts a readable layout, and every value-exposing boundary still rejects the invalid content                       |
| The commit-entry depth feature could have meant nothing where a consumer does not define it | `feature_gate::commit_entry_depth_follows_the_feature`, run both with and without `compact-commit-full-validation`                                                                                                                     | The depth dispatch resolves against PinaPod's features, not the consumer's, and the feature flips the check in both directions  |

Keep this map current when an API change moves a test. Do not delete a regression because a public method changed. Port the smallest reproducer to the replacement API.

## Upstream review log

PinaPod is a fork of [ZeroPod](https://github.com/blueshift-gg/zeropod) that reviews upstream changes before porting them. Every upstream safety release is compared against this codebase, and the comparison is recorded here so the next review starts from a known state.

### ZeroPod 0.3.6 (upstream PR #34, reviewed 2026-09-19)

Upstream's safety release fixed uninitialized storage disclosure, generated-view fabrication, missing commit revalidation, adjacent-vector offset arithmetic, unchecked narrowing, and unvalidated Wincode input. Review outcome: the fork's v0.2 redesign had already fixed or avoided every item through a different API shape, with one hardening gap that was ported.

| Upstream fix                                                | PinaPod status at review                                                                                                     |
| ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `MaybeUninit::uninit()` inactive capacity disclosure        | Already zeroed: constructors zero, shrinking operations zero vacated bytes, `initialize` zero-fills first                    |
| Safe `value_unchecked` exposing inactive option payloads    | Never existed; only `unsafe assume_init_ref`, with a compile-fail example                                                    |
| `decode_len` truncating `u64 as usize` on 32-bit targets    | `try_decode_len` rejects lengths wider than `usize`; i686 CI job and regressions                                             |
| `try_extend_from_slice` overflow                            | `checked_add` with a capacity filter                                                                                         |
| `PodVec::remove` overlapping copy                           | `ptr::copy` (memmove semantics) since the fork                                                                               |
| Wincode zero-copy reads bypassing validation                | Readers validate through `ZcValidate`; no `ZeroCopy` impls; writers emit active bytes plus canonical zero padding            |
| Fabricated views and redirected pending edits (`ViewState`) | Module privacy: view fields are private in a child module and the mutable view is not exported; compile-fail fixtures pin it |
| Commit-time header revalidation                             | **Ported at review.** Generated `commit` now validates its buffer before the offset walk (regression row above)              |
| `total_len` confused the allocation with the encoded length | The fork tracks the encoded length from the prefixes                                                                         |
| Checked offset and narrowing arithmetic                     | Checked helpers plus compile-time capacity bounds that license the fast paths for narrow prefixes                            |
| Derived prefix-width validation at the derive entry         | `validate_dynamic_prefix_args` runs on struct fields and compact-enum payloads                                               |

Known divergence: upstream restricts `PodOption` prefixes to at most four bytes. PinaPod accepts an eight-byte option prefix because its `encode_tag`/`decode_tag` round-trip the full `u64` tag, so no narrowing exists to protect against.

## Performance decision log

Performance shapes that were measured and decided stay recorded here so a later review does not re-propose a rejected change with the same justification.

### Compact reader offsets stay uncached (measured on-chain, revisited 2026-09-19)

PinaPod 0.3.0 cached every tail's start offset inside the generated compact `Ref` at construction, making each accessor O(1) instead of re-walking the preceding length prefixes. On-chain CU measurement across Pina's example programs showed the cache never reduced any measured read instruction, while update instructions paid for the wider view and its invalidation rules. `07578ba` returned the readers to uncached accessors and kept the flat, uncached preflight walk (priced at +3 CU on a write and +75 CU on a resize, a cost the atomic update contract accepts); the wire format and every safety property were unchanged.

The 2026-09-19 audit re-raised the offset cache and the related idea of making `validate` return the encoded length to fold away the construction walk. Both stay rejected on that measurement: the first was tried and reverted, and the second is a breaking `PinaPodCompact` change whose benefit is one construction-time walk — the same single-digit CU class the earlier measurement priced a full walk at. The commit-time revalidation added at that same review costs one additional validation walk per commit and is documented in its changeset.

### Commit-entry revalidation moved to a layout-only walk (measured on-chain, 2026-09-21)

The commit-entry revalidation added by the 0.4.2 hardening ran the full `PinaPodCompact::validate` on every `commit`. That walk is stronger than the pointer arithmetic it guards. Relocation reads exactly three things from the buffer — `data.len()`, the header size, and the stored length prefixes — then performs checked adds and moves bytes between the offsets they imply. Copying arbitrary bytes is safe, so the only hazard is an offset or end computed past the buffer: a _layout_ precondition, not a semantic one. The full walk additionally visits every tail element and checks UTF-8, enum ranges, and `PodBool` tags — bytes `commit` never interprets, and work proportional to the number of elements rather than the number of prefixes.

Pina's `compact_accounts_program` benchmark priced the added walk at **+308 CU on `write`, +292 on `resize`, +157 on `rename`, and +46 on `initialize`** against 0.4.1, because `write` and `resize` walk four tails, `rename` two, and `initialize` none. Two changes removed that cost:

1. **Trivial element walks are stated, not looped.** `ZcValidate::validate_slice` joins `validate_array` as an overridable walk, and `impl_zc_validate_trivial!` plus the `u8`, `i8`, float, and `Solana` `Address` impls override it to a no-op. A `Vec<u64>` or `PodVec<u8>` tail previously burned CU proving `Ok(())` per element, because a loop that is merely dead after inlining is not reliably removed at `-C opt-level=3` on SBF.
2. **`commit` proves layout, not semantics.** `PinaPodCompact::validate_layout` is a provided method defaulting to the full `validate`, so a hand-written impl cannot weaken itself by accident; the derive overrides it with the prefix decode, capacity, and chained bounds and no element iteration. Both depths are emitted from the same per-field fragments, so a bound cannot be tightened in one walk and left stale in the other. The four semantic boundaries — the `Ref` and `Mut` constructors, `updated_len`, and `try_initialize`'s post-commit check — still run the full `validate`, as do the patch-input validations.

Measured on the same benchmark, the two changes together recover the regression and land **below 0.4.1 on three of the four instructions** (−39 CU on `initialize`, −110 on `resize`, −104 on `write`, +36 on `rename`). All four are inside the gate's noise band: the gate warns only at +250 CU _and_ +5%, and `rename`'s +36 CU is +0.78%.

The residual is the layout walk itself, which is irreducible while `commit` fails closed on a corrupt prefix. It was measured directly by building the same program with the commit-entry call removed: the check costs +15 CU on `initialize`, +22 on `rename`, +30 on `resize`, and +33 on `write`. A proof-token scheme that skips it on the generated update path was considered and rejected on that measurement — the residual attributable to the check is single-digit to low-double-digit CU per commit, and the token would add public API surface to generated types for it.

Residual behavior change, recorded because it is a deliberate refinement rather than a loss: a buffer that is layout-valid but semantically invalid (invalid UTF-8 in an _untouched_ tail, for example) is no longer rejected by `commit`. It is still rejected at the next read, and the generated update path preserves the "an update cannot persist semantically invalid bytes" invariant by construction — `updated_len` runs the full `validate` over the same bytes the commit will see, and the staged edits are `(ptr, len)` intents that have not been written yet, so commit receives byte-identical input to what was just validated. `try_initialize` keeps its post-commit full `validate`, so initialization cannot persist such bytes either.

`compact-commit-full-validation` restores the full walk at the commit entry for consumers with different CU budgets. It lives behind a runtime-side dispatch function rather than a `#[cfg]` in the generated body, because a `cfg` in an expansion resolves against the _consumer's_ feature namespace, where the name is usually undefined — the generated code would silently take the disabled branch and the feature would mean nothing.

## Run the checks

Run native and Miri suites from the repository root:

```sh
devenv shell test:all
devenv shell test:miri
```

`tests/compile_contracts.rs` runs the pass and compile-fail fixtures. Every compile-fail fixture has a checked `.stderr` diagnostic, so a less useful macro error also fails the suite. `tests/renamed_dependency.rs` builds its standalone fixture crate. Both compile-only drivers are excluded under Miri and run in the native suite instead.

CI runs Kani with:

```sh
cargo kani -p pinapod --features kani
```

Kani proves integer representation round trips, ordering, explicit checked, wrapping, and saturating arithmetic, prefix semantics, and inactive option handling over symbolic inputs. Miri exercises the real pointer casts and detects invalid references, out-of-bounds operations, and reads of uninitialized memory.

The ordinary native suite also compares v0.2 bytes with pinned PinaPod v0.1 and upstream ZeroPod fixtures in `wire_compatibility.rs`. Cross-language fixtures remain necessary. Miri can show that a Rust implementation is memory-safe while every generated client agrees on the wrong wire format.
