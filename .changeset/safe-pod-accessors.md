---
pinapod: feat
pinapod-derive:
  type: none
  caused_by: ["pinapod"]
---

# volatile access and checked accessors

Two safe ways out of the two remaining sharp edges in the pod surface.

First, every integer pod gains `get_volatile` and `set_volatile`. A CPI callee can write the account a program has already formed its view over, and a plain `get` lets the compiler reuse a load from before the call, because nothing in the caller's source proves the bytes changed — the classic stale-read-after-CPI hazard. A volatile read forces the bytes to be re-read from memory, so a balance or counter updated by a callee is observed after the call returns; a volatile write is emitted as one store that cannot be sunk past a later call. These are the account-data analogue of a `Relaxed` atomic load and store on the single-threaded BPF target, and the docs say exactly that: they prevent compiler caching and provide no synchronization.

Second, `PodString::try_as_str`, `PodVec::try_as_slice`, and `PodOption::try_get_ref` validate before borrowing. `as_str` trusts that a reader validated the bytes before the reference was formed — sound, but it left a consumer holding an unchecked reference (cast from raw account bytes) with no safe exit: calling `as_str` on forged non-UTF-8 bytes is undefined behavior, and `get_ref` silently reports a corrupt option tag as absent. The checked counterparts reject exactly what the reader walk rejects — an over-capacity prefix, invalid UTF-8, an out-of-domain element, a corrupt tag, an invalid present payload — so casting raw bytes and calling them is always sound.

Both are pinned by `pinapod/tests/safe_accessors.rs`: volatile round trips across all eight integer widths, a volatile read observing bytes written through a raw pointer, and the checked accessors' accept and reject paths over forged length prefixes, tags, and payloads.
