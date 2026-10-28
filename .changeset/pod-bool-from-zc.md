---
pinapod: fix
pinapod-derive:
  type: none
  caused_by: ["pinapod"]
---

# materialize a `PodBool` field

`FromZc` landed as the return path for every `ZcField` mapping, but the identity list omitted `PodBool`. A schema that declares a `bool`'s pod directly (the closed-schema `PodBool` field the Pina macros accept) therefore failed to compile on the generated `FromZc` bound, even though `PodBool` is `Copy`, alignment one, and its reader accepts only the canonical `0` and `1` bytes that `ZcValidate` checks. It is now in the identity list alongside the integer pods, and `companion_surface` materializes a schema with a direct `PodBool` field.

A native `bool` field is unaffected: `FromZc for bool` decodes from its pod, which already existed. This restores the direct-pod form.
