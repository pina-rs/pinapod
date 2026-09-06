# pinapod

Zero-copy, alignment-1 pod types for Solana programs.

Pinapod is the Pina-maintained, wire-compatible fork of [ZeroPod](https://github.com/blueshift-gg/zeropod). It preserves the existing account representation while independently reviewing and releasing soundness fixes required by the Pina framework.

pinapod lets you read and write on-chain data through direct pointer casts — no serialization, no copies, no alignment traps. Every type is `#[repr(C)]` with alignment 1, so it maps directly onto Solana account bytes.

The fork intentionally keeps ZeroPod's byte representation stable. Public API names use the `pinapod` crate and `#[pinapod(...)]` helper attribute, while the existing `ZeroPod*` trait and derive names remain recognizable to ease audited upstream synchronization.

## Upstream Compatibility

Pinapod versions are independent from ZeroPod versions. Each Pinapod release records the upstream release and commit it was audited against so downstream users can distinguish wire compatibility from package-version equality.

| Pinapod release | ZeroPod baseline | Upstream commit | Notes                                                                               |
| --------------- | ---------------- | --------------- | ----------------------------------------------------------------------------------- |
| `0.1.x`         | `0.3.5`          | `78e6e5f`       | Same wire format, plus independently reviewed soundness and compact-accessor fixes. |

Later upstream changes are reviewed and ported rather than merged blindly. The compatibility row is updated whenever a Pinapod release adopts a new ZeroPod baseline.

## Install

```toml
[dependencies]
pinapod = "0.1"
```

Pinapod supports Rust 1.89 and newer.

## Pod Types

All pod types are `Copy`, alignment 1, and safe to cast from arbitrary byte slices after validation.

| Type                  | Size                 | Description                                   |
| --------------------- | -------------------- | --------------------------------------------- |
| `PodU16` .. `PodU128` | 2–16                 | Unsigned integers, little-endian `[u8; N]`    |
| `PodI16` .. `PodI128` | 2–16                 | Signed integers, little-endian `[u8; N]`      |
| `PodBool`             | 1                    | Boolean (byte must be 0 or 1)                 |
| `PodOption<T>`        | 1 + size_of(T)       | Optional value (tag byte + `MaybeUninit<T>`)  |
| `PodString<N, PFX>`   | PFX + N              | UTF-8 string, length-prefixed, max N bytes    |
| `PodVec<T, N, PFX>`   | PFX + N * size_of(T) | Typed vector, length-prefixed, max N elements |

Convenience aliases: `pinapod::String<N>` = `PodString<N, 1>`, `pinapod::Vec<T, N>` = `PodVec<T, N, 2>`.

## Derive Macro

`#[derive(ZeroPod)]` generates a zero-copy companion type with validation and pointer-cast access.

### Fixed layout

Every field is a known size. The companion type is a direct `#[repr(C)]` mirror.

```rust
use pinapod::ZeroPod;

#[derive(ZeroPod)]
struct TokenAccount {
    pub mint: [u8; 32],
    pub owner: [u8; 32],
    pub amount: u64,
    pub is_frozen: bool,
}

// Read from raw account bytes — validates, then pointer-casts (zero copy):
let zc = TokenAccount::from_bytes(&account_data)?;
let amount: u64 = zc.amount.get();
```

### Compact layout

For structs with variable-length fields. The on-chain format is `[fixed header + length prefixes][tail data]`. Fixed fields and length prefixes live in the header; dynamic data (strings, vecs) is packed contiguously after it.

```rust
use pinapod::ZeroPod;

#[derive(ZeroPod)]
#[pinapod(compact)]
struct Profile {
    pub authority: [u8; 32],
    pub score: u64,
    pub name: pinapod::String<32>,
    pub tags: pinapod::Vec<u8, 16>,
}

// Read via zero-copy Ref:
let r = ProfileRef::new(&data)?;
let name: &str = r.name();
let tags: &[u8] = r.tags();

// Mutate via Mut + commit:
let mut m = ProfileMut::new(&mut data)?;
m.set_name("alice")?;
m.commit()?;
```

### Fixed-point fields

Enable the opt-in `fixed` feature to use any signed or unsigned [`fixed`](https://docs.rs/fixed/1.30.0/fixed/) type in fixed or compact schemas:

```toml
[dependencies]
fixed = { version = "=1.30.0", default-features = false }
pinapod = { version = "0.1", features = ["fixed"] }
```

Pinapod pins `fixed` 1.30.0 because it supports Rust 1.85; `fixed` 1.31.0 raises its minimum supported Rust version to 1.93, above Pinapod's Rust 1.89 baseline. Fixed-point values retain their raw bits on-chain in little-endian integer pods. Convert at the account boundary with `to_bits` and `from_bits`:

```rust
use fixed::types::{I16F16, U24F8};
use pinapod::{pod::{PodI32, PodU32}, ZeroPod};

#[derive(ZeroPod)]
#[pinapod(compact)]
struct PriceBook {
    pub mark_price: I16F16,
    pub bids: pinapod::Vec<I16F16, 16>,
    pub asks: pinapod::Vec<U24F8, 16>,
}

let mut data = [0u8; 256];
let bids = [
    PodI32::from(I16F16::from_num(10.25).to_bits()),
    PodI32::from(I16F16::from_num(10.5).to_bits()),
];
let asks = [PodU32::from(U24F8::from_num(11.0).to_bits())];

let encoded_size = {
    let mut book = PriceBookMut::new(&mut data)?;
    book.mark_price = I16F16::from_num(10.5).to_bits().into();
    book.set_bids(&bids)?;
    book.set_asks(&asks)?;
    book.commit()?
};

let book = PriceBookRef::new(&data[..encoded_size])?;
let mark_price = I16F16::from_bits(book.mark_price.get());
let first_bid = I16F16::from_bits(book.bids()[0].get());
let first_ask = U24F8::from_bits(book.asks()[0].get());
```

Both vectors are independently sized tails: changing the number of bids moves the asks without reserving either vector's maximum capacity. The fixed-point format affects interpretation, not storage size; each value occupies exactly the width of its backing integer.

### Enums

Unit enums with `#[repr(u8)]` get a zero-copy companion that validates the discriminant.

```rust
#[derive(ZeroPod)]
#[repr(u8)]
enum Status {
    Inactive = 0,
    Active = 1,
    Frozen = 2,
}
```

## Arithmetic

Numeric pods use wrapping semantics in release builds and panic on overflow in debug builds — matching native integer behavior.

```rust
use pinapod::pod::PodU64;

let a = PodU64::from(100u64);
let b = PodU64::from(42u64);
assert_eq!((a + b).get(), 142);
assert_eq!((a - b).get(), 58);

// For security-sensitive code, use checked arithmetic:
assert_eq!(a.checked_sub(b), Some(PodU64::from(58)));
assert_eq!(b.checked_sub(a), None); // would underflow
```

## Validation

Every pod type implements `ZcValidate` — called automatically by `from_bytes()`. Validation rejects:

- `PodBool` with byte > 1
- `PodOption` with tag other than 0 or 1, or invalid inner value
- `PodString` with length > N or invalid UTF-8
- `PodVec` with length > N or invalid elements
- Enum discriminants outside the declared variants

```rust
// Malicious account data with bool byte = 5:
let mut buf = [0u8; 33];
buf[8] = 5; // invalid bool
assert!(TokenAccount::from_bytes(&buf).is_err());
```

## Traits

| Trait            | Purpose                                               |
| ---------------- | ----------------------------------------------------- |
| `ZeroPodSchema`  | Declares fixed vs compact layout                      |
| `ZeroPodFixed`   | Zero-copy access for fixed-size types                 |
| `ZeroPodCompact` | Zero-copy access for variable-length types            |
| `ZcValidate`     | Validates byte representations                        |
| `ZcElem`         | Marker: alignment 1, valid for packed access (unsafe) |
| `ZcField`        | Maps native Rust types to their pod companions        |

## Feature Flags

| Flag                   | What it enables                                    |
| ---------------------- | -------------------------------------------------- |
| `fixed`                | `ZcField` for every `fixed` signed/unsigned width  |
| `solana-address`       | `ZcElem` + `ZcField` for `solana_address::Address` |
| `solana-program-error` | `From<ZeroPodError> for ProgramError`              |
| `wincode`              | `SchemaWrite` / `SchemaRead` for all pod types     |

## Formal Verification

pinapod includes [Kani](https://model-checking.github.io/kani/) model-checking proofs covering:

- Roundtrip correctness for all pod types (encode -> decode preserves value)
- Length prefix encode/decode consistency across all prefix widths
- Bounds clamping (corrupted length prefixes cannot cause out-of-bounds access)
- Arithmetic operator consistency with native integers
- UTF-8 preservation in `PodString`
- `PodOption` tag semantics (invalid tags treated as None)
- Checked arithmetic matches `std` semantics

CI additionally runs the complete test suite under Miri. Wincode containers use validated deserialization and canonical recursive serialization rather than advertising direct-borrow `ZeroCopy`: inactive capacity is zero-filled and nested values cannot expose uninitialized or stale bytes.

## Security

Please report suspected soundness or security defects privately as described in [SECURITY.md](SECURITY.md). Pinapod reviews upstream ZeroPod changes, but does not merge them automatically; wire compatibility and safety invariants are verified before each release.

## License

Apache-2.0
