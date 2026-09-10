---
pinapod: feat
pinapod-derive: feat
---

# cache compact tail offsets in the reader

Generated compact `Ref` views compute every tail offset once during construction instead of re-decoding all preceding length prefixes on each accessor call. Reading all fields of a schema with k tails drops from O(k²) prefix decodes to O(k), and each accessor is constant time. Compact vector validation also strides over the proven-bounded payload directly instead of re-checking multiplication per element. A six-tail benchmark group (`compact/many-tail-fields`) guards the scaling. Generated views are two words larger; wire bytes and validation semantics are unchanged.
