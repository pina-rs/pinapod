//! Verifies a consumer can use `fixed` types through `pinapod`'s re-export.
//!
//! This fixture deliberately has no `fixed` dependency of its own; every
//! `fixed` name below resolves through `pinapod::fixed`. If the re-export
//! disappeared, this crate would fail to compile — which is the guarantee,
//! since a consumer that wants these types without restating the pin has to
//! reach them through `pinapod`.

use core::mem::size_of;

use pinapod::PinaPod;
use pinapod::ZcField;
use pinapod::fixed::types::I16F16;
use pinapod::fixed::types::U24F8;

// Sizes come from the pinned release. `cargo check` evaluates these, so a
// re-export that resolved to an incompatible `fixed` fails here first.
const _: () = assert!(size_of::<I16F16>() == 4);
const _: () = assert!(size_of::<U24F8>() == 4);
const _: () =
	assert!(size_of::<pinapod::fixed::FixedI64<pinapod::fixed::types::extra::U32>>() == 8);

/// The re-exported types are the ones `PinaPod` implements `ZcField` for.
///
/// The bound is what makes this a check rather than a name lookup: reaching
/// the type through `pinapod::fixed` is exactly what puts it in the same crate
/// instance as the mapping.
fn assert_pinapod_maps_fixed_type<T: ZcField>() {}

#[allow(dead_code)]
#[derive(PinaPod)]
struct Price {
	mark: I16F16,
	quantity: U24F8,
	previous_mark: Option<I16F16>,
	// Spell the field through the re-export path too, rather than relying only
	// on the imported name.
	settlement: pinapod::fixed::types::I8F8,
}

fn main() {
	assert_pinapod_maps_fixed_type::<I16F16>();
	assert_pinapod_maps_fixed_type::<U24F8>();
	assert_pinapod_maps_fixed_type::<pinapod::fixed::types::I8F8>();
	assert_pinapod_maps_fixed_type::<pinapod::fixed::FixedU128<pinapod::fixed::types::extra::U64>>(
	);

	let mark = I16F16::from_num(-12.75);
	let quantity = U24F8::from_num(125.5);
	let previous_mark = I16F16::from_num(-13.0);
	let settlement = pinapod::fixed::types::I8F8::from_num(1.5);

	// A fixed schema occupies exactly its declared size, so this bounds check
	// is also what pins the mapped pod widths.
	let mut bytes = [0_u8; Price::SIZE];
	{
		let price = Price::read_exact_mut(&mut bytes).expect("zeroed bytes must read as a price");
		price.mark = mark.to_bits().into();
		price.quantity = quantity.to_bits().into();
		price
			.previous_mark
			.set(Some(previous_mark.to_bits().into()));
		price.settlement = settlement.to_bits().into();
	}

	assert_eq!(&bytes[..4], &mark.to_bits().to_le_bytes());
	assert_eq!(&bytes[4..8], &quantity.to_bits().to_le_bytes());

	let price = Price::read_exact(&bytes).expect("initialized bytes must read as a price");
	assert_eq!(I16F16::from_bits(price.mark.get()), mark);
	assert_eq!(U24F8::from_bits(price.quantity.get()), quantity);
	assert_eq!(
		price
			.previous_mark
			.get()
			.map(|stored| I16F16::from_bits(stored.get())),
		Some(previous_mark),
	);
	assert_eq!(
		pinapod::fixed::types::I8F8::from_bits(price.settlement.get()),
		settlement,
	);
}
