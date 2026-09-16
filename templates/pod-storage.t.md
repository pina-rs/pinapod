<!-- {@podAlignmentAndValidationContract} -->

All representations have alignment one, so a stored field can be read at any byte offset without a copy or a relocation.

Safe readers validate tags, lengths, UTF-8, enum discriminants, nested values, and slice bounds before they return a reference.

<!-- {/podAlignmentAndValidationContract} -->

<!-- {@podTypesTable} -->

| Type                       |                     Stored size | Meaning                                     |
| -------------------------- | ------------------------------: | ------------------------------------------- |
| `PodU16` through `PodU128` |              2 through 16 bytes | Unsigned, little-endian integer             |
| `PodI16` through `PodI128` |              2 through 16 bytes | Signed, little-endian integer               |
| `PodBool`                  |                          1 byte | Boolean with a `0` or `1` byte              |
| `PodF32`                   |                         4 bytes | IEEE-754 binary32 stored as its bit pattern |
| `PodF64`                   |                         8 bytes | IEEE-754 binary64 stored as its bit pattern |
| `PodOption<T, PFX>`        |          `PFX + size_of::<T>()` | Optional fixed representation               |
| `PodString<N, PFX>`        |                       `PFX + N` | UTF-8 string with at most `N` bytes         |
| `PodVec<T, N, PFX>`        | `PFX + N * mapped element size` | Vector with at most `N` mapped pod elements |

<!-- {/podTypesTable} -->

<!-- {@podSchemaAliases} -->

The schema aliases choose common prefix widths, so ordinary declarations stay short:

- `String<N>` is `PodString<N, 1>`.
- `Vec<T, N>` is `PodVec<T, N, 2>`.

<!-- {/podSchemaAliases} -->

<!-- {@podContainerFootprintContract} -->

A container reserves its full capacity wherever it appears, so a smaller value never shrinks the representation.

`String<32>` occupies its one-byte prefix plus all 32 payload bytes, and `Vec<u64, 8>` occupies its two-byte prefix plus space for all eight elements.

<!-- {/podContainerFootprintContract} -->

<!-- {@podCapacityOverflowAdvice} -->

Choose the capacity from the largest value the schema must hold, because a write that does not fit is rejected rather than truncated.

<!-- {/podCapacityOverflowAdvice} -->
