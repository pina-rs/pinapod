<!-- {@podCompactFieldGrammar} -->

- `String<N>`
- `Vec<T, N>` when `T` has a fixed representation
- `Option<T>` when `T` has a fixed representation
- `Option<String<N>>`
- `Option<Vec<T, N>>` when `T` has a fixed representation
- `Vec<String<M>, N>`

<!-- {/podCompactFieldGrammar} -->

<!-- {@podPrefixAttributeRejection} -->

The derive rejects `#[pinapod(prefix = u16)]`. Prefix width belongs in the field type, so the declaration shows the exact wire representation and preserves the width through nested fields.

<!-- {/podPrefixAttributeRejection} -->

<!-- {@podInitializeZeroingContract} -->

Initialization zeroes the complete destination before configuration and validates the finished representation once. If configuration or validation fails, it zeroes the destination again, so a rejected initialization leaves canonical zero bytes rather than a partial value.

<!-- {/podInitializeZeroingContract} -->

<!-- {@podPinaUpdateResizableAccount} -->

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

The field is named `rent_account`, matching Pina's other reallocation builders.

<!-- {/podPinaUpdateResizableAccount} -->
