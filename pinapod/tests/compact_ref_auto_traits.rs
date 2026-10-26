//! Thread-safety contracts for the typed values exposed by compact views.

#![allow(
	dead_code,
	unsafe_code,
	reason = "schemas are compile-time fixtures with audited custom representations"
)]

use core::cell::Cell;
use core::marker::PhantomData;

use pinapod::PinaPod;
use pinapod::PinaPodError;
use pinapod::ZcElem;
use pinapod::ZcField;
use pinapod::ZcValidate;

// If the type implements the trait, both impls apply and inference fails.
macro_rules! assert_not_impl {
	($ty:ty, $bound:path) => {
		const _: fn() = || {
			trait AmbiguousIfImpl<A> {
				fn check() {}
			}
			impl<T: ?Sized> AmbiguousIfImpl<()> for T {}
			struct Invalid;
			impl<T: ?Sized + $bound> AmbiguousIfImpl<Invalid> for T {}
			let _ = <$ty as AmbiguousIfImpl<_>>::check;
		};
	};
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LocalPod {
	byte: u8,
	marker: PhantomData<Cell<u8>>,
}

impl ZcValidate for LocalPod {
	fn validate_ref(_: &Self) -> Result<(), PinaPodError> {
		Ok(())
	}
}

// SAFETY: the only stored byte is a u8. The marker adds no size, alignment,
// padding, or bit-pattern restrictions, including for the all-zero state.
unsafe impl ZcElem for LocalPod {}

// SAFETY: LocalPod is its own complete alignment-one representation.
unsafe impl ZcField for LocalPod {
	type Pod = Self;
}

// The companion value traits and materialization demand the value vocabulary
// from custom pods; these impls compare and print the stored byte and keep the
// Cell marker's !Sync semantics untouched.
impl PartialEq for LocalPod {
	fn eq(&self, other: &Self) -> bool {
		self.byte == other.byte
	}
}

impl Eq for LocalPod {}

impl core::hash::Hash for LocalPod {
	fn hash<__State: core::hash::Hasher>(&self, state: &mut __State) {
		self.byte.hash(state);
	}
}

impl core::fmt::Debug for LocalPod {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("LocalPod")
			.field("byte", &self.byte)
			.finish()
	}
}

impl pinapod::FromZc for LocalPod {
	fn from_zc(pod: &Self) -> Result<Self, PinaPodError> {
		Ok(*pod)
	}
}

#[repr(transparent)]
#[derive(Clone, Copy)]
struct SyncOnlyPod(u8, PhantomData<*const ()>);

// SAFETY: shared access only reads a u8; the raw-pointer marker stores no pointer.
unsafe impl Sync for SyncOnlyPod {}

impl ZcValidate for SyncOnlyPod {
	fn validate_ref(_: &Self) -> Result<(), PinaPodError> {
		Ok(())
	}
}

// SAFETY: the representation is a u8, without padding, and every bit pattern
// is valid, including the all-zero initialization state.
unsafe impl ZcElem for SyncOnlyPod {}

// SAFETY: SyncOnlyPod is its own complete alignment-one representation.
unsafe impl ZcField for SyncOnlyPod {
	type Pod = Self;
}

impl PartialEq for SyncOnlyPod {
	fn eq(&self, other: &Self) -> bool {
		self.0 == other.0
	}
}

impl Eq for SyncOnlyPod {}

impl core::hash::Hash for SyncOnlyPod {
	fn hash<__State: core::hash::Hasher>(&self, state: &mut __State) {
		self.0.hash(state);
	}
}

impl core::fmt::Debug for SyncOnlyPod {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("SyncOnlyPod").field("0", &self.0).finish()
	}
}

impl pinapod::FromZc for SyncOnlyPod {
	fn from_zc(pod: &Self) -> Result<Self, PinaPodError> {
		Ok(*pod)
	}
}

struct LocalSchema(PhantomData<*const ()>);

// SAFETY: only the associated pod is stored, and u8 meets the ZcElem contract.
unsafe impl ZcField for LocalSchema {
	type Pod = u8;
}

impl PartialEq for LocalSchema {
	fn eq(&self, _other: &Self) -> bool {
		true
	}
}

impl Eq for LocalSchema {}

impl core::hash::Hash for LocalSchema {
	fn hash<__State: core::hash::Hasher>(&self, _state: &mut __State) {}
}

impl core::fmt::Debug for LocalSchema {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.write_str("LocalSchema")
	}
}

impl pinapod::FromZc for LocalSchema {
	fn from_zc(_pod: &u8) -> Result<Self, PinaPodError> {
		Ok(Self(PhantomData))
	}
}

#[derive(PinaPod)]
#[pinapod(compact)]
struct Inline {
	pub value: LocalPod,
}

#[derive(PinaPod)]
#[pinapod(compact)]
struct Tail {
	values: pinapod::Vec<LocalPod, 4>,
}

#[derive(PinaPod)]
#[pinapod(compact)]
struct OptionalTail {
	values: Option<pinapod::Vec<LocalPod, 4>>,
}

#[derive(PinaPod)]
#[pinapod(compact)]
struct Generic<T: ZcField> {
	pub value: T,
	values: pinapod::Vec<T, 4>,
}

#[test]
fn non_sync_pods_prevent_sending_or_sharing_views() {
	fn assert_send<T: Send>() {}

	assert_send::<LocalPod>();
	assert_not_impl!(LocalPod, Sync);
	assert_not_impl!(InlineRef<'static>, Send);
	assert_not_impl!(InlineRef<'static>, Sync);
	assert_not_impl!(TailRef<'static>, Send);
	assert_not_impl!(TailRef<'static>, Sync);
	assert_not_impl!(OptionalTailRef<'static>, Send);
	assert_not_impl!(OptionalTailRef<'static>, Sync);
	assert_not_impl!(GenericRef<'static, LocalPod>, Send);
	assert_not_impl!(GenericRef<'static, LocalPod>, Sync);

	assert_eq!(InlineRef::new(&[7]).unwrap().value.byte, 7);
	assert_eq!(TailRef::new(&[1, 0, 7]).unwrap().values()[0].byte, 7);
	assert_eq!(
		OptionalTailRef::new(&[1, 1, 0, 7])
			.unwrap()
			.values()
			.unwrap()[0]
			.byte,
		7,
	);
}

#[test]
fn thread_safe_pods_allow_sending_and_sharing_views() {
	fn assert_send_sync<T: Send + Sync>() {}

	assert_send_sync::<GenericRef<'static, u64>>();
	assert_send_sync::<GenericRef<'static, LocalSchema>>();
	assert_not_impl!(SyncOnlyPod, Send);
	assert_send_sync::<GenericRef<'static, SyncOnlyPod>>();

	let values = [7_u64.into()];
	let patch = GenericPatch::<u64>::new()
		.value(42_u64)
		.replace_values(&values);
	let mut bytes = [0; Generic::<u64>::MAX_SIZE];
	Generic::<u64>::initialize(&mut bytes, &patch).unwrap();

	let view = GenericRef::<u64>::new(&bytes).unwrap();
	let moved = GenericRef::<u64>::new(&bytes).unwrap();

	std::thread::scope(|scope| {
		let shared = &view;
		scope.spawn(move || assert_eq!(shared.values()[0].get(), 7));
		scope.spawn(move || assert_eq!(moved.value.get(), 42));
	});
}
