---
pinapod: patch
pinapod-derive: patch
---

# restore the byte-array fast path for `[u8; N]` validation

0.4.0 generalized the `[u8; N]`-specific `ZcValidate` impl into a `[T; N]` impl whose body walks every element. That walk is correct but its per-element work no longer disappears for byte arrays: at `-C opt-level=3` on SBF the optimizer inlines a neighboring restricted-domain check and duplicates a byte load instead of folding the loop away. A program whose only arrays are byte arrays therefore spends compute units it does not need to. Measured against a downstream `pina` checkout, `migrations_program/update` moved 533 to 537 CU in the original report; reproduced in-repo against `pina` at `430a2e95`, the same instruction moved 758 to 761 CU, and 0.3.3 measures 758 with every other input held constant.

Validation now dispatches through a new provided method, `ZcValidate::validate_array`, whose default implementation is the per-element walk. Element types whose every initialized bit pattern is valid override it with a no-op, and `[T; N]` forwards to it. A trivially valid element therefore has no loop in its instantiation at all, while `PodBool`, length-prefix-bearing containers, and every other restricted-domain element keep the walk unchanged — that walk remains the only gate rejecting a non-canonical bool byte, an out-of-range prefix, or non-UTF-8 string bytes. `u8`, `i8`, the integer pods, the float pods, and `solana_address::Address` take the no-op override.

The `ZcElem` safety contract now states the matching obligation: overriding `validate_array` with a no-op asserts that the element's whole domain is valid, exactly as a trivial `validate_ref` does, so an array of a restricted-domain type must keep the default walk. Wire format, sizes, and every rejected-input behavior are unchanged; only the per-element cost of a trivially valid array moves.

`ZcValidate` gains a method with a default body, so downstream implementors keep compiling without changes. Both crates bump together because the runtime crate's exact `=x.y.z` derive pin is what keeps generated code matched to the private contracts it expands against.
