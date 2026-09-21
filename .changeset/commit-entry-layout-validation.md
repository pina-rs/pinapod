---
pinapod: fix
pinapod-derive: fix
---

# keep commit-entry revalidation safe at layout cost

0.4.2 added a full `PinaPodCompact::validate` walk to the top of every generated `commit()`. The hardening was right, but the walk was stronger than the pointer arithmetic it guards. Relocation reads exactly three things from the buffer — `data.len()`, the header size, and the stored length prefixes — and then moves bytes between the offsets they imply. The only hazard is an offset or end computed past the buffer, which is a layout property. The full walk additionally visited every tail element and checked UTF-8, enum ranges, and `PodBool` tags: bytes `commit` never interprets, at a cost proportional to the element count rather than the prefix count.

On Pina's `compact_accounts_program` benchmark that measured **+308 CU on `write`, +292 on `resize`, +157 on `rename`, and +46 on `initialize`** against 0.4.1, tracking how many tails each instruction walks. Two changes remove it.

`ZcValidate::validate_slice` joins `validate_array` as an overridable element walk, and every trivially valid type overrides it to a no-op alongside its existing `validate_array` override. Generated tail loops call it instead of spelling the loop at each emission site, so a `Vec<u64>` or `PodVec<u8>` tail no longer burns CU proving `Ok(())` per element — a loop that is merely dead after inlining is not reliably removed at `-C opt-level=3` on SBF. The nested-container walk inside `PodVecRepr::validate_ref` routes through it as well.

`PinaPodCompact::validate_layout` is the commit entry now. It is a _provided_ method defaulting to the full `validate`, so a hand-written implementation cannot weaken itself by accident and existing ones keep compiling unchanged. The derive overrides it with the prefix decode, the capacity check, and the chained bounds, and emits it from the same per-field fragments as `validate` so a bound cannot be tightened in one walk and left stale in the other. The four semantic boundaries — the `Ref` and `Mut` constructors, `updated_len`, and `try_initialize`'s post-commit check — still run the full `validate`, as do the patch-input validations.

Measured against 0.4.1 on the same benchmark, the regression is recovered and three of four instructions are faster: −39 CU on `initialize`, −110 on `resize`, −104 on `write`, and +36 on `rename` — all inside the gate's noise band, which warns only at +250 CU and +5%.

One behavior refinement is deliberate and documented. A buffer that is layout-valid but semantically invalid — invalid UTF-8 in an untouched tail, say — is no longer rejected by `commit`. It is still rejected at the next read. On the generated update path the stronger invariant still holds by construction: `updated_len` runs the full `validate` over the same bytes the commit will see, because the staged edits are `(ptr, len)` intents that have not been written yet. `try_initialize` keeps its post-commit full `validate`, so initialization cannot persist such bytes either. The new `compact-commit-full-validation` feature restores the full walk at the commit entry for consumers with different CU budgets; it dispatches from the runtime crate rather than a `#[cfg]` in the generated body, because a `cfg` in an expansion resolves against the consumer's feature namespace, where the name is usually undefined and the feature would silently mean nothing.

The measurement, the rejected proof-token alternative, and the residual behavior change are recorded in the book's performance decision log.
