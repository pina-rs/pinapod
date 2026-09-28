---
pinapod: fix
pinapod-derive: fix
---

# trim compact-enum checks and document the generated surface

The compact-enum commit-entry check no longer interprets fixed payloads. `validate_layout` for a variant with a `Fixed` payload ran the nested `PinaPodFixed::validate_prefix` — a full semantic walk — although relocation consumes only the payload's compile-time range. The bounds half is now the pure range check, and the semantic half stays in `validate` and every value-exposing boundary, emitted from the same per-variant fragment so the two walks cannot drift. A derive contract test asserts the split, and the dead tag-size guard after `validate_storage_len` is gone with it: `MIN_SIZE` is always at least the tag size, so the storage check already rejected every short buffer and the branch was unreachable.

Generated code is cheaper and more consistent on SBF. The compact-enum support helpers carry `#[inline(always)]` like their compact-struct twins, the compact `Ref` accessors and `Mut` setters carry `#[inline]`, and staged vector setters validate through `ZcValidate::validate_slice` so a trivially valid element removes the walk exactly as it does in the runtime and patch paths. The offset walk and prefix decode are unchanged by design; their validate-before-use licenses are now stated once on `compute_offset_tokens` and `read_len_expr`, including the associativity argument and which edits would require re-deriving it.

The generated compact `Ref`, `Mut`, and `Patch` surface now emits rustdoc, matching the fixed generator's precedent, and the derive crate's internal public items explain why each exists. The mdt graph grows a `derive-contracts` template: the compact field grammar, the initialize-zeroing contract, the Pina `UpdateResizableAccount` snippet, and the prefix-attribute rejection are each defined once and consumed wherever they were previously hand-copied, including the derive README. The feature table now lists `compact-commit-full-validation` and `kani`, install snippets say `pinapod = "0.4"`, and the benchmark page names Criterion 0.8.2 and carries a current measured comparison, matching the lockfile.
