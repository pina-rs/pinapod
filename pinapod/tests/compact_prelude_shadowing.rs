//! Generated compact code must not resolve bare prelude names through the
//! schema module.
//!
//! The compact generators wrap their output in a child module that opens with
//! `use super::*`, so a glob import outranks the prelude for every bare
//! `Ok`, `Err`, `Result`, `Option`, and primitive the generated code spells.
//! A schema module that defines (or glob-imports) an item with one of those
//! names rebinds the generated code: an `Ok`-shaped function with a matching
//! signature compiled into every validation return and panicked at runtime
//! when the derived reader ran, while the `Some`/`None` constructors and
//! primitive names produced type errors from inside the generated module
//! instead of the schema's own scope. The generated modules bind those names
//! to `core` explicitly, so the round trips below hold no matter what the
//! schema module defines.
//!
//! The shadows here use the shapes that compiled before the fix: an `Ok`
//! function generic enough to type-check at every `Ok(())` return site, plus
//! type-identical `Result` and `Option` aliases. The `compact_prelude_patterns`
//! fixture pins the shapes that were compile errors before the fix.

#![allow(
	dead_code,
	missing_docs,
	unused_qualifications,
	reason = "these fixtures shadow prelude names on purpose, so their schemas and helper items \
	          are not a published surface and spell fully qualified paths deliberately"
)]

mod schema {
	// A schema module item that rebinds the bare `Ok` the generated code
	// spells at every validation return. The signature matches the call sites
	// exactly, so before the fix this compiled and the panic fired inside the
	// generated commit path.
	#[allow(
		non_snake_case,
		clippy::needless_pass_by_value,
		reason = "the shadow reuses the constructor's name and by-value signature on purpose, so \
		          it type-checks at every generated `Ok(())` call site"
	)]
	fn Ok<T, E>(value: T) -> ::core::result::Result<T, E> {
		let _ = value;
		panic!("schema module rebound the generated Ok");
	}

	// Type-identical aliases: these compiled before the fix too, but they made
	// the generated signatures resolve through the schema module. They stay to
	// show the generated code no longer consults them either.
	type Result<T, E> = ::core::result::Result<T, E>;
	type Option<T> = ::core::option::Option<T>;

	#[derive(pinapod::PinaPod)]
	#[pinapod(compact, crate = pinapod)]
	pub struct Profile {
		pub id: u64,
		label: pinapod::String<16>,
		note: ::core::option::Option<pinapod::String<8>>,
	}

	#[repr(u8)]
	#[derive(pinapod::PinaPod)]
	#[pinapod(compact, crate = pinapod)]
	pub enum Event {
		Empty = 0,
		Named(pinapod::String<8>) = 1,
	}
}

use pinapod::PinaPodCompact;

#[test]
fn compact_struct_round_trips_inside_a_shadowing_module() {
	let mut data = vec![0_u8; <schema::Profile as PinaPodCompact>::MAX_SIZE];
	let patch = schema::ProfilePatch::new()
		.id(7)
		.label("probe")
		.note(::core::option::Option::Some("empty"));

	let encoded = schema::Profile::initialize(&mut data, &patch).unwrap();
	let view = schema::Profile::read_prefix(&data).unwrap();
	assert_eq!(view.id, 7);
	assert_eq!(view.label(), "probe");
	assert_eq!(view.note(), ::core::option::Option::Some("empty"));
	assert_eq!(encoded, view.encoded_len());

	// A shrinking update exercises the commit relocation path, whose staged
	// edits and range checks spell bare `Ok`, `Result`, and `Option` too.
	let shorter = schema::ProfilePatch::new()
		.id(7)
		.label("hi")
		.note(::core::option::Option::None);
	let encoded = schema::Profile::update(&mut data, &shorter).unwrap();
	let view = schema::Profile::read_prefix(&data).unwrap();
	assert_eq!(view.label(), "hi");
	assert_eq!(view.note(), ::core::option::Option::None);
	assert_eq!(encoded, view.encoded_len());
}

#[test]
fn compact_enum_round_trips_inside_a_shadowing_module() {
	let mut data = vec![0_u8; <schema::Event as PinaPodCompact>::MAX_SIZE];
	let patch = schema::EventPatch::Named("probe");

	let encoded = schema::Event::initialize(&mut data, &patch).unwrap();
	let view = schema::Event::read_prefix(&data).unwrap();
	match view {
		schema::EventRef::Empty => panic!("expected the Named variant"),
		schema::EventRef::Named(label) => assert_eq!(label, "probe"),
	}
	// Tag (1) + one-byte prefix (1) + payload (5).
	assert_eq!(encoded, 7);

	// Swap to the unit variant: the shrink path zeroes the vacated payload
	// while the same shadowed names resolve through the generated module.
	let encoded = schema::Event::update(&mut data, &schema::EventPatch::Empty).unwrap();
	let view = schema::Event::read_prefix(&data).unwrap();
	assert!(matches!(view, schema::EventRef::Empty));
	// The `Empty` tag (zero) plus five zeroed bytes where the vacated prefix
	// and payload used to be.
	assert_eq!(encoded, 1);
	assert_eq!(&data[..7], &[0_u8, 0, 0, 0, 0, 0, 0]);
}
