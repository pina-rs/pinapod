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

| Reviewed failure                                                             | Permanent check                                                                                                                                                                                          | What it proves                                                                                               |
| ---------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| A default string or vector copied uninitialized capacity                     | `container_initialization::assigning_default_containers_keeps_every_account_byte_initialized` and `compact_copy_initializes_inactive_capacity_in_nested_containers`                                      | Fresh and nested container representations contain initialized bytes                                         |
| An absent option exposed unchecked payload bytes as a valid `T`              | `container_initialization::absent_options_do_not_expose_or_validate_inactive_payloads` and the `PodOption::value_unchecked` compile-fail example                                                         | Safe code cannot borrow an inactive semantic value                                                           |
| A shorter string or vector retained the removed value                        | `container_initialization::shortening_or_clearing_a_vector_zeroes_removed_values` and `shortening_or_clearing_a_string_zeroes_removed_bytes`                                                             | Removed fixed-container bytes become zero                                                                    |
| Wincode serialized inactive or nested capacity inconsistently                | `wincode_serialize::serialization_does_not_disclose_truncated_capacity` and its nested-container round trips                                                                                             | Serialization is fixed-size, recursive, and canonical                                                        |
| A fixed reader silently accepted trailing data                               | `fixed_pina_pod::fixed_exact_reads_reject_trailing_bytes`                                                                                                                                                | Exact and prefix reads have different contracts                                                              |
| Zero was not a valid enum value during initialization                        | The three `fixed_pina_pod::initialization_*` tests                                                                                                                                                       | Configuration runs before validation, and failures zero the destination                                      |
| A derive replayed enum discriminant expressions in an invalid context        | `fixed_pina_pod::enum_uses_compiler_evaluated_discriminants`, `ui/pass/enum_discriminant_expressions.rs`, and `ui/pass/compact_enum_discriminant_expressions.rs`                                         | The generated code uses compiler-evaluated discriminants                                                     |
| A caller-local type named like a Rust primitive received a built-in mapping  | `ui/fail/primitive_name_shadowing.rs`                                                                                                                                                                    | A type name alone cannot grant a representation contract                                                     |
| A malformed compact tail escaped its slice through offset or length overflow | Compact overflow and capacity regressions in `compact_backend.rs`                                                                                                                                        | Checked arithmetic rejects the bytes before a pointer operation                                              |
| A compact writer accepted an invalid supplied element                        | Compact mutation validation regressions in `compact_backend.rs`                                                                                                                                          | Patch preflight rejects invalid values without changing the destination                                      |
| Compact initialization failed after receiving nonzero destination bytes      | `compact_backend::compact_initialize_zeroes_the_destination_after_an_error`                                                                                                                              | Failed initialization leaves the complete destination zeroed                                                 |
| Safe code forged a generated compact view, patch, or removed mutable view    | `ui/fail/compact_ref_construction_private.rs`, `compact_header_construction_private.rs`, `compact_patch_construction_private.rs`, `compact_patch_metadata_private.rs`, and `compact_mut_not_exported.rs` | Only generated constructors create views, patch metadata stays private, and the mutable view is not exported |
| A framework-owned field leaked into a generated patch                        | `ui/pass/compact_skip_patch.rs`, `ui/fail/compact_skip_patch_builder.rs`, and `ui/pass/compact_inline_only_patch.rs`                                                                                     | `skip_patch` omits its builder while patches remain usable for dynamic and inline-only schemas               |
| A compact shrink retained an old tail suffix                                 | Compact shortening regressions in `compact_backend.rs`                                                                                                                                                   | Bytes from the old encoded suffix become zero                                                                |
| An eight-byte fixed prefix truncated on a 32-bit target                      | `validation::validate_rejects_eight_byte_lengths_that_do_not_fit_usize` and the i686 CI job                                                                                                              | Length conversion fails instead of accepting a truncated prefix                                              |
| A dishonest exact-size iterator partially changed an existing vector         | `pod_types::pod_vec_slice_like_setters_are_atomic_on_overflow`                                                                                                                                           | Slice-like setters know the complete length and leave bytes unchanged on overflow                            |
| A prefix width or capacity could not fit its representation                  | The seven `ui/fail/prefix_*.rs` and `ui/fail/fixed_prefix_*.rs` fixtures                                                                                                                                 | Invalid prefixes fail during compilation                                                                     |
| A compact vector used a zero-sized or dynamically nested element             | `ui/fail/zero_sized_compact_vec.rs` and `ui/fail/unsupported_compact_nesting.rs`                                                                                                                         | Unsupported layouts fail with schema-specific guidance                                                       |
| Generated names collided with a schema lifetime or a renamed dependency      | `ui/pass/compact_lifetime_a.rs` and `renamed_dependency.rs`                                                                                                                                              | Macro hygiene does not depend on one lifetime or Cargo dependency spelling                                   |

Keep this map current when an API change moves a test. Do not delete a regression because a public method changed. Port the smallest reproducer to the replacement API.

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
