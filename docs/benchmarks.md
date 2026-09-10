# API comparison benchmarks

`pinapod/benches/api_comparison.rs` compares three implementations: the PinaPod code in this checkout, previous PinaPod at commit `71ad8bee53e3e6939fe14760e539942d0f1bdd77`, and upstream `blueshift-gg/zeropod` at commit `78e6e5f4b515e85999bcc719eb8db59d3ca11b13` (`v0.3.5`). Cargo records both exact revisions in `Cargo.lock`; the benchmark and scripts always use `--locked`.

The benchmark derives separate, wire-identical schemas for every implementation. It asserts equality of their fixed and compact encodings before collecting samples, including compact vector counts of 1, 4, 8, and 16. A mismatch therefore fails instead of producing a misleading performance comparison. The normal integration suite repeats this three-way wire check, so `cargo test` catches compatibility regressions without running Criterion. Criterion labels the three contenders as `pinapod-current`, `pinapod-previous-71ad8be`, and `zeropod-upstream-78e6e5f`.

| Workload         | Wire bytes | What is timed                                                                                                |
| ---------------- | ---------: | ------------------------------------------------------------------------------------------------------------ |
| Fixed            |         45 | Parse, validation-only, read four fields from a validated view, mutate a valid value, or initialize a value  |
| Compact small    |         36 | Parse, validation-only, access a five-byte string and two `u64` values, update from a small record           |
| Compact maximum  |        207 | Parse, validation-only, access a 64-byte string and 16 `u64` values, grow a small record to maximum capacity |
| Many-tail fields |        145 | Parse plus access of every field, or only the last field, on a six-tail compact schema                       |

The many-tail fixture guards reader scaling with the number of tail fields rather than the number of elements. A compact `Ref` computes every tail offset once during construction and stores them, so each accessor is a constant-time slice regardless of how many tails precede it; the fixture times both a full six-field sweep and a last-field-only read so the scaling stays visible.

The harness prints the fixed/header/encoded sizes, generated view sizes, and allocation counts for representative PinaPod writes and updates. It uses fixed stack buffers and prebuilt inputs, so any reported allocation comes from the implementation rather than benchmark-buffer setup.

The compact writer types differ by API generation. The current fixture measures the generated `CompactPatch` and reports `current ref=.../patch=...`. The pinned previous and upstream fixtures measure their generated mutable views and report `ref=.../mut=...`. The workload and wire bytes remain the same; the labels make the compared API shapes explicit.

## Deliberate costs that must not be optimized away

Several PinaPod operations do more byte work than a naive implementation because the extra writes are load-bearing for security:

- Shortening a string, vector, or compact tail zeroes the removed bytes.
- Absent option payloads are zeroed when cleared and never serialized.
- Compact updates zero the old suffix when the encoded value shrinks.
- Fixed and compact initialization zero the destination before and after a failed attempt.

These writes prevent stale account data from leaking through inactive capacity later, and they make repeated writes produce identical bytes. Stale-capacity disclosure is in the [SECURITY.md](../SECURITY.md) threat model. A performance change that removes a zero-fill is a security regression, not an optimization.

## v0.2 release-candidate results

The table below reports median latency from GitHub Actions run `34127045383` on 7 September 2026. A positive score means the current implementation is faster than PinaPod v0.1; a negative score means it is slower. The score is `(previous - current) / previous`, so its sign follows performance rather than elapsed time.

| Workload                 | PinaPod v0.2 | PinaPod v0.1 | ZeroPod v0.3.5 | Performance score |
| ------------------------ | -----------: | -----------: | -------------: | ----------------: |
| Fixed parse              |     1.406 ns |     1.406 ns |       1.406 ns |            +0.02% |
| Fixed validation         |     0.703 ns |     0.703 ns |       0.703 ns |             0.00% |
| Fixed read               |     3.516 ns |     3.515 ns |       3.514 ns |            -0.01% |
| Fixed mutation           |     2.044 ns |     2.082 ns |       2.036 ns |            +1.80% |
| Fixed initialization     |     1.979 ns |     2.023 ns |       2.024 ns |            +2.18% |
| Compact-small parse      |     7.032 ns |     6.682 ns |       6.682 ns |            -5.24% |
| Compact-small validation |     6.426 ns |     5.977 ns |       5.975 ns |            -7.51% |
| Compact-small access     |     2.108 ns |     2.108 ns |       2.108 ns |            +0.01% |
| Compact-small update     |    17.231 ns |    16.390 ns |      16.500 ns |            -5.13% |
| Compact-maximum parse    |    12.665 ns |    12.339 ns |      12.436 ns |            -2.65% |
| Compact-maximum validate |    12.694 ns |    11.979 ns |      11.983 ns |            -5.97% |
| Compact-maximum access   |     2.108 ns |     2.108 ns |       2.108 ns |            +0.01% |
| Compact-maximum update   |    18.282 ns |    17.769 ns |      17.666 ns |            -2.89% |

The fixed path is effectively unchanged and its safe one-pass initializer is 2.18% faster than the historical zero-buffer setup. Compact access is also unchanged. Compact parse, validation, and atomic update add between 2.65% and 7.51% in these representative records. That cost buys allocation-bound validation, checked offset arithmetic, preflighted all-or-nothing updates, and stale-suffix clearing. The scaling fixtures show that compact access and validation remain flat as the vector grows; the fixed validation overhead is not proportional to active element count.

The GitHub-hosted job runs all three implementations in one Criterion process on `ubuntu-24.04` with the checked-in Rust toolchain and `Cargo.lock`. It uses Criterion 0.5.1 with 100 samples, a three-second warmup, and a five-second measurement. The job uploads the complete `target/criterion` directory as a `pinapod-api-comparison-<run>-<attempt>` artifact for 14 days. This preserves the raw estimates, distributions, and HTML report used to populate the final release table.

PinaPod v0.2 fixed initialization zeroes the destination before configuration and validates the finished value. PinaPod v0.1 and upstream ZeroPod have no equivalent safe initializer, so their comparison workload is the historical operation available to a caller with a new zeroed buffer: validate that buffer, take a mutable view, and write the fields. Fixed mutation remains a separate apples-to-apples workload.

The compact scaling groups hold the string tail at five bytes and vary the `u64` vector across 1, 4, 8, and 16 values. The harness also records allocation counts for fixed mutation, fixed initialization, compact-small update, and compact-maximum update. It reports generated reference and writer/patch object sizes alongside the wire sizes.

This standalone harness measures native host latency, throughput, allocations, generated stack-object sizes, and encoded byte counts. Encoded bytes are relevant to Solana rent, reallocation, and copy volume, but this harness does not measure SBF compute units. The downstream Pina integration suite is the right place for an SBF program-test compute-unit regression because Pina owns the account borrow, resize, and CPI lifecycle.

## Run

From the repository root, run the full comparison:

```sh
devenv shell bench:compare
```

Criterion reports both latency and throughput. Local results live under `target/criterion/`. Pull requests that change the benchmark, runtime, derive implementation, Cargo graph, Rust toolchain, or development environment run the same locked comparison on GitHub-hosted hardware and upload that directory as an artifact.

Before changing PinaPod, save the current measurements under the stable baseline name:

```sh
devenv shell bench:compare:baseline
```

After the change, compare the same checkout and machine against that saved baseline:

```sh
devenv shell bench:compare:after
```

Use a quiet machine, a release build, and the same target triple for both runs. Do not update either historical git revision, fixture values, or workload byte sizes while evaluating a PinaPod implementation change. If the v0.2 public API changes, adapt only the `pinapod-current` half of the harness so the two pinned fixtures continue to define the historical workload and wire baseline.
