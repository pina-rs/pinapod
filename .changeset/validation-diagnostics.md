---
pinapod: feat
pinapod-derive: feat
---

# diagnose: attribute rejections to fields

A flat `PinaPodError` answers whether a buffer holds a valid value; it cannot answer which field rejected it. Debugging a corrupt account — in an explorer, a simulator, a test failure — was guesswork over the byte layout.

Every derived schema now exposes `Type::diagnose(data)`, which re-runs the validation walk with attribution and returns a `Diagnostic` carrying the rejecting field's name beside the underlying `PinaPodError`. Fixed schemas attribute each field's validation; compact schemas attribute the allocation checks to the schema's own name (no field owns them), the header's inline fields to their names, and every tail rejection — tag, capacity, bounds, UTF-8, element domain — to the tail's field. A nested schema field is attributed at its own name, and the nested `diagnose` can be called on the field's bytes for one level deeper.

The acceptance contract is the readers': `diagnose` accepts and rejects exactly what the corresponding validation accepts and rejects, with the same error variant. That contract is not a claim but a test — `validation_diagnostics.rs` pins it across 2,000 pseudo-randomly corrupted buffers in addition to every attribution site — and the compact walk uses checked arithmetic throughout, because unlike the reader accessors it deliberately runs on bytes that may be invalid.

`Diagnostic` implements `Display` (`field \`label\`: invalid UTF-8 in string field`),`core::error::Error`with the underlying error as its`source`, and`From<Diagnostic> for PinaPodError`, so it drops into existing error handling unchanged.
