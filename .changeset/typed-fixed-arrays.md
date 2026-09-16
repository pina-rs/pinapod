---
pinapod: major
pinapod-derive: major
---

# support typed fixed arrays `[T; N]` in every schema position

`ZcElem`, `ZcValidate`, and `ZcField` are now implemented for arrays of any pod element, not only `[u8; N]`. A field declared `[u64; 4]` stores `[PodU64; 4]` little-endian with no length prefix, validation recurses per element, and the identity mapping for `[u8; N]` is preserved exactly. Nested arrays such as `[[u8; 4]; 2]`, pod-spelled arrays such as `[PodU64; 4]`, arrays of `PodBool`, and `Option<[u64; N]>` all resolve through the same composition rules.

The derive now maps array fields element-wise (`[u64; 4]` emits `[PodU64; 4]` storage), recurses capacity checks into array elements, and compact patch builders accept both native and pod spellings through a new hidden `IntoPodArray` conversion trait — `.weights([5, 6])` and `.weights([PodU64::from(5), PodU64::from(6)])` both compile. Fixed-schema accessors return the pod array by reference, matching the existing `Vec<T, N>` accessor shape.

For example, the field type now carries the element type instead of forcing raw bytes, with no change to the wire size:

```rust
// Before: only `[u8; N]` was supported, so a fixed u64 array was raw bytes.
#[derive(PinaPod)]
#[pinapod(compact)]
struct Table {
	weights: [u8; 16],
}
```

```rust
// After: the native element type is accepted throughout, storing `[PodU64; 2]`.
#[derive(PinaPod)]
#[pinapod(compact)]
struct Table {
	weights: [u64; 2],
}

fn round_trip() {
	let mut buffer = [0u8; Table::MAX_SIZE];
	let patch = TablePatch::new().weights([5_u64, 6]);
	let encoded_len = Table::initialize(&mut buffer, &patch).unwrap();
	let table = Table::read_prefix(&buffer[..encoded_len]).unwrap();
	assert_eq!(table.weights[0].get(), 5);
}
```

Soundness of the generalized `ZcElem` rests on the documented array layout rules: `[T; N]` inherits alignment 1 from `T`, its size is exactly `N * size_of::<T>()` so no padding can exist between elements, and per-element bit validity composes. The `[u8; N]`-specific impls were replaced by the generic ones, so downstream crates that hand-wrote `ZcField`/`ZcValidate` impls for their own array types will now conflict with the blanket impls — the closed-world safety guidance already rules that pattern out, but it is the one theoretically breaking edge and the reason both crates bump together.
