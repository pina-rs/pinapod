# PinaPod fuzzing

Coverage-guided fuzz targets for every path that accepts untrusted account bytes. The suite runs from CI (short smoke runs on relevant pull requests, longer scheduled runs) and can be run locally with [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz), which requires a nightly toolchain.

## Targets

| Target                  | What it covers                                                                    |
| ----------------------- | --------------------------------------------------------------------------------- |
| `fixed_validate`        | Fixed reads, every fixed field kind, and the initialize-then-validate invariant   |
| `compact_validate`      | Compact storage, header, and tail validation plus the cached-offset accessors     |
| `compact_patch`         | Compact `initialize`/`update` commits, `updated_len` preflight, and round-trips   |
| `compact_enum_validate` | Compact enums across repr widths and payload kinds                                |
| `wincode_read`          | Wincode deserialization of pod storage types and canonical serialization          |
| `pod_containers`        | `PodString`, `PodVec`, and `PodOption` mutator sequences with capacity invariants |

## Running locally

```sh
cargo install cargo-fuzz --locked
cargo fuzz run compact_validate -- -max_total_time=60
```

Corpora and crash artifacts live under `fuzz/corpus` and `fuzz/artifacts` and are never committed. When a target finds a crash, minimize it before reporting:

```sh
cargo fuzz tmin compact_validate fuzz/artifacts/compact_validate-crash-*
```

A minimized reproducer that triggers a validation bypass or unsoundness belongs in [SECURITY.md](../SECURITY.md) reporting, not a public issue.
