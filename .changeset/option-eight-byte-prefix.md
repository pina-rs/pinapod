---
pinapod: feat
pinapod-derive:
  type: none
  caused_by: ["pinapod"]
---

# accept eight-byte option tags

`PodOption<T, PFX>` now supports the same prefix widths as strings and vectors: 1, 2, 4, or 8 bytes. Tags decode through `u64` without truncation, and the Kani option proofs cover every width.
