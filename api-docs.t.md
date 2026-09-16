<!-- {@podMdtManagedDocNote} -->

This section is synchronized by `mdt` and expands from `api-docs.t.md`. Edit the provider, then run `devenv shell docs:sync`.

<!-- {/podMdtManagedDocNote} -->

<!-- {@podRawDecodeLenContract} -->

The raw decoded length prefix.

This is the unvalidated prefix value. On a prefix wider than `usize` (eight-byte prefixes on 32-bit targets) the sentinel `usize::MAX` is returned. Safe accessors such as [`len`](Self::len) clamp the value to the capacity; readers reject it during validation.

<!-- {/podRawDecodeLenContract} -->

<!-- {@podClampedLenContract} -->

The active length, clamped to the fixed capacity `N`.

A forged or corrupt prefix can decode above `N`; this accessor never trusts it. Callers that need to distinguish a corrupt prefix from a valid one must validate through a reader first (see [`ZcValidate`](crate::ZcValidate)).

<!-- {/podClampedLenContract} -->

<!-- {@podWriteCapacityContract} -->

Returns [`PinaPodError::Overflow`](crate::PinaPodError::Overflow) when the write would exceed the fixed capacity.

The destination keeps its previous contents, so a rejected write is a no-op.

<!-- {/podWriteCapacityContract} -->

<!-- {@podPrefixWidthContract} -->

`PFX` is the length-prefix width in bytes and must be `1`, `2`, `4`, or `8`.

The capacity must fit that prefix: `String<255>` is valid, `String<256>` is not, and `PodString<256, 2>` restores it.

<!-- {/podPrefixWidthContract} -->

<!-- {@podZeroedInactiveCapacityContract} -->

Every container starts with fully initialized backing storage.

Operations that shorten or clear active data zero the bytes they vacate, so a later raw read or canonical serialization cannot disclose a previous value.

<!-- {/podZeroedInactiveCapacityContract} -->

<!-- {@podDeriveSupportTraitContract} -->

This trait is public only because generated code expands in downstream crates.

It is not part of the hand-written PinaPod API. Its shape follows the generated output and changes only in breaking releases, in lockstep with the derive.

<!-- {/podDeriveSupportTraitContract} -->

<!-- {@podFloatBitPatternContract} -->

Storage is the complete IEEE-754 bit pattern, little-endian.

Every bit pattern is a valid stored value, so validation never rejects a NaN, an infinity, or the sign of zero, and an all-zero field decodes as `+0.0`.

<!-- {/podFloatBitPatternContract} -->

<!-- {@podFloatBitwiseEqualityContract} -->

Equality compares stored bit patterns rather than decoded floats.

That keeps `Eq` sound in the presence of NaN payloads and preserves the distinction between `+0.0` and `-0.0`. The pods deliberately implement no `PartialOrd` or `Ord`, because bitwise equality and float ordering cannot both hold: an ordering would have to rank NaN payloads and separate `+0.0` from `-0.0`. Decode with `get` and compare the natives when an ordering is needed.

<!-- {/podFloatBitwiseEqualityContract} -->

<!-- {@podErrorContract} -->

| Variant               | Meaning                                                                      |
| --------------------- | ---------------------------------------------------------------------------- |
| `BufferTooSmall`      | The supplied slice cannot contain the required header, value, or active tail |
| `Overflow`            | A requested write exceeds a field capacity or checked arithmetic fails       |
| `InvalidBool`         | A stored boolean byte is not zero or one                                     |
| `InvalidTag`          | A stored option tag is not zero or one                                       |
| `InvalidDiscriminant` | A stored enum value has no declared variant                                  |
| `InvalidLength`       | A stored length exceeds capacity or violates the read contract               |
| `InvalidUtf8`         | Active string bytes are not UTF-8                                            |

<!-- {/podErrorContract} -->

<!-- {@podErrorProgramErrorMapping} -->

The `solana-program-error` feature maps a small buffer to `ProgramError::AccountDataTooSmall`.

It maps every invalid representation and every write overflow to `ProgramError::InvalidAccountData`.

<!-- {/podErrorProgramErrorMapping} -->
