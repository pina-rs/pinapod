---
pinapod-derive: fix
---

# generated compact modules bind the prelude names they spell

The compact generators wrap their output in a child module that opens with `use super::*`, and a glob import outranks the prelude. A schema module that defines (or glob-imports) an item named `Ok` with a matching signature therefore rebound every validation return the generated code spelled: the generated commit path compiled the shadow in and called it at runtime. An `Ok`-shaped function that panics turned every generated read, update, and initialize into a panic; `Some`/`None`-shaped functions and a `usize`-named type instead broke compilation with type errors pointing inside the generated module. The fixed generator was already immune through fully qualified `::core` spellings, so only the compact generators were exposed.

The generated compact struct and enum modules now bind those names to `core` explicitly (`use ::core::option::Option::{self, None, Some}` and companions), which outranks the schema module's glob for every bare `Result`, `Option`, `Some`, `None`, `Ok`, `Err`, and primitive the generated body spells, without rewriting the hundreds of emission sites. Schema modules can shadow prelude names again without reaching generated code.

The regression is pinned from three directions: `compact_prelude_shadowing.rs` defines a panicking `Ok` shadow plus type-identical `Result`/`Option` aliases in the schema module and runs full initialize/update/read round trips for both a compact struct and a compact enum — before the fix the first generated `Ok(())` panicked — and the `compact_prelude_patterns` fixture covers the shapes that were compile errors before the fix (`Some`/`None` functions and a `usize`-named type) behind the compile-pass gate.

Zero-variant enums are also now a focused derive error rather than an opaque one: both layout paths previously emitted a degenerate representation for `enum Never {}` and let rustc surface `E0084`/`E0004` from the declaration and the generated module. The derive rejects the schema at the declaration site for both layouts, pinned by `compact_empty_enum` and `fixed_empty_enum` compile-fail fixtures.
