---
pinapod: feat
pinapod-derive: feat
---

# add trivially-valid and zeroed-init fast paths

Add compile-time trivial-validity detection, zero-buffer initialization fast paths, and single-pass compact update helpers so callers (such as Solana on-chain programs) avoid redundant validation, double zeroing, and duplicate update traversals:

- Add `ZcValidate::TRIVIALLY_VALID: bool` (defaulting to `false`) and set it to `true` for bit-unrestricted primitives, fixed-width pods, floats, arrays/wrappers of trivially valid elements, and derived fixed/compact headers whose fields are all trivially valid. Fixed and compact validation paths short-circuit when `TRIVIALLY_VALID` is true.
- Add `PinaPodFixed::initialize_zeroed` and `PinaPodPatch::initialize_zeroed` (with `&P` blanket forwarding) for callers whose target buffer is already guaranteed zero-filled, skipping redundant `fill(0)` passes while preserving full validation.
- Add `PinaPodPatch::try_update_or_len` (with `&P` blanket forwarding) so compact updates attempt in-place commits directly and return `Err(needed_len)` on insufficient capacity without a separate preflight measurement pass.
- Remove unreachable `if __elem_size == 0` branches in `pinapod-derive` compact codegen.
