# Security Policy

PinaPod treats memory soundness, invalid-reference construction, unchecked offset arithmetic, and disclosure of inactive storage as security issues.

Validation bypasses, wire-format ambiguity, and differences between safe readers and writers are also in scope. Include generated derive code in a report even when the unsafe operation occurs in the `pinapod` runtime crate.

Please report vulnerabilities through GitHub's private vulnerability reporting for `pina-rs/pinapod`. Do not open a public issue containing an exploit or sensitive account data.

Reports should include the affected feature set, target architecture, minimal reproducer, and any Miri or sanitizer output available. Maintainers will coordinate disclosure after a fix and patched release are ready.

Do not include real account keys, private keys, or production account data. A small synthetic byte fixture is enough for most validation and layout reports.

The [safety model and regression map](docs/src/safety.md) explains the contracts that a fix must preserve. A soundness fix must add the smallest permanent native, Miri, compile-fail, or Kani regression that demonstrates the original path.

## Development toolchain provenance

The verification toolchain is part of the security boundary: CI executes it against untrusted pull-request code, and it produces the published release artifacts.

The devenv environment builds three tools from a personal nixpkgs fork pinned by branch in `devenv.yaml` (`github:ifiokjr/nixpkgs/main`): `kani` (formal verification), `mdt` (documentation synchronization), and `monochange` (release management). The flake lock pins the exact revision and hash, so a build is reproducible until someone runs a lock update. That update is a privileged operation: review the fork's new commits the same way a dependency bump is reviewed, because they replace the tools that gate every merge and cut every release. Prefer moving a tool to upstream `nixpkgs` or an in-repo overlay when it becomes available there, so the trust anchor narrows over time.

Everything else in the environment comes from upstream `nixpkgs` (`nixpkgs-unstable`, lock-pinned), `oxalica/rust-overlay`, or crates.io via the committed lock files. Rust toolchains are pinned by date in `rust-toolchain.toml`.
