# Discriminators and versioning

PinaPod schemas describe one value, and a real account usually stores more than a value: Solana programs prefix account data with an 8-byte account discriminator, and long-lived accounts eventually need a version tag. Neither concept belongs in the wire format itself, so PinaPod does not generate one — the discriminator is an ordinary prefix that the program owns, and the schema reads the bytes after it.

## Read a schema after a discriminator

`read_prefix` validates the schema over the first `Type::SIZE` bytes of a larger buffer, which is exactly the operation a prefixed account needs. Check the discriminator yourself, then read the schema from the same bytes:

```rust
use pinapod::PinaPod;

#[derive(PinaPod)]
struct Profile {
	authority: [u8; 32],
	display_name: pinapod::String<32>,
}

const PROFILE_DISCRIMINATOR: &[u8; 8] = b"PROFILE\x00";

enum AccountError {
	WrongAccount,
	InvalidProfile(pinapod::PinaPodError),
}

fn read_profile(data: &[u8]) -> Result<&ProfileZc, AccountError> {
	let (discriminator, rest) = data.split_at_checked(8).ok_or(AccountError::WrongAccount)?;

	if discriminator != PROFILE_DISCRIMINATOR {
		return Err(AccountError::WrongAccount);
	}

	Profile::read_prefix(rest).map_err(AccountError::InvalidProfile)
}
```

The order matters: the discriminator is checked before the schema is parsed, so a mismatching account is rejected without paying for validation, and `read_prefix` never sees bytes it does not own. `split_at_checked` keeps the boundary arithmetic checked instead of slicing on an unchecked index.

## Store the discriminator in the schema instead

A discriminator the program writes but never changes can live in the schema as its first field. Declaring it as `[u8; 8]` keeps it on the wire at offset zero, and the derived validation alone cannot know which eight bytes are yours, so the equality check stays in program code:

```rust
#[derive(PinaPod)]
struct ProfileAccount {
	discriminator: [u8; 8],
	authority: [u8; 32],
	display_name: pinapod::String<32>,
}
```

Set the discriminator inside `ProfileAccount::initialize`'s closure so it is written once at account creation. `PinaPodFixed::initialize` zeroes the destination first and validates the completed value, so a partially written account is never observable.

For frameworks this is what `skip_accessor` and `skip_patch` are for: a field marked `#[pinapod(skip_accessor, skip_patch)]` is still stored and validated, but application code cannot read it through a generated accessor or move it through a compact patch, because the framework owns those bytes. Pina marks its account discriminators exactly this way.

## Version a schema

Version the value, not the wire format: a leading `repr(u8)` enum whose variants name the layout generations, followed by the fields that generation carries. A reader dispatches on the discriminant before choosing a schema:

```rust
use pinapod::PinaPod;

#[derive(PinaPod)]
#[repr(u8)]
enum JournalVersion {
	V1 = 1,
	V2 = 2,
}

#[derive(PinaPod)]
struct JournalV1 {
	revision: u64,
	entries: pinapod::Vec<u64, 64>,
}

#[derive(PinaPod)]
struct JournalV2 {
	revision: u64,
	entries: pinapod::Vec<u64, 256>,
	note: Option<pinapod::String<128>>,
}
```

`JournalVersion::read_prefix` consumes and validates the first byte, and `read_prefix` again on the remaining bytes parses whichever body the version names. Because every generation keeps its own `SIZE` and validation, an unknown future version is an `InvalidDiscriminant` error rather than a misparsed body — old programs reject newer accounts cleanly instead of reading them wrongly.

Migrations stay ordinary program code: read the old generation, write the new one through its own initializer, and let the account's rent-exempt allocation grow with the new size. For compact schemas the patch API already models "plan the new size, then apply", which is the same lifecycle a version migration needs.
