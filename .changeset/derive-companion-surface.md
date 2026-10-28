---
pinapod: feat
pinapod-derive: feat
---

# value traits and owned materialization

Generated representations were compare-blind: a fixed schema's `Zc` companion had `Copy` and `Clone` but no `PartialEq`, `Hash`, or `Debug`, so `assert_eq!` between two snapshots of the same account did not compile, a snapshot could not key a map, and a test failure printed nothing. Compact views had none of them either.

Every fixed companion and compact view now implements `PartialEq`, `Eq`, `Hash`, and `Debug` over **active** content: two encodings of the same value are equal regardless of inactive capacity, hashing agrees with that equality, and `Debug` renders field names without disclosing vacated bytes. The impls are conditional on the mapped pods carrying the corresponding trait, so a custom `ZcField` mapping without `Hash` keeps compiling and simply has no `Hash` companion impl. The fixed-enum companions and `PodOption` gained the `Eq`/`Hash` pieces they were missing so enum-bearing and optional fields compose into all four.

The second half is materialization: a new `FromZc` trait is the return path for `ZcField`'s outbound mapping, and every fixed companion and compact view exposes `to_native()`, which builds the fully owned declared struct — snapshots for logging and tests, off-chain decoding, cross-thread transfer; the shapes a borrowed view cannot serve. Materialization is fallible for exactly one reason: a stored enum discriminant naming no variant, which cannot occur for a value a validating reader accepted. Compact materialization copies active tail content into the declared containers, so the result never borrows the account bytes.

One contract follows for custom representations: because the generated impls bound on the mapped pods, a schema field using a hand-written `ZcField` mapping now needs that mapping to implement the value vocabulary — `PartialEq`, `Eq`, `Hash`, and `Debug` for the companion traits, `FromZc` for materialization. Built-in and derived-schema mappings already do; `compact_ref_auto_traits.rs` extends its deliberate `!Sync`/`!Send` fixtures with the same impls, which leave their thread-safety semantics untouched.
