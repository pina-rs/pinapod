//! Instruction-count gates for every `PinaPod` hot path.
//!
//! The Criterion harness in `api_comparison.rs` answers "is `PinaPod` fast
//! against the pinned historical implementations?" on wall-clock hardware.
//! This harness answers a different question with a different instrument:
//! "did this pull request change the instruction count of a hot path?"
//!
//! Callgrind counts retired instructions, which are a property of the binary
//! rather than the machine, so a comparison against the committed baseline in
//! `pinapod/benches/iai-baseline.json` is exact on any Linux runner and no
//! timing noise can hide a regression. Running the gate requires Valgrind and
//! therefore Linux; the library still compiles on every other host, where
//! `cargo test` builds this target but never runs it.
//!
//! The fixtures are the shared ones in `benches/common`, so the gate measures
//! the same workloads over the same bytes as the tracked release comparison.
//! Bench bodies include their own seed construction where a fresh destination
//! is needed; that setup is deterministic, so it contributes a constant to
//! every measurement and never masks a change in the measured operation.

#![allow(
	missing_docs,
	reason = "this is a benchmark harness rather than a published surface, so its fixtures stay \
	          undocumented"
)]
// iai-callgrind's generated harness `main` calls `expect` on its own internal
// plumbing; the repository's disallow-list targets PinaPod source, not the
// framework the harness expands into this bench crate.
#![allow(
	clippy::disallowed_methods,
	reason = "the iai-callgrind harness macro uses expect internally"
)]

use std::hint::black_box;
use std::sync::LazyLock;

use iai_callgrind::library_benchmark;
use iai_callgrind::library_benchmark_group;
use iai_callgrind::main;

#[allow(
	dead_code,
	reason = "each harness uses a different subset of the shared fixtures"
)]
#[path = "common/mod.rs"]
mod common;

use common::current;
// Aliased so the `library_benchmark_group!` modules below can own the bare
// names `many_tail` and `large`, which are the benchmark ids the baseline is
// keyed by.
use common::large as large_fixture;
use common::many_tail as many_tail_fixture;

const FIXED_SIZE: usize = 45;
const COMPACT_CAPACITY: usize = 207;
const SMALL_LABEL: &str = "small";
const SMALL_UPDATE_LABEL: &str = "tiny!";
const MAX_LABEL: &str = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
const SMALL_VALUE_COUNT: usize = 2;
const MAX_VALUE_COUNT: usize = 16;
const LARGE_CAPACITY: usize = 8192;

static FIXED: LazyLock<[u8; FIXED_SIZE]> = LazyLock::new(|| {
	let mut data = [0u8; FIXED_SIZE];
	current::write_fixed(&mut data);
	data
});

static SMALL: LazyLock<[u8; COMPACT_CAPACITY]> = LazyLock::new(|| {
	let mut data = [0u8; COMPACT_CAPACITY];
	current::write_compact(&mut data, SMALL_LABEL, SMALL_VALUE_COUNT, 1);
	data
});

static MAX: LazyLock<[u8; COMPACT_CAPACITY]> = LazyLock::new(|| {
	let mut data = [0u8; COMPACT_CAPACITY];
	current::write_compact(&mut data, MAX_LABEL, MAX_VALUE_COUNT, 1);
	data
});

static WIDE: LazyLock<[u8; many_tail_fixture::STORAGE_LEN]> = LazyLock::new(|| {
	let mut data = [0u8; many_tail_fixture::STORAGE_LEN];
	many_tail_fixture::write(&mut data);
	data
});

static WIDE_VIEW: LazyLock<many_tail_fixture::WideRef<'static>> =
	LazyLock::new(|| many_tail_fixture::view(&*WIDE));

static LARGE: LazyLock<Vec<u8>> = LazyLock::new(|| {
	let mut data = vec![0u8; large_fixture::storage_len(LARGE_CAPACITY)];
	large_fixture::write::<LARGE_CAPACITY>(&mut data, LARGE_CAPACITY, 1);
	data
});

static LARGE_VIEW: LazyLock<large_fixture::LargeRef<'static, LARGE_CAPACITY>> =
	LazyLock::new(|| large_fixture::Large::<LARGE_CAPACITY>::read_prefix(&LARGE).unwrap());

/// Forces every fixture static to finish initializing before measurement.
///
/// The statics are `LazyLock`s: their first dereference constructs the fixture
/// and, for the prebuilt views, parses it. Group setup runs outside the
/// measured call, so warming here keeps construction and parsing out of the
/// instruction counts — without it, `large_access` would measure a full
/// `large_parse` inside the first `LazyLock` initialization.
fn warm_instruction_fixtures() {
	black_box(&*FIXED);
	black_box(&*SMALL);
	black_box(&*MAX);
	black_box(&*WIDE);
	black_box(&*WIDE_VIEW);
	black_box(&*LARGE);
	black_box(&*LARGE_VIEW);
}

#[library_benchmark]
fn fixed_parse() -> (u8, u64, u32, bool) {
	let view = current::parse_fixed(black_box(&*FIXED));
	black_box(view).read()
}

#[library_benchmark]
fn fixed_validate() {
	current::validate_fixed(black_box(&*FIXED));
}

#[library_benchmark]
fn fixed_read() -> (u8, u64, u32, bool) {
	black_box(current::parse_fixed(&*FIXED)).read()
}

// Write and initialize derive their values from the runtime fixture bytes:
// constant-valued writers let LLVM fold the whole operation into a constant
// materialization, which would pin the cost of a write at seven instructions.
#[library_benchmark]
fn fixed_write() -> [u8; FIXED_SIZE] {
	let base = u64::from_le_bytes(black_box(FIXED[..8].try_into().unwrap()));
	let mut data = [0u8; FIXED_SIZE];
	current::write_fixed_from(&mut data, base);
	black_box(data)
}

#[library_benchmark]
fn fixed_initialize() -> [u8; FIXED_SIZE] {
	let base = u64::from_le_bytes(black_box(FIXED[..8].try_into().unwrap()));
	let mut data = [0u8; FIXED_SIZE];
	current::initialize_fixed_from(&mut data, base);
	black_box(data)
}

#[library_benchmark]
fn compact_small_parse() -> (usize, u64, u64) {
	let view = current::parse_compact(black_box(&*SMALL));
	black_box(view).access(SMALL_VALUE_COUNT - 1)
}

#[library_benchmark]
fn compact_small_validate() {
	current::validate_compact(black_box(&*SMALL));
}

#[library_benchmark]
fn compact_small_access() -> (usize, u64, u64) {
	black_box(current::parse_compact(&*SMALL)).access(SMALL_VALUE_COUNT - 1)
}

#[library_benchmark]
fn compact_small_update() -> usize {
	let mut data = *SMALL;
	let used = current::write_compact(&mut data, SMALL_UPDATE_LABEL, SMALL_VALUE_COUNT, 10_000);
	black_box((data, used)).1
}

#[library_benchmark]
fn compact_max_parse() -> (usize, u64, u64) {
	let view = current::parse_compact(black_box(&*MAX));
	black_box(view).access(MAX_VALUE_COUNT - 1)
}

#[library_benchmark]
fn compact_max_validate() {
	current::validate_compact(black_box(&*MAX));
}

#[library_benchmark]
fn compact_max_access() -> (usize, u64, u64) {
	black_box(current::parse_compact(&*MAX)).access(MAX_VALUE_COUNT - 1)
}

#[library_benchmark]
fn compact_grow_to_max_update() -> usize {
	let mut data = *SMALL;
	let used = current::write_compact(&mut data, MAX_LABEL, MAX_VALUE_COUNT, 10_000);
	black_box((data, used)).1
}

#[library_benchmark]
fn many_tail_parse() -> usize {
	black_box(many_tail_fixture::parse(black_box(&*WIDE)))
}

#[library_benchmark]
fn many_tail_access_all() -> usize {
	many_tail_fixture::access_all(black_box(&*WIDE_VIEW))
}

#[library_benchmark]
fn many_tail_access_last() -> Option<&'static str> {
	black_box(many_tail_fixture::access_last(black_box(&*WIDE_VIEW)))
}

#[library_benchmark]
fn large_parse() -> usize {
	black_box(large_fixture::parse::<LARGE_CAPACITY>(black_box(&*LARGE)))
}

#[library_benchmark]
fn large_validate() {
	large_fixture::validate::<LARGE_CAPACITY>(black_box(&*LARGE));
}

#[library_benchmark]
fn large_access() -> (u64, u64) {
	black_box(large_fixture::access(
		black_box(&*LARGE_VIEW),
		LARGE_CAPACITY - 1,
	))
}

#[library_benchmark]
fn large_grow_to_full_update() -> usize {
	let mut data = {
		let mut seed = vec![0u8; large_fixture::storage_len(LARGE_CAPACITY)];
		large_fixture::write::<LARGE_CAPACITY>(&mut seed, LARGE_CAPACITY / 2, 1);
		seed
	};
	let used = large_fixture::write::<LARGE_CAPACITY>(&mut data, LARGE_CAPACITY, 10_000);
	black_box((data, used)).1
}

library_benchmark_group!(
	name = fixed;
setup = warm_instruction_fixtures;
	benchmarks = fixed_parse, fixed_validate, fixed_read, fixed_write, fixed_initialize
);

library_benchmark_group!(
	name = compact_small;
setup = warm_instruction_fixtures;
	benchmarks = compact_small_parse, compact_small_validate, compact_small_access,
	compact_small_update
);

library_benchmark_group!(
	name = compact_max;
setup = warm_instruction_fixtures;
	benchmarks = compact_max_parse, compact_max_validate, compact_max_access,
	compact_grow_to_max_update
);

library_benchmark_group!(
	name = many_tail;
setup = warm_instruction_fixtures;
	benchmarks = many_tail_parse, many_tail_access_all, many_tail_access_last
);

library_benchmark_group!(
	name = large;
setup = warm_instruction_fixtures;
	benchmarks = large_parse, large_validate, large_access, large_grow_to_full_update
);

main!(
	library_benchmark_groups = fixed,
	compact_small,
	compact_max,
	many_tail,
	large
);
