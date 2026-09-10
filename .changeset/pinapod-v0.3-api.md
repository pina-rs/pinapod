---
pinapod: breaking
pinapod-derive:
  type: none
  caused_by: ["pinapod"]
---

# harden the error type and option tag surface

`PinaPodError` now implements `core::error::Error` and is `#[non_exhaustive]`, so downstream matches need a wildcard arm to receive new validation variants in later releases. `PodOption::raw_tag` returns `u64` instead of `u32` so eight-byte tags never truncate into a valid value, and `PodString::is_empty`/`PodVec::is_empty` now use the capacity-clamped length, which only changes behavior for zero-capacity containers. The `decode_len` sentinel and hidden generated-code types gained documented stability policies.
