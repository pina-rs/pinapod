---
pinapod: docs
pinapod-derive:
  type: docs
  caused_by: ["pinapod"]
---

# add fuzzing and broader toolchain verification

A cargo-fuzz suite now covers every untrusted-input reader: fixed validation, compact validation, compact patch commits, compact enums, wincode reads, and container mutators, with smoke runs on relevant pull requests and a weekly scheduled run. Kani proofs cover a derive-generated compact schema (validated accessor bounds, initialize and update round-trips) in a new CI shard. The full test suite runs on current stable and on the MSRV, the 32-bit job runs the fixed and compact regression suites, and `cargo-deny` rejects yanked crates. New book pages document the MSRV/Agave support policy, derive type resolution, and the deliberate zero-fill costs.
