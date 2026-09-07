# Fixed accounts

Use a fixed account when every field has a compile-time bound and the account does not need to follow its active payload size.

## Declare bounded fields

Do not add `#[pinapod(compact)]`.

```rust
use pinapod::{PinaPod, String, Vec};

#[derive(PinaPod)]
struct Profile {
    authority: [u8; 32],
    status: Status,
    display_name: String<32>,
    roles: Vec<u16, 8>,
    note: Option<String<64>>,
}

#[derive(PinaPod, Debug, PartialEq)]
#[repr(u8)]
enum Status {
    Active = 1,
    Suspended = 2,
}
```

The derive creates `ProfileZc` and exposes `Profile::SIZE`. The generated size includes all bounded capacity:

```text
32                         authority
+ 1                        status
+ (1 + 32)                 display_name
+ (2 + 8 * 2)              roles
+ (1 + 1 + 64)             note
```

`Option<String<64>>` contains both an option tag and the string's length prefix.

## Initialize once, then validate

Use `initialize` for new bytes. The method zeros the complete destination before it calls your closure. It validates the finished representation once.

```rust
use pinapod::{PinaPodError, String};

let mut data = vec![0xff_u8; Profile::SIZE];
Profile::initialize(&mut data, |profile| {
	profile.authority = [7; 32];
	profile.status = Status::Active.into();
	profile.display_name.try_set("ifi")?;
	profile.roles.try_set([3_u16, 5, 8])?;

	let note = String::<64>::try_from("primary profile")?;
	profile.note.set(Some(note));
	Ok::<(), PinaPodError>(())
})?;
# Ok::<(), PinaPodError>(())
```

If the closure fails or the finished value is invalid, `initialize` zeros the destination again and returns the error. This behavior matters for enums such as `Status`, where zero is not a valid discriminant. A reader cannot configure such a field because the reader validates before returning.

## Choose exact or prefix reads

Use `read_exact` when the slice must contain one fixed value and nothing else.

```rust
let profile = Profile::read_exact(&data)?;
assert_eq!(profile.display_name.as_str(), "ifi");
assert_eq!(profile.roles[1].get(), 5);
# Ok::<(), pinapod::PinaPodError>(())
```

`read_exact` rejects a slice with trailing bytes. This is the right contract for instruction data and an account allocation whose size is part of the schema.

Use `read_prefix` when one fixed value starts a larger containing format:

```rust
let mut envelope = vec![0_u8; Profile::SIZE + 16];
Profile::initialize(&mut envelope[..Profile::SIZE], |profile| {
	profile.status = Status::Active.into();
	Ok(())
})?;
let profile = Profile::read_prefix_mut(&mut envelope)?;
profile.display_name.try_set("updated")?;
# Ok::<(), pinapod::PinaPodError>(())
```

The prefix methods require at least `Profile::SIZE` bytes and ignore the remaining bytes. Pick one contract at the boundary. Do not use a prefix read to hide an unexpected account-size mismatch.

## Nest bounded containers

Fixed accounts can nest containers when every level has a fixed representation.

```rust
use pinapod::{PinaPod, String, Vec};

#[derive(PinaPod)]
struct Directory {
    names: Vec<String<16>, 8>,
    aliases: Vec<Option<String<8>>, 4>,
    preferred_ids: Option<Vec<u64, 16>>,
}
```

Each active nested value has its own logical length. The account still reserves each container's maximum representation. Validation follows every active value and ignores inactive option payloads.

Use a [compact account](./compact-accounts.md) if these maximum representations would waste material rent.
