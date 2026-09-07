# Pod containers and prefixes

`String`, `Vec`, and `Option` in a schema are bounded account types. They do not use heap allocation. The derive maps them to `PodString`, `PodVec`, and `PodOption` representations.

## Schema aliases choose common prefixes

The root aliases keep ordinary schema declarations short:

```rust
use pinapod::{String, Vec};

type Name = String<32>; // PodString<32, 1>
type Scores = Vec<u64, 16>; // PodVec<u64, 16, 2>
```

The aliases are ordinary Rust aliases. An editor can resolve `String` and `Vec` to the `pinapod` imports. The derive does not rewrite the spelling as a hidden macro convention.

Use `PodString` or `PodVec` when a format needs an explicit prefix:

```rust
use pinapod::{PodString, PodVec};

type LongText = PodString<1024, 2>;
type SmallList = PodVec<u64, 12, 1>;
type LargeList = PodVec<u64, 100_000, 4>;
```

The last const argument is a byte count. It must be `1`, `2`, `4`, or `8`. Do not write a prefix type such as `u16`, and do not attach a prefix attribute to the field.

The capacity must fit in the selected prefix. For example, `String<256>` is invalid because its one-byte prefix can represent at most 255. Use `PodString<256, 2>` for that schema.

Changing a prefix width changes the wire format. Capacity alone does not change existing value bytes, but it changes the fixed representation size.

## `PodVec` maps native elements

`PodVec<u64, 8, 2>` stores `PodU64` elements. Use native element types in a schema and at write boundaries:

```rust
use pinapod::PodVec;

let mut values = PodVec::<u64, 8, 2>::default();
values.try_set([13_u64, 21, 34])?;

assert_eq!(values[0].get(), 13);
# Ok::<(), pinapod::PinaPodError>(())
```

`try_set` and `try_extend` accept arrays, slices, and standard vectors through `AsRef<[T]>`. They check the complete input length before changing the destination, then convert each native value into its stored representation. The stored representation remains visible when you read. Integer pod types use methods such as `get` and `set` because their bytes can be unaligned.

## Removed values become zero bytes

Every pod container has fully initialized backing storage. `Default` zeros both the prefix and inactive capacity. Operations that shorten or clear a string or vector zero the removed range. Setting a `PodOption` to `None` zeros its payload.

These rules prevent a later raw account read or canonical serialization from disclosing a previous value.

## A vector of strings has fixed-size elements

`Vec<String<M>, N>` is a supported compact tail. The outer vector stores only its active elements, but each active element occupies the complete fixed representation of `String<M>`.

For `Vec<String<8>, 4>`, each active element uses nine bytes: one inner length byte and eight payload bytes. The strings can have different logical lengths. The representation still uses nine bytes for `"a"` and nine bytes for `"pinapod"`.

This fixed footprint keeps element indexing constant-time and reuses `PodString` validation. It also means that a vector of short strings may use more account bytes than a recursively packed string table. Version 0.2 chooses the fixed-footprint format so the first compact nesting release has one clear layout.
