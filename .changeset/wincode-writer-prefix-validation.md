---
pinapod: fix
pinapod-derive: fix
---

# reject corrupt length prefixes at the Wincode write boundary

The Wincode `SchemaWrite` implementations for `PodString` and `PodVec` emitted the stored length prefix verbatim. A container holding a corrupt prefix — one that decodes above its capacity, or wider than `usize` on a 32-bit target — therefore serialized into a wire value that fails on re-read: silent corruption at a format boundary rather than an error at write time. The option writer already rejected invalid tags; the string and vector writers now apply the matching check and fail with a `WriteError` naming the container, so a value that cannot round-trip is rejected where it is written instead of where it is read back.
