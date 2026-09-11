---
pinapod-derive: fix
pinapod:
  type: none
  caused_by: ["pinapod-derive"]
---

# restore 0.2 compact update costs

On-chain measurement against Pina's example programs showed 0.3.0's update preflight paying for the reader's offset cache and for checked arithmetic that `validate` had already proved unnecessary. Generated `updated_len` now walks the current tails once with plain locals and arithmetic. Measured on-chain versus 0.2: compact `write` uses 24 fewer compute units, `resize` 96 fewer, and `rename` 8 fewer, with every other instruction unchanged. Wire format, validation order, error mapping, canonical zeroing, and every Kani proof are unchanged.
