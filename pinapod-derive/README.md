# pinapod-derive

`pinapod-derive` implements `#[derive(PinaPod)]`. Depend on `pinapod`, which re-exports the derive and its runtime traits.

```toml
[dependencies]
pinapod = "0.2"
```

```rust
use pinapod::PinaPod;

#[derive(PinaPod)]
struct Balance {
    owner: [u8; 32],
    amount: u64,
    frozen: bool,
}
```

## Fixed output

For a fixed struct named `Balance`, the derive generates:

- `BalanceZc`, an alignment-one representation with mapped pod fields.
- A `PinaPod` marker implementation.
- A `PinaPodFixed` implementation with `BalanceZc` as its complete representation.
- An inherent `Balance::SIZE` calculated with `size_of::<BalanceZc>()`.
- Inherent `read_exact`, `read_prefix`, validation, and initialization methods.
- Recursive `ZcValidate`, `ZcElem`, and `ZcField` implementations.

Direct dependencies can rename the `pinapod` package in `Cargo.toml`; the derive resolves that dependency automatically. A framework that re-exports PinaPod must provide its public crate path:

```rust
#[derive(pina::PinaPod)]
#[pinapod(crate = pina::pinapod, no_inherent)]
struct FrameworkAccount {
    value: u64,
}
```

`no_inherent` suppresses schema-level fixed helper methods so the framework can provide account-aware methods with the same names. It does not remove the trait implementations or the generated representation. Direct derives should keep the default inherent helpers.

Framework-owned fields can opt out of generated conveniences:

```rust
#[pinapod(skip_accessor, skip_patch)]
discriminator: [u8; 8],
```

`skip_accessor` omits the field's convenience accessor. `skip_patch` keeps the field in compact storage and validation but omits it from the generated patch. Pina uses both options for account discriminators that only the framework may write.

For a unit enum with `#[repr(u8)]`, `#[repr(u16)]`, `#[repr(u32)]`, or `#[repr(u64)]`, the derive generates a transparent companion. The companion checks compiler-evaluated discriminants and provides conversions to and from the native enum.

## Compact output

Add `#[pinapod(compact)]` to select the compact layout.

```rust
use pinapod::{PinaPod, String, Vec};

#[derive(PinaPod)]
#[pinapod(compact)]
struct Journal {
    revision: u64,
    entries: Vec<u64, 1024>,
    note: Option<String<128>>,
}
```

The derive generates an alignment-one header, a validated borrowed reader, and an atomic patch API. Dynamic length metadata and internal edit state remain private. The generated validator checks the allocation against its minimum, maximum, and tail granularity, then checks every offset and length before it forms a tail reference.

Compact fields support these forms:

- `String<N>`
- `Vec<T, N>` when `T` has a fixed representation
- `Option<T>` when `T` has a fixed representation
- `Option<String<N>>`
- `Option<Vec<T, N>>` when `T` has a fixed representation
- `Vec<String<M>, N>`

A compact struct can contain more than one dynamic tail. Unsupported dynamic nesting produces a compile error that lists the accepted forms.

## Prefix widths

`String<N>` uses a one-byte prefix. `Vec<T, N>` uses a two-byte prefix. Use `PodString<N, PFX>` or `PodVec<T, N, PFX>` for an explicit width, where `PFX` is `1`, `2`, `4`, or `8`.

```rust
use pinapod::{PinaPod, PodVec};

#[derive(PinaPod)]
struct History {
    values: PodVec<u64, 1024, 2>,
}
```

The derive rejects `#[pinapod(prefix = u16)]`. Keeping the width in the type makes it part of the schema and preserves it through nested fields.

## License

Apache-2.0
