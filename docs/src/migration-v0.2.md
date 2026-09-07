# Migrate from v0.1 to v0.2

Version 0.2 changes the Rust API but preserves v0.1 account and instruction bytes. Migrate source code and generated clients together. You do not need an on-chain data migration unless you also change a field type, capacity, prefix width, order, or enum discriminant.

## Update the dependency

```toml
[dependencies]
pinapod = "0.2"
```

Keep any existing feature flags. The PinaPod runtime remains `no_std`.

## Rename the derive and public contracts

Replace the old public names as follows:

| v0.1 name        | v0.2 name        |
| ---------------- | ---------------- |
| `ZeroPod`        | `PinaPod`        |
| `ZeroPodSchema`  | `PinaPod`        |
| `ZeroPodFixed`   | `PinaPodFixed`   |
| `ZeroPodCompact` | `PinaPodCompact` |
| `ZeroPodError`   | `PinaPodError`   |

Before:

```rust
use pinapod::{ZeroPod, ZeroPodFixed};

#[derive(ZeroPod)]
struct Counter {
	value: u64,
}

let counter = Counter::from_bytes(data)?;
# Ok::<(), pinapod::ZeroPodError>(())
```

After:

```rust
use pinapod::PinaPod;

#[derive(PinaPod)]
struct Counter {
	value: u64,
}

let counter = Counter::read_exact(data)?;
# Ok::<(), pinapod::PinaPodError>(())
```

The derive adds the common `PinaPod` contract and either `PinaPodFixed` or `PinaPodCompact`. Most applications only need to import the derive.

## Configure framework re-exports

The derive automatically resolves a renamed direct `pinapod` dependency. No attribute is required for this Cargo dependency:

```toml
[dependencies]
account-pod = { package = "pinapod", version = "0.2" }
```

A framework that re-exports PinaPod must tell the derive where its runtime re-export lives. It can also suppress inherent fixed helpers when the framework provides account-aware methods with the same names:

```rust
#[derive(pina::PinaPod)]
#[pinapod(crate = pina::pinapod, no_inherent)]
struct FrameworkAccount {
    value: u64,
}
```

`no_inherent` removes only the schema's inherent fixed helper methods. The `PinaPod` and `PinaPodFixed` implementations remain available. Direct derives should keep the default helpers.

## Choose fixed read semantics

The v0.1 fixed reader accepted a valid value at the start of a longer slice. Version 0.2 separates that behavior from exact reads.

Use these replacements:

| v0.1 call              | v0.2 call when trailing bytes are invalid | v0.2 call for a containing format |
| ---------------------- | ----------------------------------------- | --------------------------------- |
| `from_bytes(data)`     | `read_exact(data)`                        | `read_prefix(data)`               |
| `from_bytes_mut(data)` | `read_exact_mut(data)`                    | `read_prefix_mut(data)`           |
| `validate(data)`       | `validate_exact(data)`                    | `validate_prefix(data)`           |

Prefer exact reads for account types whose allocation size is part of their contract and for instruction data. Use a prefix read only when another format owns the remaining bytes.

## Use fixed initialization for new storage

Do not read a zeroed destination before configuring it. A schema can contain an enum whose valid discriminants do not include zero.

Before:

```rust
let value = Counter::from_bytes_mut(&mut data)?;
value.value = 1_u64.into();
```

After:

```rust
Counter::initialize(&mut data, |value| {
	value.value = 1_u64.into();
	Ok(())
})?;
# Ok::<(), pinapod::PinaPodError>(())
```

`initialize` requires an exact-size slice. It zeros the destination, runs the closure, and validates the finished representation. If the closure or validation fails, the method zeros the destination again.

## Add bounded containers to fixed accounts

PinaPod v0.2 supports `String<N>`, `Vec<T, N>`, and `Option<T>` in fixed accounts. It also supports recursively bounded combinations.

```rust
use pinapod::{PinaPod, String, Vec};

#[derive(PinaPod)]
struct Profile {
    display_name: String<32>,
    tags: Vec<u16, 16>,
    bio: Option<String<128>>,
    previous_names: Vec<String<32>, 4>,
}
```

These fields reserve their complete capacities in `Profile::SIZE`. Use a compact account if the active values must determine rent.

## Replace numeric operators

Version 0.2 removes the following operator implementations from integer pod types:

| Removed surface         | Includes                                                                                                                        |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Arithmetic              | `Add`, `Sub`, `Mul`, `Div`, and `Rem` between pods, between a pod and a native integer, and with the native integer on the left |
| Arithmetic assignment   | `AddAssign`, `SubAssign`, `MulAssign`, `DivAssign`, and `RemAssign`, with pod or native right-hand values                       |
| Bitwise                 | `BitAnd`, `BitOr`, `BitXor`, and `Not`                                                                                          |
| Bitwise assignment      | `BitAndAssign`, `BitOrAssign`, and `BitXorAssign`                                                                               |
| Shifts                  | `Shl` and `Shr`                                                                                                                 |
| Shift assignment        | `ShlAssign` and `ShrAssign`                                                                                                     |
| Signed negation         | `Neg`                                                                                                                           |
| Native-left comparisons | `native == pod`, `native < pod`, and the other reverse `PartialEq` and `PartialOrd` forms                                       |

The explicit numeric API makes overflow behavior visible at every call site.

Before:

```rust
let next = current + 1_u64;
```

After, when overflow is an error:

```rust
let next = current
	.checked_add(1_u64)
	.ok_or(pinapod::PinaPodError::Overflow)?;
```

The retained methods are:

- `checked_add`, `checked_sub`, `checked_mul`, and `checked_div`
- `wrapping_add`, `wrapping_sub`, and `wrapping_mul`
- `saturating_add`, `saturating_sub`, and `saturating_mul`
- `checked_neg` and `wrapping_neg` on signed pods

There is no pod method for remainder, bitwise operations, or shifts. Decode with `get`, apply the native operation after checking its preconditions, then convert the result back:

```rust
let mask = PodU64::from(flags.get() & 0xff);
let remainder = amount
	.get()
	.checked_rem(divisor.get())
	.map(PodU64::from)
	.ok_or(pinapod::PinaPodError::Overflow)?;
```

Replace a compound assignment by calculating the result and assigning it, or by calling `set` with a native result. Pod-left comparisons remain available. Reverse native-left comparisons by swapping the operands and relation: replace `native == pod` with `pod == native`, and replace `native < pod` with `pod > native`.

## Replace iterator-based vector writes

`PodVec::try_set` and `PodVec::try_extend` now accept a slice-like input through `AsRef<[T]>`, including arrays, slices, and standard vectors. They no longer accept an arbitrary `ExactSizeIterator`. Knowing the complete input length before the first write makes overflow failures atomic and avoids trusting a user-defined iterator's reported length.

Collect a transformed iterator at the application edge, or fill an array, then pass that collection to the pod vector:

```rust
let native = vec![3_u64, 5, 8];
profile.roles.try_set(&native)?;
# Ok::<(), pinapod::PinaPodError>(())
```

## Put prefix width in the field type

Remove any proposed or local `#[pinapod(prefix = ...)]` syntax. Use the final const generic on the pod type:

```rust
use pinapod::{PodString, PodVec};

type Memo = PodString<1024, 2>;
type Entries = PodVec<u64, 1024, 2>;
```

Write the prefix width as `1`, `2`, `4`, or `8`, not as `u8`, `u16`, `u32`, or `u64`. Changing the width changes the wire format.

## Replace direct compact mutation

Version 0.1 generated a mutable view, exposed inline header fields through mutable dereferencing, staged tail pointers with `set_*`, and changed bytes in `commit`. Version 0.2 uses a typed patch. The patch keeps dynamic metadata private and validates every requested value before writing.

For a resize, use the same patch for both phases:

1. Read the current value and calculate the updated encoded length.
2. Release the account-data borrow.
3. Grow the account if the calculated length exceeds the allocation.
4. Borrow account data again and apply the patch.
5. Release the borrow, then shrink the allocation when needed.

The [compact account guide](./compact-accounts.md) shows the generated patch API and a complete grow-update-shrink flow.

## Update manual unsafe implementations

`PinaPodFixed` and `PinaPodCompact` are unsafe traits in v0.2. A manual implementation must uphold the layout and validation contract documented on the trait.

Remove a manual `PinaPodFixed::SIZE`. The trait no longer has a size constant. A direct derive exposes inherent `Type::SIZE`, calculated from `size_of::<TypeZc>()`. Generic code can calculate `size_of::<T::Zc>()` when `T: PinaPodFixed`.

Remove `ZcField::POD_SIZE` for the same reason. These changes prevent separate size metadata from disagreeing with the type that an unsafe operation reads.

Most projects should replace manual implementations with `#[derive(PinaPod)]`.

## Replace removed unsafe shortcuts

Do not replace the removed safe `PodOption::value_unchecked` method with an unchecked reference. Use `get_ref`, which returns `None` without exposing the inactive payload. Use `unsafe { assume_init_ref() }` only in code that can prove the tag is `Some` and that the payload has already passed validation.

Compact length fields and edit descriptors are no longer public. Use generated accessors and patches. Do not reconstruct a compact reader or writer from its fields.

## Verify wire compatibility

Run the repository fixtures before deploying a migration:

```sh
devenv shell test:all
devenv shell test:miri
devenv shell bench:compare
```

For Pina, regenerate every Rust, TypeScript, and Dart client after changing the derive name. Compare the generated bytes with the committed cross-language fixtures before updating a program.
