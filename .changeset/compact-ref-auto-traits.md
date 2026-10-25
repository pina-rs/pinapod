---
pinapod-derive: fix
---

# Preserve thread-safety bounds on compact views

Generated compact `Ref` types now implement `Send` and `Sync` only when their header and vector element pods allow shared access across threads. This prevents views from bypassing a custom pod's `!Sync` restriction while preserving thread-safe representation mappings.
