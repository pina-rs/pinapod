# Migrate from v0.2 to v0.3

Version 0.3 preserves v0.2 account and instruction bytes exactly. Wire compatibility needs no on-chain data migration. The source changes are small, but callers must also review zero-capacity emptiness behavior and generated compact `Ref` size assertions.

## Update the dependency

```toml
[dependencies]
pinapod = "0.3"
```

The runtime crate now pins `pinapod-derive` to its exact released version, so a `pinapod` upgrade always carries its matching derive. Projects that vendor or fork the crates should keep the two versions in lockstep.

## Add a wildcard arm to `PinaPodError` matches

`PinaPodError` is now `#[non_exhaustive]`. An exhaustive `match` no longer compiles:

```rust
match error {
    PinaPodError::BufferTooSmall => retry(),
    PinaPodError::Overflow => return Err(Overflow),
    # PinaPodError::InvalidBool
    # | PinaPodError::InvalidTag
    # | PinaPodError::InvalidDiscriminant
    # | PinaPodError::InvalidLength
    # | PinaPodError::InvalidUtf8 => revert(),
}
```

Keep a wildcard arm so later minor releases can add validation variants without breaking your build:

```rust
match error {
    PinaPodError::BufferTooSmall => retry(),
    PinaPodError::Overflow => return Err(Overflow),
    other => revert(other),
}
```

Code that compares with `==`, `assert_eq!`, or `matches!` needs no change. `PinaPodError` also implements `core::error::Error` in v0.3, so it composes with `?` and error-reporting crates without an adapter.

## Widen option tag inspection to `u64`

`PodOption::raw_tag` returned `u32`; it now returns `u64`. Only code that inspects raw tags is affected:

```rust
let tag: u64 = option.raw_tag();
assert_eq!(tag, 1);
```

Safe accessors such as `get`, `get_ref`, `is_some`, and `tag_valid` are unchanged. The wider return type lets `PodOption` accept eight-byte prefixes without truncating a hostile tag into a valid value.

## Revisit zero-capacity `is_empty` expectations

`PodString::is_empty` and `PodVec::is_empty` now report the capacity-clamped length, matching `len`. The difference is observable only on zero-capacity containers whose length prefix holds a corrupt non-zero value: `String<0>` and `Vec<T, 0>` now report `is_empty() == true` for such bytes. Readers rejected those bytes before and still reject them; only the pre-validation accessor changed.

## Expect larger generated compact views

Generated compact `Ref` structs cache one tail offset per dynamic field, adding one machine word per tail to each view (the six-tail benchmark fixture stores six offsets). Schema code does not change, and accessors are now constant time regardless of how many tails precede the field. Code that asserts an exact `size_of` on a generated view must update the constant.

## What did not change

- Wire bytes: fixed and compact layouts, prefix widths, option tags, enum discriminants, and wincode encodings are identical to v0.2.
- The MSRV stays Rust 1.89.
- All v0.2 reader, patch, and initialization APIs keep their names and signatures, except the items above.
- `PodOption` additionally accepts the eight-byte prefix that strings and vectors already supported: `PodOption<T, 8>` is new, not a replacement.

## Verify the migration

```sh
devenv shell test:all
devenv shell test:miri
```

For Pina, no client regeneration is required: the wire format and the generated account layouts are unchanged. Re-run the cross-language fixtures if your fork also carries local schema changes.
