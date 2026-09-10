---
pinapod: fix
pinapod-derive:
  type: none
  caused_by: ["pinapod"]
---

# pin the derive to its exact released version

The runtime crate now requires `pinapod-derive = "=x.y.z"` instead of a caret range. Generated code expands against the runtime crate's private contracts, so a mixed pinapod/pinapod-derive pair could emit code the runtime does not match. Both crates continue to release together through MonoChange, which updates the pin.
