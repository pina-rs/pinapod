# API comparison benchmarks

`pinapod/benches/api_comparison.rs` compares three implementations: the PinaPod code in this checkout, previous PinaPod at commit `71ad8bee53e3e6939fe14760e539942d0f1bdd77`, and upstream `blueshift-gg/zeropod` at commit `78e6e5f4b515e85999bcc719eb8db59d3ca11b13` (`v0.3.5`). Cargo records both exact revisions in `Cargo.lock`; the benchmark and scripts always use `--locked`.

The benchmark derives separate, wire-identical schemas for every implementation. It asserts equality of their fixed and compact encodings before collecting samples, including compact vector counts of 1, 4, 8, and 16. A mismatch therefore fails instead of producing a misleading performance comparison. The normal integration suite repeats this three-way wire check, so `cargo test` catches compatibility regressions without running Criterion. Criterion labels the three contenders as `pinapod-current`, `pinapod-previous-71ad8be`, and `zeropod-upstream-78e6e5f`.

| Workload        | Wire bytes | What is timed                                                                                                |
| --------------- | ---------: | ------------------------------------------------------------------------------------------------------------ |
| Fixed           |         45 | Parse, validation-only, read four fields from a validated view, mutate a valid value, or initialize a value  |
| Compact small   |         36 | Parse, validation-only, access a five-byte string and two `u64` values, update from a small record           |
| Compact maximum |        207 | Parse, validation-only, access a 64-byte string and 16 `u64` values, grow a small record to maximum capacity |

The harness prints the fixed/header/encoded sizes, generated view sizes, and allocation counts for representative PinaPod writes and updates. It uses fixed stack buffers and prebuilt inputs, so any reported allocation comes from the implementation rather than benchmark-buffer setup.

The compact writer types differ by API generation. The current fixture measures the generated `CompactPatch` and reports `current ref=.../patch=...`. The pinned previous and upstream fixtures measure their generated mutable views and report `ref=.../mut=...`. The workload and wire bytes remain the same; the labels make the compared API shapes explicit.

## v0.2 release-candidate results

Authoritative v0.2 timings are intentionally pending the first matching pull-request run of the `benchmark` workflow. A local release-candidate run on 7 September 2026 identified a redundant compact-update validation, which was then removed. The host became shared with other CPU-intensive builds before the required post-fix run, so those earlier measurements are not published as final results.

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
