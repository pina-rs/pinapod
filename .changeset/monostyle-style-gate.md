---
pinapod: patch
pinapod-derive: patch
pinapod-fuzz: patch
---

# Apply the monostyle style gate

Blank-line layout from `monostyle fix` (padding around control flow, a blank before returns, collapse of stacked blank runs), with `monostyle.toml` configuring the rules and a CI step that reports findings as inline PR annotations.
