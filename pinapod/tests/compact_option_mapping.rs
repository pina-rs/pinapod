//! Colliding type names must preserve their resolved representation and validation.
#![allow(
	dead_code,
	unsafe_code,
	reason = "custom representation fixtures exercise derive hygiene"
)]

use pinapod::PinaPod;
use pinapod::PinaPodError;
use pinapod::ZcElem;
use pinapod::ZcField;
use pinapod::ZcValidate;

mod unrelated {
	pub struct PodOption<T>(pub T);
}

/// Custom storage whose semantic tag differs from a built-in option.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Marker([u8; 2]);

impl ZcValidate for Marker {
	fn validate_ref(value: &Self) -> Result<(), PinaPodError> {
		if value.0[0] != 0xa5 {
			return Err(PinaPodError::InvalidTag);
		}

		Ok(())
	}
}

// SAFETY: Marker is two bytes with alignment one and no invalid Rust bit patterns.
unsafe impl ZcElem for Marker {}

// SAFETY: Marker is already its complete pod representation.
unsafe impl ZcField for Marker {
	type Pod = Self;
}

// SAFETY: Only Marker is stored; it satisfies the alignment-one ZcElem contract.
unsafe impl ZcField for unrelated::PodOption<u8> {
	type Pod = Marker;
}

// The companion value traits and materialization demand the value vocabulary
// from custom pods; Marker already derives the equality and Debug traits, so
// only Hash and the materialization mapping are missing.
impl core::hash::Hash for Marker {
	fn hash<__State: core::hash::Hasher>(&self, state: &mut __State) {
		self.0.hash(state);
	}
}

impl pinapod::FromZc for Marker {
	fn from_zc(pod: &Self) -> Result<Self, PinaPodError> {
		Ok(*pod)
	}
}

impl pinapod::FromZc for unrelated::PodOption<u8> {
	fn from_zc(pod: &Self::Pod) -> Result<Self, PinaPodError> {
		// The stored Marker's two bytes are the semantic tag and the payload;
		// the mapping under test keeps them exactly as written.
		Ok(unrelated::PodOption(pod.0[1]))
	}
}

#[derive(PinaPod)]
#[pinapod(compact)]
struct Inline {
	pub value: unrelated::PodOption<u8>,
}

#[derive(PinaPod)]
#[pinapod(compact)]
#[repr(u8)]
enum Event {
	Values(pinapod::Vec<unrelated::PodOption<u8>, 2>) = 1,
}

#[test]
fn inline_collision_uses_custom_validation_and_storage() {
	assert_eq!(Inline::MAX_SIZE, 2);
	assert!(InlineRef::new(&[0, 0]).is_err());
	let view = InlineRef::new(&[0xa5, 7]).unwrap();
	assert_eq!(view.value, Marker([0xa5, 7]));

	let mut bytes = [0; Inline::MAX_SIZE];
	Inline::initialize(&mut bytes, &InlinePatch::new().value(Marker([0xa5, 9]))).unwrap();
	assert_eq!(bytes, [0xa5, 9]);
}

#[test]
fn enum_vector_elements_use_custom_validation_and_storage() {
	assert!(EventRef::new(&[1, 1, 0, 0, 0]).is_err());
	let EventRef::Values(values) = EventRef::new(&[1, 1, 0, 0xa5, 7]).unwrap();
	assert_eq!(values, &[Marker([0xa5, 7])]);
}

#[derive(PinaPod)]
#[pinapod(compact)]
struct Builtin {
	pub value: pinapod::pod::PodOption<pinapod::pod::PodU16, 4>,
}

#[test]
fn genuine_pod_option_preserves_its_payload_and_prefix() {
	assert_eq!(Builtin::MAX_SIZE, 6);
	let view = BuiltinRef::new(&[1, 0, 0, 0, 0x34, 0x12]).unwrap();
	assert_eq!(view.value.get().unwrap().get(), 0x1234);
	assert!(BuiltinRef::new(&[2, 0, 0, 0, 0, 0]).is_err());
}
