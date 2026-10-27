---
pinapod: fix
---

# Decode Wincode containers through their nested codecs

`PodVec` and `PodOption` now require a matching fixed-size `SchemaRead` implementation for their elements and decode active values through it, preserving custom wire transformations and codec errors. All container readers reject nonzero inactive storage, including nested padding, so accepted bytes already match the canonical representation emitted by their writers.
