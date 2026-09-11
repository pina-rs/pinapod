# PinaPod

PinaPod defines alignment-one, zero-copy representations for Solana account and instruction data. The derive generates the representation, validates bytes before forming references, and keeps the v0.1 wire format.

PinaPod is the Pina-maintained fork of [ZeroPod](https://github.com/blueshift-gg/zeropod). The crates have independent versions. PinaPod reviews upstream changes before porting them.

## Install

```toml
[dependencies]
pinapod = "0.2"
```

PinaPod supports Rust 1.89 and newer. The MSRV follows the Rust versions supported by Solana's Agave releases; see the [supported toolchains page](https://pina-rs.github.io/pinapod/support.html) for the current alignment and policy. The runtime crate is `no_std`.

## Pick a layout

Use a fixed layout when the account allocation never changes. Bounded strings, vectors, and options work in fixed layouts. Their full capacities occupy account bytes.

```rust
use pinapod::{PinaPod, String, Vec};

#[derive(PinaPod)]
struct Profile {
	authority: [u8; 32],
	display_name: String<32>,
	roles: Vec<u16, 8>,
	note: Option<String<64>>,
}

let mut data = vec![0_u8; Profile::SIZE];
Profile::initialize(&mut data, |profile| {
	profile.display_name.try_set("ifi")?;
	profile.roles.try_set([7_u16, 11])?;
	Ok(())
})?;

let profile = Profile::read_exact(&data)?;
assert_eq!(profile.display_name.as_str(), "ifi");
# Ok::<(), pinapod::PinaPodError>(())
```

Use a compact layout when the allocation must track active data. Compact schemas store fixed fields in a header and pack active string and vector bytes after that header.

```rust
use pinapod::{PinaPod, String, Vec};

#[derive(PinaPod)]
#[pinapod(compact)]
struct Journal {
    authority: [u8; 32],
    revision: u64,
    entries: Vec<u64, 1024>,
    note: Option<String<128>>,
}
```

Compact schemas support multiple tails. See the [compact account guide](https://pina-rs.github.io/pinapod/compact-accounts.html) for the supported grammar and update API.

## Set a prefix width in the type

The schema aliases choose common prefix widths:

- `String<N>` is `PodString<N, 1>`.
- `Vec<T, N>` is `PodVec<T, N, 2>`.

Use the pod types when the wire format needs another width. `PFX` is the number of prefix bytes and must be `1`, `2`, `4`, or `8`.

```rust
use pinapod::{PinaPod, PodString, PodVec};

#[derive(PinaPod)]
struct Archive {
    label: PodString<300, 2>,
    values: PodVec<u64, 1024, 2>,
}
```

Do not use `#[pinapod(prefix = u16)]`. Prefix width belongs in the field type, so the declaration shows the exact wire representation.

## Pod types

| Type                       |                     Stored size | Meaning                                     |
| -------------------------- | ------------------------------: | ------------------------------------------- |
| `PodU16` through `PodU128` |              2 through 16 bytes | Unsigned, little-endian integer             |
| `PodI16` through `PodI128` |              2 through 16 bytes | Signed, little-endian integer               |
| `PodBool`                  |                          1 byte | Boolean with a `0` or `1` byte              |
| `PodOption<T, PFX>`        |          `PFX + size_of::<T>()` | Optional fixed representation               |
| `PodString<N, PFX>`        |                       `PFX + N` | UTF-8 string with at most `N` bytes         |
| `PodVec<T, N, PFX>`        | `PFX + N * mapped element size` | Vector with at most `N` mapped pod elements |

All representations have alignment one. Safe readers validate tags, lengths, UTF-8, enum discriminants, nested values, and slice bounds before they return a reference.

## Features

| Feature                | Adds                                                     |
| ---------------------- | -------------------------------------------------------- |
| `fixed`                | Mappings for signed and unsigned `fixed` 1.30.0 values   |
| `solana-address`       | A mapping for `solana_address::Address`                  |
| `solana-program-error` | Conversion from `PinaPodError` to `ProgramError`         |
| `wincode`              | Canonical `SchemaRead` and `SchemaWrite` implementations |

## Documentation and verification

The [PinaPod book](https://pina-rs.github.io/pinapod/) contains the API guide, the migration guides ([v0.1 to v0.2](https://pina-rs.github.io/pinapod/migration-v0.2.html) and [v0.2 to v0.3](https://pina-rs.github.io/pinapod/migration-v0.3.html)), the safety model, and benchmark instructions. The repository runs native tests on the pinned nightly, current stable, and the MSRV; Miri regressions; Kani proofs including derive-generated compact schemas; coverage-guided fuzzing over every untrusted-input reader (see [fuzz/README.md](fuzz/README.md)); wire-format fixtures; and a Criterion comparison against both PinaPod v0.1 and upstream ZeroPod. Versions and changelogs are managed by MonoChange, which also provides the semantic-version compatibility check for each release.

Report a suspected soundness or security defect through GitHub private vulnerability reporting. See [SECURITY.md](SECURITY.md) for the required report details.

## License

Apache-2.0
