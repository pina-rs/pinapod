# Compact accounts

Use a compact account when its allocation must follow active string and vector data. The schema still sets hard capacities. The account stores a fixed header and only the active tail bytes.

## Declare the schema

Add `#[pinapod(compact)]`. Put fixed fields before dynamic tails.

```rust
use pinapod::{PinaPod, String, Vec};

#[derive(PinaPod)]
#[pinapod(compact)]
struct Journal {
    authority: [u8; 32],
    revision: u64,
    entries: Vec<u64, 1024>,
    note: Option<String<128>>,
    labels: Vec<String<16>, 32>,
}
```

`Journal` has three independent dynamic tails. Changing `entries` moves `note` and `labels` as needed. It does not reserve 1,024 entries, 128 note bytes, or 32 labels in every account.

The derive accepts these compact field forms:

- `String<N>`
- `Vec<T, N>` when `T` has a fixed representation
- `Option<T>` when `T` has a fixed representation
- `Option<String<N>>`
- `Option<Vec<T, N>>` when `T` has a fixed representation
- `Vec<String<M>, N>`

`PodString<N, PFX>` and `PodVec<T, N, PFX>` are the explicit-prefix versions of the string and vector forms. `PFX` must be the const value `1`, `2`, `4`, or `8`.

Other dynamic nesting is not part of the v0.2 compact format. For example, `Option<Vec<String<M>, N>>`, `Vec<Vec<T, M>, N>`, and a compact schema used as a vector element are rejected. The derive error lists the accepted forms and points at the unsupported field.

## Why dynamic nesting stops here

A compact tail has no reserved slot for its maximum payload. With a `Vec<Vec<T, M>, N>`, the address of element five depends on the active lengths of elements zero through four. Direct indexing then requires either a scan or an additional offset table in the wire format.

Updates have the same problem in reverse. Changing one nested value can move every later nested value and every later account tail. The implementation must validate all old and new ranges before the first move. Rust, TypeScript, and Dart also need to agree on the exact offset-table or scan rules.

Version 0.2 supports the combinations that have one bounded calculation per tail. `Vec<String<M>, N>` fits because each active string occupies one fixed `PodString<M>` slot. A later release can add recursive packing with a separate wire-format decision and cross-language fixtures.

## Understand `Vec<String<M>, N>`

The outer vector packs only active elements into the tail. Each active string is a fixed `PodString<M>` representation, so it occupies `1 + M` bytes with the default prefix. Individual strings can have different logical lengths.

This is a fixed-footprint element format, not a recursively packed string table. It provides constant-time element indexing and uses the same string validator as a fixed account. See [Pod containers and prefixes](./containers.md) for the byte cost.

## Read active and allocated lengths

`read_prefix` validates the compact value and returns the generated borrowed reader. The reader distinguishes encoded data from the physical slice:

```rust
let journal = Journal::read_prefix(account_data)?;

assert!(journal.encoded_len() <= journal.storage_len());
assert_eq!(
	journal.spare_capacity(),
	journal.storage_len() - journal.encoded_len(),
);

let revision = journal.revision.get();
let entries = journal.entries();
let note = journal.note();
# Ok::<(), pinapod::PinaPodError>(())
```

`encoded_len` is the header plus active tails. `storage_len` is the complete supplied allocation. `spare_capacity` is the difference.

`PinaPodCompact` enforces the physical allocation contract. The storage length must be between `Journal::MIN_SIZE` and `Journal::MAX_SIZE`, inclusive, and growth beyond `MIN_SIZE` must be a multiple of `Journal::TAIL_ALIGNMENT`. `read_prefix` checks this contract before it validates the active value at the start of the allocation. A framework can add account-specific rules, but it cannot bypass the schema bounds or granularity.

## Describe one atomic update

The derive generates `JournalPatch`. A new patch keeps every field unchanged until a builder method sets it.

```rust
let patch = JournalPatch::new()
	.revision(next_revision)
	.replace_entries(&entries)
	.note(Some("Updated"));
```

Fixed fields use their field name. A vector tail uses `replace_` because the slice replaces the complete active vector. An optional string accepts `Option<&str>`. `None` sets the field to absent, while omitting `.note(...)` keeps the current value.

A vector replacement borrows the mapped element representation. For `Vec<u64, N>`, `entries` is a slice of `PodU64`. Convert native values with `PodU64::from` or `.into()` before building the patch. This contract avoids a hidden allocation in a `no_std` program.

The patch borrows strings and element slices. It does not expose header prefixes, tail offsets, raw pointers, or staged edit state.

## Calculate a resize before changing bytes

Call `updated_len` while the old account data is borrowed. The method validates the current value and every patch input. It returns the required encoded length without changing `account_data`.

```rust
let required_len = Journal::updated_len(account_data, &patch)?;
```

Release the account-data borrow before reallocating. Grow the account when `required_len` exceeds the current allocation. After the resize, borrow the data again and apply the same patch:

```rust
let encoded_len = Journal::update(resized_account_data, &patch)?;
assert_eq!(encoded_len, required_len);
```

`update` repeats the preflight against the current slice, then applies the complete patch. If preflight fails, the destination is unchanged. If the new value is shorter, `update` zeros the old encoded suffix. After the mutable borrow ends, the account framework can shrink the allocation to `encoded_len`.

This two-phase API exists because Solana cannot reallocate account data while a borrow of that data remains active.

## Initialize new storage

Use the generated `initialize` method for a new compact allocation:

```rust
let labels = [
	String::<16>::try_from("todo")?,
	String::<16>::try_from("complete")?,
];

let patch = JournalPatch::new()
	.authority(authority)
	.revision(0)
	.replace_entries(&[])
	.note(None)
	.replace_labels(&labels);

let encoded_len = Journal::initialize(account_data, &patch)?;
```

`initialize` zeros the supplied storage before applying the patch. It validates the final value once. If an input or final validation fails, the complete supplied slice remains zeroed.

Set every field whose all-zero representation is not valid. A nonzero enum discriminant is the common example.

## Let Pina manage the resize lifecycle

Pina wraps `updated_len`, reallocation, and `update` in one builder:

```rust
UpdateResizableAccount {
	account: self.journal,
	rent_account: self.authority,
	program_id: &ID,
	patch: JournalPatch::new()
		.revision(next_revision)
		.replace_entries(&entries)
		.note(Some("Updated")),
}
.invoke::<Journal>()?;
```

Use `rent_account`, matching Pina's other reallocation builders. The builder drops each account-data guard before it reallocates and parses the resized bytes again before applying the patch.
