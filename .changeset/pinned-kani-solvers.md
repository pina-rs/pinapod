---
pinapod: none
---

# run Kani proofs against pinned Kani and modern solvers

The CI job installed z3 with `apt-get`, which on ubuntu-24.04 resolves to a 2021 release whose wide bitvector reasoning is far too slow. The `kani (u128)` and `kani (i128)` shards took 31.7 and 50.7 minutes and had begun timing out. Proofs now run through a `kani` devenv profile that pins Kani from `ifiokjr/nixpkgs` with `z3` and `cvc5` from official nixpkgs, and each arithmetic harness is split so every expensive operation gets its own verification condition. The u128 shard now verifies 10 harnesses in 54 seconds and the i128 shard 11 harnesses in 18 seconds.

Every assertion is preserved: each operation is still proven equal to its native counterpart, including signed overflow and division-by-zero cases. Nothing was weakened, bounded, or removed to make the proofs fit. No public API changes — the split harnesses live behind `#[cfg(kani)]` and the shard scripts make proofs runnable locally for the first time.
