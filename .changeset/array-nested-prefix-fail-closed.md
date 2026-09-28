---
pinapod: fix
pinapod-derive: fix
---

# reject array-nested prefixes instead of rewriting them

A compact field declared as `[PodString<8, 3>; 4]` previously skipped the prefix-width check entirely: validation descended through generic arguments but never through array elements, so the element-wise mapping silently re-encoded the field as `PodString<8, 1>` and the account compiled with a different wire layout than the schema declared. The runtime still validated whatever bytes the rewritten type produced, so this was never memory-unsound, but a client generated from the declared schema would disagree with the on-chain bytes — exactly the divergence the crate's every-unsupported-declaration-is-a-compile-error contract exists to prevent.

The declaration is now a compile error with a focused diagnostic. The descent also covers `Option<[PodVec<u8, 8, 0>; 2]>` and parenthesized spellings, and a valid explicit width inside an array (such as `[PodString<300, 2>; 4]`) is still accepted. The regression is pinned from three directions: a derive unit test that fails on the pre-fix parser, two `ui/fail` fixtures with checked `.stderr` snapshots that compile successfully on the pre-fix parser, and a new adversarial suite (`tests/array_nested_prefixes.rs`) attacking the runtime side of the same surface — forged inner length prefixes, non-UTF-8 payloads, forged option tags over inactive array payloads, and short allocations, each rejected by the walk that owns it, with the layout-versus-semantics split asserted explicitly.
