---
pinapod-derive: fix
pinapod: fix
---

# cut compact validation to bounded plain arithmetic

On-chain compute-unit measurement against an SBF test programme showed the compact reader's per-element arithmetic costing instructions that `validate` had already proved unnecessary. Generated vector validation now strides the payload once through the audited `from_raw_parts` slice pattern instead of re-deriving every element offset, stores each tail end with plain adds for prefixes of four bytes or fewer (a new compile-time capacity assertion proves `max * size_of::<T>()` fits `isize`), and reads optional tail prefixes with one bounds compare plus a constant-width decode. Fixed-layout `validate_exact` now takes a single length compare on the happy path, and the compact `validate` drops a header-length branch that `validate_storage_len` already subsumed. Measured on-chain: compact `validate` with dense non-trivial elements (`PodBool` vector plus optional tails) drops 5 compute units and every other measured instruction is unchanged or within run-to-run determinism, with wire format, validation order, error variants, and error precedence unchanged. Schemas declaring a four-byte-or-narrower prefix vector whose `max * size_of::<T>()` exceeds `isize::MAX` now fail to compile instead of always failing validation at runtime.
