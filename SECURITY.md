# Security Policy

PinaPod treats memory soundness, invalid-reference construction, unchecked offset arithmetic, and disclosure of inactive storage as security issues.

Validation bypasses, wire-format ambiguity, and differences between safe readers and writers are also in scope. Include generated derive code in a report even when the unsafe operation occurs in the `pinapod` runtime crate.

Please report vulnerabilities through GitHub's private vulnerability reporting for `pina-rs/pinapod`. Do not open a public issue containing an exploit or sensitive account data.

Reports should include the affected feature set, target architecture, minimal reproducer, and any Miri or sanitizer output available. Maintainers will coordinate disclosure after a fix and patched release are ready.

Do not include real account keys, private keys, or production account data. A small synthetic byte fixture is enough for most validation and layout reports.

The [safety model and regression map](docs/src/safety.md) explains the contracts that a fix must preserve. A soundness fix must add the smallest permanent native, Miri, compile-fail, or Kani regression that demonstrates the original path.
