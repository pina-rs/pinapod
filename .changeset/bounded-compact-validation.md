---
pinapod-derive: fix
pinapod: fix
---

# cut compact validation to bounded plain arithmetic

On-chain compute-unit measurement against an SBF test programme showed the compact reader's per-element arithmetic costing instructions that `validate` had already proved unnecessary. Generated vector validation now strides the payload once through the audited `from_raw_parts` slice pattern instead of re-deriving every element offset, stores each tail end with plain adds for one- and two-byte prefixes only (a compile-time division-form assertion proves `max <= isize::MAX / size_of::<T>()` for those tails), and reads one- and two-byte optional tail prefixes with one bounds compare plus a constant-width decode. Four- and eight-byte prefixes keep the checked arithmetic so lengths that saturate or exceed `usize` on 32-bit targets can never wrap a plain sum before the bounds check. Fixed-layout `validate_exact` now takes a single length compare on the happy path, and the compact `validate` drops a header-length branch that `validate_storage_len` already subsumed. Measured on-chain: compact `validate` with dense non-trivial elements (`PodBool` vector plus optional tails) drops 5 compute units and every other measured instruction is unchanged, with wire format, validation order, error variants, and error precedence unchanged. Schemas declaring a one- or two-byte-prefix vector whose `max * size_of::<T>()` exceeds `isize::MAX` now fail to compile instead of always failing validation at runtime.
