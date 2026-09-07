# Choose an account layout

Fixed and compact accounts use the same field representations. They differ in where bounded capacity lives.

## Fixed accounts reserve capacity in the account

A fixed account always occupies `Schema::SIZE` bytes. `String<32>` reserves its one-byte prefix and all 32 payload bytes. `Vec<u64, 8>` reserves its two-byte prefix and space for all eight elements. `Option<T>` reserves its tag and the complete representation of `T`, even when the value is absent.

This layout is the better default when the maximum allocation is small or the program does not need reallocations. A field offset never moves, so each access is direct after validation.

Fixed accounts support recursively bounded combinations. For example, `Vec<String<16>, 4>`, `Option<Vec<u64, 8>>`, and `Vec<Option<String<8>>, 4>` all have one compile-time size.

Read [Fixed accounts](./fixed-accounts.md) for construction and read semantics.

## Compact accounts reserve capacity in the schema

A compact account has a fixed header followed by active tail bytes. The schema still declares a maximum for each string and vector, but the allocation does not reserve every maximum. A five-byte `String<64>` tail occupies five payload bytes. Its length metadata lives in the header.

Compact fields save rent when active data is much smaller than its bound. The cost is a resize lifecycle and moving later tails when an earlier tail changes size. Accessors may also need to calculate the positions of preceding tails.

Read [Compact accounts](./compact-accounts.md) for the accepted field grammar and the update lifecycle.

## The layouts keep the same field bytes

The container encodings do not change between layouts:

- Integers use little-endian bytes.
- Booleans use one byte with values `0` and `1`.
- Strings store a length prefix and UTF-8 bytes.
- Vectors store an element count and fixed-size element bytes.
- Options store a zero or one tag and an optional payload.

The compact layout moves string and vector payloads out of their header fields. It does not introduce another value encoding.
