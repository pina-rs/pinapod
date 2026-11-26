---
pinapod: fix
pinapod-derive: fix
---

# Optimize compact layout validation and patch commits

- Collapse triple-pass compact `Patch::try_update_or_len` (`validate` + `updated_len` + `Mut::new` re-walk) into single-pass `Patch::preflight_lens` + `Mut::new_unchecked_with_len`, and eliminate duplicate `current_encoded_len()` walks in `Ref::new` and `Mut::new` via `Ref::validate_with_len`.
- Replace per-field commit range checks and slice copies with a single monotonic endpoint check, `core::ptr::copy_nonoverlapping`, direct LE prefix stores, and tail-0 shift skipping.
- Skip redundant bounds/slice construction in `validate_layout` and for `TRIVIALLY_VALID` vec elements, simplify `validate_storage_len` to a single unsigned range check, make `compact_enum` `Ref::new` and `Patch::try_update_or_len` single-dispatch, and add strategic `#[inline]` hints across traits and generated views (`-21.6%` to `-54.94%` Callgrind `Ir`, `-18.4%` to `-35.1%` Criterion wall time, 0 regressions).
- Add build-time mdBook optimization (`scripts/optimize-docs.ts`) with native system fonts, build-time syntax highlighting, single CSS/JS bundles, and compact on-demand search (`-93.2%` landing doc wire bytes).
