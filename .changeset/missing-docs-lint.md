---
pinapod: docs
pinapod-derive: docs
---

# document the public API and enforce `missing_docs`

Every publicly reachable item in `pinapod` now carries documentation: the crate root, both public modules, each pod type and container method, the error variants, and every trait, associated type, constant, and method in `traits.rs`. The derive crate documents its crate root, the `PinaPod` macro, and every struct and field option it accepts.

The workspace lint policy denies `missing_docs` alongside `unsafe_code`, so an undocumented public item in either library fails the build instead of shipping an undocumented entry in the wire-format contract. Test, benchmark, and fixture targets opt out with a reasoned `#[allow(missing_docs, reason = "...")]`, matching how `unsafe_code` is scoped.

Shared wording lives in one place. `mdt` providers in `api-docs.t.md` (API contracts expanded into rustdoc) and `templates/` (tables and contracts expanded into the README and the book) replace the text that was previously repeated across the pod containers, the error table, the float bit-pattern rules, and the prefix-width rules. `devenv shell docs:sync` rewrites the consumers, and `verify:docs` runs `mdt check` so a stale block fails CI. No API, wire format, or runtime behavior changes.
