---
pinapod: feat
pinapod-derive: feat
---

# validate the buffer inside every generated compact commit

A generated compact writer held a construction-time proof that its buffer was a valid representation, but the proof was call-site discipline rather than something the commit itself re-established: `new_unchecked` exists for the patch paths, and a future construction site could have skipped validation and committed over stale or tampered bytes. Upstream ZeroPod's 0.3.6 safety release (PR #34) fixed the same class by revalidating in `commit`, and this ports that hardening.

Every generated `commit`, tail-bearing or tail-free, now runs the schema's `PinaPodCompact::validate` over the buffer as its first statement, before any offset arithmetic or pointer work, so a stale or tampered writer fails closed instead of relocating tails over bytes that were never validated. The cost is one additional validation walk per commit.

The preflight-consistency `debug_assert_eq!` in `Patch::update` and `Patch::try_initialize` is now a release-mode check as well: if the preflighted length and the committed length ever disagree — which can only indicate a defect in the generated code — the patch returns `InvalidLength` instead of trusting and persisting a length the bytes do not support. The `PinaPodPatch::update` documentation names that error path. The full upstream review, including every already-covered item, is recorded in the book's new upstream review log.
