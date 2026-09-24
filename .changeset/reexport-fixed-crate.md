---
pinapod: feat
pinapod-derive:
  type: none
  caused_by: ["pinapod"]
---

# re-export the pinned `fixed` crate from the `fixed` feature

The `fixed` feature now re-exports its dependency as `pinapod::fixed`, so a consumer names the mapped fixed-point types through PinaPod instead of declaring its own `fixed` requirement. `use pinapod::fixed::types::I16F16;` now compiles with `pinapod` as the only dependency, and the book's fixed-point section documents that form.

The re-export matters because the version is an exact pin. `fixed` is held at 1.30.0 since later releases require a newer compiler than PinaPod's Rust 1.89 baseline, and a schema field's `ZcField` mapping belongs to the types of whichever `fixed` release the program resolves. A consumer that declares its own `fixed` therefore owns a requirement it cannot get wrong safely: an incompatible spelling such as `fixed = "1.31"` fails Cargo's resolver outright, and a compatible one such as `fixed = "1"` compiles but obliges the consumer to keep tracking PinaPod's pin. Routing the import through the feature removes the question — `pinapod` supplies the `fixed` it maps, and a program declares `pinapod` and nothing else.

A compile fixture at `pinapod/tests/fixed_reexport` depends on `pinapod` and nothing else and exercises the re-exported path in a derived schema across the 16-, 32-, and 128-bit storage widths. It is run by a test alongside the `renamed_dependency` fixture and fails to compile if the re-export disappears, so the guarantee is checked rather than documented only. The `changeset-policy` release flow refreshes that fixture's lock file with every version bump, matching the existing renamed-dependency entry.
