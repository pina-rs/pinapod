//! Fixtures shared by the Criterion API comparison and the instruction-count
//! gate.
//!
//! The two harnesses answer different questions — wall-clock standing against
//! the pinned historical implementations, and instruction counts against the
//! committed baseline — so they must run the *same* workloads over the *same*
//! bytes. Both bench roots include this module through a `#[path]` attribute,
//! which keeps one fixture definition per workload instead of two drifting
//! copies.
//!
//! This module is a benchmark fixture rather than a published surface, so its
//! schemas stay undocumented.

/// The current checkout's fixed and compact fixtures, wire-identical to the
/// pinned previous and upstream ones defined in `api_comparison.rs`.
pub mod current {
	use std::mem::size_of;

	#[allow(dead_code)]
	#[derive(pinapod::PinaPod)]
	pub struct Fixed {
		pub authority: [u8; 32],
		pub amount: u64,
		pub revision: u32,
		pub active: bool,
	}

	#[allow(dead_code)]
	#[derive(pinapod::PinaPod)]
	#[pinapod(compact)]
	pub struct Compact {
		pub sequence: u64,
		pub revision: u32,
		pub label: pinapod::String<64>,
		pub values: pinapod::Vec<u64, 16>,
	}

	pub struct FixedView<'data>(&'data FixedZc);

	pub struct CompactView<'data>(CompactRef<'data>);

	pub fn fixed_size() -> usize {
		Fixed::SIZE
	}

	pub fn compact_header_size() -> usize {
		<Compact as pinapod::PinaPodCompact>::HEADER_SIZE
	}

	pub fn view_sizes() -> (usize, usize) {
		(size_of::<CompactRef<'_>>(), size_of::<CompactPatch<'_>>())
	}

	pub fn write_fixed(data: &mut [u8]) {
		let record = Fixed::read_exact_mut(data).unwrap();

		record.authority = [0xA5; 32];
		record.amount = 1_000_000u64.into();
		record.revision = 42u32.into();
		record.active = true.into();
	}

	pub fn initialize_fixed(data: &mut [u8]) {
		Fixed::initialize(data, |record| {
			record.authority = [0xA5; 32];
			record.amount = 1_000_000u64.into();
			record.revision = 42u32.into();
			record.active = true.into();

			Ok(())
		})
		.unwrap();
	}

	/// Mutates a valid value with values derived from `base`.
	///
	/// The instruction-count gate writes runtime-derived values because the
	/// constant-valued writer lets LLVM fold the whole operation into a
	/// constant materialization, which measures nothing.
	pub fn write_fixed_from(data: &mut [u8], base: u64) {
		let record = Fixed::read_exact_mut(data).unwrap();

		record.authority = [(base % 256) as u8; 32];
		record.amount = base.into();
		record.revision = (base as u32).into();
		record.active = base.is_multiple_of(2).into();
	}

	/// Initializes a destination with values derived from `base`, for the same
	/// reason as [`write_fixed_from`](self::write_fixed_from).
	pub fn initialize_fixed_from(data: &mut [u8], base: u64) {
		Fixed::initialize(data, |record| {
			record.authority = [(base % 256) as u8; 32];
			record.amount = base.into();
			record.revision = (base as u32).into();
			record.active = base.is_multiple_of(2).into();

			Ok(())
		})
		.unwrap();
	}

	pub fn parse_fixed(data: &[u8]) -> FixedView<'_> {
		FixedView(Fixed::read_exact(data).unwrap())
	}

	pub fn validate_fixed(data: &[u8]) {
		Fixed::validate_exact(data).unwrap();
	}

	impl FixedView<'_> {
		pub fn read(&self) -> (u8, u64, u32, bool) {
			(
				self.0.authority[0],
				self.0.amount.get(),
				self.0.revision.get(),
				self.0.active.get(),
			)
		}
	}

	pub fn write_compact(data: &mut [u8], label: &str, value_count: usize, base: u64) -> usize {
		let values: [pinapod::pod::PodU64; 16] =
			core::array::from_fn(|index| (base + index as u64).into());
		let patch = CompactPatch::new()
			.sequence(42u64)
			.revision(7u32)
			.label(label)
			.replace_values(&values[..value_count]);

		Compact::update(data, &patch).unwrap()
	}

	pub fn parse_compact(data: &[u8]) -> CompactView<'_> {
		CompactView(Compact::read_prefix(data).unwrap())
	}

	pub fn validate_compact(data: &[u8]) {
		<Compact as pinapod::PinaPodCompact>::validate(data).unwrap();
	}

	impl CompactView<'_> {
		pub fn access(&self, last: usize) -> (usize, u64, u64) {
			let label = self.0.label();
			let values = self.0.values();

			(label.len(), values[0].get(), values[last].get())
		}
	}
}

/// A six-tail compact schema guarding reader and update cost against the
/// number of tail fields rather than the number of elements.
pub mod many_tail {
	#[allow(dead_code)]
	#[derive(pinapod::PinaPod)]
	#[pinapod(compact)]
	pub struct Wide {
		pub sequence: u64,
		pub label_a: pinapod::String<16>,
		pub values_a: pinapod::Vec<u32, 8>,
		pub label_b: pinapod::String<16>,
		pub values_b: pinapod::Vec<u32, 8>,
		pub label_c: pinapod::String<16>,
		pub note: Option<pinapod::String<16>>,
	}

	const _: () = assert!(<Wide as pinapod::PinaPodCompact>::HEADER_SIZE == 16);
	const _: () = assert!(<Wide as pinapod::PinaPodCompact>::MIN_SIZE == 16);
	const _: () = assert!(<Wide as pinapod::PinaPodCompact>::MAX_SIZE == 145);
	const _: () = assert!(<Wide as pinapod::PinaPodCompact>::TAIL_ALIGNMENT == 1);

	const LABEL: &str = "aaaaaaaaaaaaaaaa";
	const VALUES: [u32; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
	const NOTE: &str = "nnnnnnnnnnnnnnnn";
	pub const ENCODED_LEN: usize = 145;
	pub const STORAGE_LEN: usize = 145;

	pub fn write(data: &mut [u8]) -> usize {
		let values: [pinapod::pod::PodU32; 8] = [
			pinapod::pod::PodU32::from(VALUES[0]),
			pinapod::pod::PodU32::from(VALUES[1]),
			pinapod::pod::PodU32::from(VALUES[2]),
			pinapod::pod::PodU32::from(VALUES[3]),
			pinapod::pod::PodU32::from(VALUES[4]),
			pinapod::pod::PodU32::from(VALUES[5]),
			pinapod::pod::PodU32::from(VALUES[6]),
			pinapod::pod::PodU32::from(VALUES[7]),
		];
		let patch = WidePatch::new()
			.sequence(1)
			.label_a(LABEL)
			.replace_values_a(&values)
			.label_b(LABEL)
			.replace_values_b(&values)
			.label_c(LABEL)
			.note(Some(NOTE));
		Wide::initialize(data, &patch)
			.unwrap_or_else(|error| panic!("wide fixture must initialize: {error}"))
	}

	/// Parses the fixture once. Benchmarks receive the prebuilt view so they
	/// measure accessor cost without validation or offset-walk work.
	pub fn view(data: &[u8]) -> WideRef<'_> {
		Wide::read_prefix(data).unwrap_or_else(|error| panic!("wide fixture must parse: {error}"))
	}

	pub fn parse(data: &[u8]) -> usize {
		Wide::read_prefix(data)
			.unwrap_or_else(|error| panic!("wide fixture must parse: {error}"))
			.encoded_len()
	}

	pub fn access_all(view: &WideRef<'_>) -> usize {
		std::hint::black_box((
			view.sequence,
			view.label_a(),
			view.values_a(),
			view.label_b(),
			view.values_b(),
			view.label_c(),
			view.note(),
		));
		view.encoded_len()
	}

	pub fn access_last<'a>(view: &'a WideRef<'a>) -> Option<&'a str> {
		view.note()
	}
}

/// A compact schema with one vector tail sized toward large accounts, so tail
/// validation and relocation are measured at allocations the 207-byte fixture
/// cannot reach. The capacity is generic: the same schema serves the 8 KiB,
/// 64 KiB, and 256 KiB active-tail sizes.
pub mod large {
	#[allow(dead_code)]
	#[derive(pinapod::PinaPod)]
	#[pinapod(compact)]
	pub struct Large<const CAPACITY: usize> {
		pub sequence: u64,
		pub payload: pinapod::Vec<u64, CAPACITY>,
	}

	pub const HEADER_SIZE: usize = 10;

	const _: () = assert!(<Large<8> as pinapod::PinaPodCompact>::HEADER_SIZE == HEADER_SIZE);
	const _: () = assert!(<Large<8> as pinapod::PinaPodCompact>::MAX_SIZE == HEADER_SIZE + 8 * 8);
	// The only tail is a `u64` vector, so valid growth granularity is the
	// element size rather than one byte.
	const _: () = assert!(<Large<8> as pinapod::PinaPodCompact>::TAIL_ALIGNMENT == 8);

	pub const LABEL_INDEX: usize = 0;

	pub fn storage_len(capacity: usize) -> usize {
		HEADER_SIZE + capacity * 8
	}

	pub fn write<const CAPACITY: usize>(data: &mut [u8], count: usize, base: u64) -> usize {
		let values: [pinapod::pod::PodU64; CAPACITY] =
			core::array::from_fn(|index| (base + index as u64).into());
		let patch = LargePatch::<CAPACITY>::new()
			.sequence(base)
			.replace_payload(&values[..count]);
		Large::<CAPACITY>::update(data, &patch)
			.unwrap_or_else(|error| panic!("large fixture must update: {error}"))
	}

	pub fn parse<const CAPACITY: usize>(data: &[u8]) -> usize {
		Large::<CAPACITY>::read_prefix(data)
			.unwrap_or_else(|error| panic!("large fixture must parse: {error}"))
			.encoded_len()
	}

	pub fn validate<const CAPACITY: usize>(data: &[u8]) {
		<Large<CAPACITY> as pinapod::PinaPodCompact>::validate(data)
			.unwrap_or_else(|error| panic!("large fixture must validate: {error}"));
	}

	pub fn access<const CAPACITY: usize>(view: &LargeRef<'_, CAPACITY>, last: usize) -> (u64, u64) {
		let payload = view.payload();

		(payload[LABEL_INDEX].get(), payload[last].get())
	}
}

/// An unchecked `bytemuck` cast over the same 45 fixed wire bytes, so the
/// Criterion comparison can price `PinaPod`'s validating read against the
/// ecosystem's cast-without-validation floor. The cast type mirrors the pod
/// layout — byte arrays, alignment one, no padding — exactly like the
/// generated `FixedZc`.
pub mod unchecked {
	use std::mem::align_of;
	use std::mem::size_of;

	use bytemuck::AnyBitPattern;

	#[repr(C)]
	#[derive(Clone, Copy, AnyBitPattern)]
	pub struct FixedCast {
		pub authority: [u8; 32],
		pub amount: [u8; 8],
		pub revision: [u8; 4],
		pub active: [u8; 1],
	}

	const _: () = assert!(size_of::<FixedCast>() == 45);
	const _: () = assert!(align_of::<FixedCast>() == 1);

	pub fn parse(data: &[u8]) -> (u8, u64, u32, bool) {
		let value: &FixedCast = bytemuck::from_bytes(data);

		(
			value.authority[0],
			u64::from_le_bytes(value.amount),
			u32::from_le_bytes(value.revision),
			value.active[0] != 0,
		)
	}
}
