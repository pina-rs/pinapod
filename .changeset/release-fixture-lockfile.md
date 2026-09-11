---
pinapod:
  type: none
pinapod-derive:
  type: none
  caused_by: ["pinapod"]
---

# refresh the renamed-dependency fixture lock during releases

The compile fixture at `pinapod/tests/renamed_dependency` carries its own `Cargo.lock` that pins the path dependency's version. The release flow now refreshes it alongside the workspace lock so the fixture's `--locked` compile check stays valid across version bumps.
