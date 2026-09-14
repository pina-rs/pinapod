---
pinapod: feat
pinapod-derive: feat
---

# add IEEE-754 float pods behind the `floats` feature

The new `floats` feature adds `PodF32` and `PodF64`, alignment-one storage for IEEE-754 `f32` and `f64` values, and implements `ZcField` for the native primitives so a schema can declare `f32` or `f64` fields directly instead of spelling a pod type. Generated accessors decode back to the native float, so `field.get()` and the generated accessor both return `f32`/`f64` the same way the integer pods return `u16`/`u64`.

Storage is the complete bit pattern of the value, little-endian: four bytes for `f32` and eight for `f64`. `get`/`set` convert through the bit pattern while `to_bits`/`set_bits` expose it directly. Every bit pattern is a valid stored value, so `ZcValidate` accepts any bytes, validation cannot reject a NaN, infinity, or the sign of zero, and an all-zero field decodes as `+0.0`. Pod equality is bitwise rather than float-valued, which keeps `Eq` sound with NaN payloads and preserves the distinction between `+0.0` and `-0.0`; decode with `get` to compare with float semantics. The pods are byte containers and provide no arithmetic operators.

`PodF32` and `PodF64` compose with every container: they work as `PodOption` payloads, `PodVec` elements, compact inline header fields, and compact tails, and they implement canonical `wincode` serialization under that feature. The new mapping is additive — no existing wire format, validation order, or error variant changes, and `f32`/`f64` remain unmapped when the feature is disabled.
