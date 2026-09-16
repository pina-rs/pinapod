---
pinapod: none
---

# run Kani proofs against pinned Kani and modern solvers

The CI job installed z3 with `apt-get`, which on ubuntu-24.04 resolves to a 2021 release whose wide bitvector reasoning is far too slow. The `kani (u128)` and `kani (i128)` shards took 31.7 and 50.7 minutes and had begun timing out. Two changes fix that.

Each arithmetic harness is split so every expensive operation gets its own verification condition instead of accumulating one large formula, and the resulting harnesses request `cvc5` rather than `z3`. On the same formulas `z3` could not finish the signed 128-bit division proof within 25 minutes, while `cvc5` verifies it in 14 seconds; `z3` 5.1.0 did not help, so this is not a solver-version problem. The u128 shard now verifies 10 harnesses in about 30 seconds and the i128 shard 11 harnesses in about 22 seconds.

CI installs both solvers from their GitHub releases through a new `kani-solvers` action, pinned by SHA-256 per platform so a moved or compromised release asset cannot silently change what the proofs ran against. The Kani version is pinned alongside them. Proofs no longer run through the `devenv` environment, which saves roughly ten minutes per shard and keeps the devenv setup for local runs, where the same profile pins `cvc5` and `z3` for investigation.

Every assertion is preserved: each operation is still proven equal to its native counterpart, including signed overflow and division-by-zero cases. Nothing was weakened, bounded, or removed to make the proofs fit. No public API changes — the split harnesses live behind `#[cfg(kani)]` and the shard scripts make proofs runnable locally for the first time.
