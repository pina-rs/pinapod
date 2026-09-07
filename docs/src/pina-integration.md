# Migrate Pina

Pina and PinaPod can be developed in parallel, but they cannot merge in either order. The Pina change depends on the final PinaPod v0.2 API. Merge and publish PinaPod first, then point Pina at the published release.

## Keep both pull requests usable during development

During development, use a local path override in the Pina worktree. This lets Pina compile against every PinaPod API change without publishing an intermediate crate.

Before the Pina pull request merges:

1. Merge the PinaPod pull request after its native, Miri, Kani, documentation, security, and benchmark checks pass.
2. Publish `pinapod-derive` and `pinapod` v0.2 in dependency order.
3. Replace the Pina path override with the released `pinapod = "0.2"` dependency.
4. Regenerate all Pina clients and documentation.
5. Run the complete Pina workspace and example suites.
6. Merge the Pina pull request.

This order keeps both `main` branches buildable. It also gives Pina's lockfile a published PinaPod source instead of a temporary branch revision.

## Update every machine consumer of the derive name

The rename to `PinaPod` affects code that reads or writes Rust source. Update these Pina components together:

- Account, instruction, and event macros that inject the derive.
- Source parsing that discovers derived enums.
- Pina's public traits and PinaPod re-exports.
- Codama Rust account and instruction renderers.
- TypeScript codec helpers and their generated filenames.
- Checked-in Rust, TypeScript, and Dart clients.
- Snapshots, UI fixtures, examples, and documentation.

A partial rename can compile the program while the CLI silently omits an enum from its IDL. Test enum discovery before relying on generated-client compilation.

Pina's framework macros inject `#[pinapod(crate = pina::pinapod, no_inherent)]`. The crate option resolves the runtime re-export. `no_inherent` leaves method ownership with Pina's account-aware helpers while preserving the generated trait implementations.

The macros mark account discriminators with `#[pinapod(skip_accessor, skip_patch)]`. The derive still stores and validates the discriminator, but application patches cannot change framework-owned bytes.

## Lift fixed-account collection restrictions

After Pina depends on v0.2, allow fixed accounts to contain bounded strings, vectors, options, and recursively bounded combinations. Move the old compile-fail fixtures for these fields to pass fixtures.

Pina must calculate a generic fixed representation with `size_of::<<T as PinaPodFixed>::Zc>()`. Its account-aware API can expose that result as inherent `AccountType::SIZE`. It must not recalculate nested sizes from syntax or accept capacity from an untrusted documentation string.

## Use one validation boundary

Pina checks account ownership, the discriminator, writable status, and Solana-specific resize and rent rules. PinaPod checks the compact allocation against the schema's minimum, maximum, and tail granularity, validates the representation, and returns the view. Do not validate with PinaPod and then call a second validating constructor.

Mutable account loaders must reject a non-writable account before they return a mutable view. Drop every account-data guard before a resize or CPI, then borrow and parse the new allocation again.

## Update compact accounts with one patch

Pina's resizable-account builder accepts the generated patch and manages the borrow and resize order:

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

The field is named `rent_account`, matching Pina's other reallocation builders. The builder performs these steps:

1. Borrow and validate the current account.
2. Calculate the patched encoded length without changing bytes.
3. Release the borrow.
4. Grow the allocation when required.
5. Borrow and validate the resized account.
6. Apply the same patch once.
7. Release the borrow.
8. Shrink the allocation when required.

The patch borrows its input strings and slices. It does not borrow account data, so it can survive the resize between planning and application.

## Keep generated clients at the same boundary

Carry capacities as structured IDL data. Do not recover `N` from prose. Generated Rust, TypeScript, and Dart encoders and decoders must reject:

- A string or vector above its declared capacity.
- A length prefix that exceeds the available bytes.
- Invalid UTF-8.
- An option tag other than zero or one.
- A boolean byte other than zero or one.
- An account allocation outside the schema's compact size policy.

Use shared golden and malformed fixtures across all three languages. Include `None`, `Some(empty)`, multiple unequal tails, the maximum capacity, one item over capacity, and `Vec<String<M>, N>` with different logical string lengths.

## Publish the matching Pina guide

The Pina pull request owns framework-specific instructions. Update its mdBook, crate readmes, source templates, examples, agent references, and migration guide before merging. Run Pina's documentation sync command after editing source templates so generated docs cannot retain the v0.1 API.
