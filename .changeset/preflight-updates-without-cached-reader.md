---
pinapod-derive: fix
pinapod:
  type: none
  caused_by: ["pinapod-derive"]
---

# preflight compact updates without building the cached reader

Generated `updated_len` now walks the current tails once with locals instead of constructing the caching `Ref`. The preflight performs no offset stores and builds no view, so compact update instructions no longer pay for the read-path offset cache: on the Pina example suite, `rename` and `write` instructions regained the 24 and 9 compute units that 0.3.0 cost them. Reads keep the cached constant-time accessors, and the preflight still validates every input value and the complete current representation before any bytes change.
