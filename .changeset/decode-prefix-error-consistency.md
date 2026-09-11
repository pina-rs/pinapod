---
pinapod-derive: fix
---

# report unrepresentable stored lengths as invalid length

Generated compact decode helpers now return `InvalidLength`, matching the handwritten runtime, when a stored length prefix is wider than the target `usize`. On 32-bit targets a hostile eight-byte prefix previously surfaced as `Overflow` from generated readers while fixed readers reported `InvalidLength`; both now agree. Sixty-four-bit targets are unaffected. The expanded i686 regression job caught the disagreement.
