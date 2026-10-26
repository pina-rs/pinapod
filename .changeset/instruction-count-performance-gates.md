---
pinapod: patch
pinapod-derive:
  type: none
  caused_by: ["pinapod"]
---

# instruction-count performance gates on every pull request

Every pull request now runs an instruction-count gate over PinaPod's hot paths, in addition to the wall-clock API comparison. The new `performance` workflow measures the `iai_hot_paths` bench through Valgrind's Callgrind and compares the retired-instruction counts against a committed baseline with a +3% tolerance. Instruction counts are a property of the binary rather than the machine, so the comparison is exact on any Linux runner and no timing noise can hide a regression; that determinism is why the gate can run on every PR instead of only on performance-motivated ones.

The gate covers fixed parse, validate, read, write, and initialize; compact small and maximum parse, validate, access, and update; the six-tail accessors; and, new in this change, large-tail parse, validate, access, and grow-to-full update at a 64 KiB tail. The large-tail fixtures exist because accounts may allocate up to 10 MiB and the previous 207-byte fixture could not observe scaling at real account sizes. The fixtures are shared with the Criterion harness through `pinapod/benches/common`, so both harnesses measure the same workloads over the same bytes, and the Criterion harness also gained a `bytemuck-unchecked-cast` floor for the fixed parse, which prices the same read with validation removed.

The baseline lives in `pinapod/benches/iai-baseline.json` and changes only through `devenv shell bench:iai:update`, whose diff must be justified in the pull request. A tolerance-exceeding regression without a baseline update fails the check by design. The book documents the gate, the update policy, and the new workloads in `docs/benchmarks.md`, and a new cookbook page covers discriminators and schema versioning built on `read_prefix`.
