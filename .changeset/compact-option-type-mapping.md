---
pinapod-derive: fix
---

# Preserve custom PodOption representation mappings

Compact derives now resolve types named `PodOption` through their `ZcField` contract instead of substituting PinaPod's built-in option storage. Custom qualified types retain their declared validation and layout, including vector elements in compact enums. Genuine PinaPod options retain their existing payload and tag widths through their identity mapping.
