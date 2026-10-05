//! Fuzzes the fixed-layout reader over a schema that exercises every fixed
//! field kind: integers, bool, enum discriminants, arrays, bounded strings,
//! bounded vectors, and nested options.
//!
//! Post-conditions asserted after a successful read mirror the safety model:
//! accessor lengths stay within capacity and a successful `initialize`
//! always produces bytes that validate.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pinapod::PinaPod;
use pinapod::String;
use pinapod::Vec;
use pinapod::pod::PodBool;

#[allow(dead_code)]
#[repr(u8)]
#[derive(PinaPod)]
enum Tier {
	Base = 0,
	Silver = 1,
	Gold = 3,
}

#[allow(dead_code)]
#[derive(PinaPod)]
struct Kitchen {
	authority: [u8; 32],
	amount: u64,
	balance: i128,
	active: bool,
	tier: Tier,
	name: String<12>,
	tags: Vec<u16, 6>,
	note: Option<String<9>>,
	flags: Option<Vec<PodBool, 3>>,
}

// A second schema whose containers use four- and eight-byte length prefixes,
// so the wide fixed-layout decode and validation paths see attacker-chosen
// prefixes rather than only the unit fixtures.
#[allow(dead_code)]
#[derive(PinaPod)]
struct WidePrefixKitchen {
	head: pinapod::PodString<80, 4>,
	tail: pinapod::PodString<40, 8>,
	values: pinapod::PodVec<u32, 64, 8>,
}

fuzz_target!(|data: &[u8]| {
	let validation = Kitchen::validate_prefix(data);

	// A short buffer must never validate, and validation success must
	// agree with read success.
	if data.len() < Kitchen::SIZE {
		assert!(validation.is_err(), "a short buffer must never validate");
	}
	if let Ok(()) = validation {
		let view = Kitchen::read_prefix(data).expect("validate_prefix agreed with read_prefix");
		assert!(view.name().len() <= 12);
		assert!(view.tags().len() <= 6);
		match view.note() {
			Some(note) => assert!(note.len() <= 9),
			None => {}
		}
		match view.flags() {
			Some(flags) => assert!(flags.len() <= 3),
			None => {}
		}
	}

	// Exact reads only accept a complete, valid representation.
	let expect_exact = validation.is_ok() && data.len() == Kitchen::SIZE;
	assert_eq!(
		Kitchen::read_exact(data).is_ok(),
		expect_exact,
		"read_exact must require exact size and valid content"
	);

	let wide_validation = WidePrefixKitchen::validate_prefix(data);
	if data.len() < WidePrefixKitchen::SIZE {
		assert!(
			wide_validation.is_err(),
			"a short buffer must never validate"
		);
	}
	if let Ok(()) = wide_validation {
		let view =
			WidePrefixKitchen::read_prefix(data).expect("validate_prefix agreed with read_prefix");
		assert!(view.head().len() <= 80);
		assert!(view.tail().len() <= 40);
		assert!(view.values().len() <= 64);
	}

	assert_eq!(
		WidePrefixKitchen::read_exact(data).is_ok(),
		wide_validation.is_ok() && data.len() == WidePrefixKitchen::SIZE,
		"read_exact must require exact size and valid content"
	);

	// A successful initialization must leave a validating representation.
	let mut destination = [0u8; Kitchen::SIZE];
	let result = Kitchen::initialize(&mut destination, |value| {
		value.name.try_set("fuzz")?;
		value.tags.try_set([1_u16, 2, 3])?;
		Ok(())
	});
	match result {
		Ok(_) => {
			assert!(Kitchen::validate_exact(&destination).is_ok());
		}
		Err(_) => {
			assert!(destination.iter().all(|byte| *byte == 0));
		}
	}
});
