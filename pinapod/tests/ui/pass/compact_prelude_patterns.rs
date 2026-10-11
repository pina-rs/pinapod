// The generated compact modules bind the prelude names they spell to `core`
// explicitly, so a schema module that defines shadowing items cannot rebind
// generated code. The shadows below are the shapes that produced compile
// errors from inside the generated module before those imports existed:
// `Some` and `None` functions cannot appear in pattern position, and a
// `usize`-named type breaks every bare `usize` the generated code spells.
// This fixture only has to compile, not run.

use pinapod::PinaPodCompact;

mod schema {
	use pinapod::PinaPod;

	// These shadowing items are deliberately never called: they exist to
	// prove the generated module resolves its own names instead.
	#[allow(dead_code, non_snake_case)]
	fn Some<T>(value: T) -> Option<T> {
		let _ = value;
		unreachable!("schema module rebound the generated Some")
	}

	#[allow(dead_code, non_snake_case)]
	fn None<T>() -> Option<T> {
		unreachable!("schema module rebound the generated None")
	}

	#[allow(dead_code, non_camel_case_types)]
	struct usize;

	#[derive(PinaPod)]
	#[pinapod(compact, crate = pinapod)]
	pub struct Holder {
		#[allow(dead_code)]
		pub id: u64,
		#[allow(dead_code)]
		note: Option<pinapod::String<8>>,
	}
}

fn main() {
	let mut data = vec![0u8; <schema::Holder as PinaPodCompact>::MAX_SIZE];
	let patch = schema::HolderPatch::new().id(1).note(None);
	let encoded = schema::Holder::initialize(&mut data, &patch).unwrap();
	let view = schema::Holder::read_prefix(&data).unwrap();
	assert_eq!(view.id, 1);
	assert_eq!(view.note(), None);
	assert_eq!(encoded, view.encoded_len());
}
