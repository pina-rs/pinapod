---
pinapod: fix
---

# wincode writers validate the content their readers judge

The wincode writers rejected a corrupt length prefix and a corrupt option tag, but emitted corrupt _content_ verbatim: a `PodString` whose prefix was in range over non-UTF-8 bytes, a `PodVec<PodBool>` with an out-of-domain element, and a `PodOption` with a valid tag over an invalid payload all serialized successfully into bytes the crate's own reader refuses. The canonical-output contract therefore held only for already-valid values, which only an unchecked reference could violate — but that is exactly the tier the prefix and tag checks were added for, so the boundary was inconsistent.

Every writer now runs the `ZcValidate` walk its reader runs before the first byte is written: `PodBool`, `PodString`, `PodVecRepr`, and `PodOption` each reject invalid stored content with a focused message (`PinaPod string content failed validation`, and the matching variants), and the option writer checks its tag _before_ writing the prefix, so all three rejection orderings match and a rejected serialization leaves the destination untouched. Valid values serialize identically; the added walk is proportional to what the writer already emits and is a no-op for trivially valid elements through the existing `ZcValidate::validate_slice` fast path.

The regression is pinned from the forging side in `wincode_serialize.rs`: five tests hand-cast values whose metadata is in range and whose content is invalid, and assert serialization fails without touching the destination.

`PodString`'s documentation now also states the precondition its accessors trust: `as_str` forms its reference through `from_utf8_unchecked`, so the active bytes are UTF-8 because a safe constructor wrote them or a validating reader accepted them — not because the length clamp checks them. `as_bytes`, `chars`, `bytes`, and `truncate`'s boundary walk carry the same note, and a `validation.rs` regression pins the boundary by proving every validating reader rejects a forged non-UTF-8 string before a reference exists.
