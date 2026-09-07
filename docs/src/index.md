# PinaPod

PinaPod maps validated Solana account and instruction bytes to alignment-one Rust representations. Fixed accounts reserve every bounded field at its maximum size. Compact accounts store only active tail data and can change allocation size.

Version 0.2 is a breaking API release with a stable wire format. It renames the derive and traits to `PinaPod`, adds bounded containers to fixed accounts, and replaces direct compact-header mutation with checked updates.

Start with [Choose an account layout](./account-layouts.md). If you already use PinaPod v0.1, follow [Migrate from v0.1 to v0.2](./migration-v0.2.md).

## What v0.2 guarantees

Safe APIs provide these guarantees:

- A reader validates the complete representation before it returns a typed reference.
- Fixed exact reads reject both short buffers and trailing bytes.
- Fixed prefix reads validate one value at the start of a larger allocation.
- Compact validation uses checked arithmetic for every length, offset, and byte count.
- Compact updates validate the complete change before they alter account bytes.
- Shortening a string, vector, or compact value zeros the removed bytes.
- Constructors initialize inactive capacity before a representation can be copied into account data.

The byte layout remains compatible with v0.1 and the pinned upstream ZeroPod baseline. Golden tests compare the representations. The [benchmark chapter](./benchmarks.md) explains how the repository measures the runtime cost of the stronger contracts.

## Published documentation

The project publishes this book to GitHub Pages after `main` and release builds pass. The same workflow builds the Rust API documentation with warnings denied.

To verify both outputs locally, run:

```sh
devenv shell verify:docs
```

Use the [`pinapod` API reference](https://docs.rs/pinapod) for item signatures. Use this book for layout choices, migration steps, and safety reasoning.
