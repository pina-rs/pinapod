---
pinapod: none
pinapod-derive: none
---

# finish the tabs-to-spaces switch and fix style fallout

The `hard_tabs = false` switch left most of the workspace formatted under the previous tab style, so `lint:format` (`dprint check`) failed across benches, tests, and both crates. `dprint fmt` completes the conversion, including the `tests/ui` fixtures, and `mdt check` stays green because the mdt `indent:"    "` directives now match the enforced style.

The compile-fail snapshots are re-blessed for the resulting span shifts and for the richer const-eval diagnostics of the pinned nightly, which also renders the containers' forced capacity assertions differently.

Two new tool lints needed reasoned workspace-policy entries. `rustdoc::invalid_markdown_table` rejects mdt's block close markers when they sit inline after the final row of a generated table, but the marker is an invisible synchronization delimiter, so the lint is always a false positive here. `clippy::used_underscore_items` misreads the containers' `let _ = Self::_CAP_CHECK` idiom, which references an underscore-prefixed const on purpose to force its compile-time assertion. No public API, wire format, or runtime behavior changes.
