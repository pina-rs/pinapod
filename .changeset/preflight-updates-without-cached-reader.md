---
pinapod-derive: fix
pinapod:
  type: none
  caused_by: ["pinapod-derive"]
---

# restore 0.2 compact update and read costs

On-chain measurement against Pina's example programs showed 0.3.0's offset cache costing compact update instructions compute units (`rename` -24 CU, `write` -9 CU) while never reducing any measured read. Generated `updated_len` now walks the current tails once with locals instead of constructing a view, and generated readers return to the 0.2 shape where each accessor walks the preceding length prefixes. Measured result: `write` +3 CU and `resize` +75 CU improve on 0.2, and `rename` returns to its 0.2 baseline. Wire format, validation order, checked arithmetic, canonical zeroing, and every Kani proof are unchanged; only the generated performance shape changes.
