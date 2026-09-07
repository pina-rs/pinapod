//! Reproducible current and previous `PinaPod` plus upstream `ZeroPod` benchmarks.
//!
//! The three fixture modules derive separate but wire-identical schemas. The
//! setup assertion makes a benchmark fail rather than compare different bytes.

use {
    criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, Throughput},
    stats_alloc::{Region, StatsAlloc, INSTRUMENTED_SYSTEM},
    std::alloc::System,
};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const FIXED_SIZE: usize = 45;
const COMPACT_HEADER_SIZE: usize = 15;
const COMPACT_CAPACITY: usize = 207;
const SMALL_LABEL: &str = "small";
const SMALL_UPDATE_LABEL: &str = "tiny!";
const MAX_LABEL: &str = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
const SMALL_VALUE_COUNT: usize = 2;
const MAX_VALUE_COUNT: usize = 16;
const SMALL_COMPACT_SIZE: usize = COMPACT_HEADER_SIZE + SMALL_LABEL.len() + SMALL_VALUE_COUNT * 8;
const MAX_COMPACT_SIZE: usize = COMPACT_HEADER_SIZE + MAX_LABEL.len() + MAX_VALUE_COUNT * 8;

struct FixtureSet {
    fixed: [u8; FIXED_SIZE],
    small: [u8; COMPACT_CAPACITY],
    small_len: usize,
    max: [u8; COMPACT_CAPACITY],
    max_len: usize,
}

struct Fixtures {
    current: FixtureSet,
    previous: FixtureSet,
    upstream: FixtureSet,
}

// PinaPod's current derive emits a private generated module that imports the
// schema name. Suppress that expansion-only lint until the derive itself owns
// the suppression; benchmark source imports remain linted normally.
#[allow(unused_imports)]
mod current {
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

mod previous {
    use {pinapod_previous as pinapod, std::mem::size_of};

    #[allow(dead_code)]
    #[derive(pinapod::ZeroPod)]
    pub struct Fixed {
        pub authority: [u8; 32],
        pub amount: u64,
        pub revision: u32,
        pub active: bool,
    }

    #[allow(dead_code)]
    #[derive(pinapod::ZeroPod)]
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
        <Fixed as pinapod::ZeroPodFixed>::SIZE
    }

    pub fn compact_header_size() -> usize {
        <Compact as pinapod::ZeroPodCompact>::HEADER_SIZE
    }

    pub fn view_sizes() -> (usize, usize) {
        (size_of::<CompactRef<'_>>(), size_of::<CompactMut<'_>>())
    }

    pub fn write_fixed(data: &mut [u8]) {
        let record = <Fixed as pinapod::ZeroPodFixed>::from_bytes_mut(data).unwrap();

        record.authority = [0xA5; 32];
        record.amount = 1_000_000u64.into();
        record.revision = 42u32.into();
        record.active = true.into();
    }

    pub fn initialize_fixed(data: &mut [u8]) {
        write_fixed(data);
    }

    pub fn parse_fixed(data: &[u8]) -> FixedView<'_> {
        FixedView(<Fixed as pinapod::ZeroPodFixed>::from_bytes(data).unwrap())
    }

    pub fn validate_fixed(data: &[u8]) {
        <Fixed as pinapod::ZeroPodFixed>::validate(data).unwrap();
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
        let mut record = CompactMut::new(data).unwrap();

        record.sequence = 42u64.into();
        record.revision = 7u32.into();
        record.set_label(label).unwrap();
        record.set_values(&values[..value_count]).unwrap();

        record.commit().unwrap()
    }

    pub fn parse_compact(data: &[u8]) -> CompactView<'_> {
        CompactView(CompactRef::new(data).unwrap())
    }

    pub fn validate_compact(data: &[u8]) {
        <Compact as pinapod::ZeroPodCompact>::validate(data).unwrap();
    }

    impl CompactView<'_> {
        pub fn access(&self, last: usize) -> (usize, u64, u64) {
            let label = self.0.label();
            let values = self.0.values();

            (label.len(), values[0].get(), values[last].get())
        }
    }
}

mod upstream {
    use std::mem::size_of;

    #[allow(dead_code)]
    #[derive(zeropod::ZeroPod)]
    pub struct Fixed {
        pub authority: [u8; 32],
        pub amount: u64,
        pub revision: u32,
        pub active: bool,
    }

    #[allow(dead_code)]
    #[derive(zeropod::ZeroPod)]
    #[zeropod(compact)]
    pub struct Compact {
        pub sequence: u64,
        pub revision: u32,
        pub label: zeropod::String<64>,
        pub values: zeropod::Vec<u64, 16>,
    }

    pub struct FixedView<'data>(&'data FixedZc);

    pub struct CompactView<'data>(CompactRef<'data>);

    pub fn fixed_size() -> usize {
        <Fixed as zeropod::ZeroPodFixed>::SIZE
    }

    pub fn compact_header_size() -> usize {
        <Compact as zeropod::ZeroPodCompact>::HEADER_SIZE
    }

    pub fn view_sizes() -> (usize, usize) {
        (size_of::<CompactRef<'_>>(), size_of::<CompactMut<'_>>())
    }

    pub fn write_fixed(data: &mut [u8]) {
        let record = <Fixed as zeropod::ZeroPodFixed>::from_bytes_mut(data).unwrap();

        record.authority = [0xA5; 32];
        record.amount = 1_000_000u64.into();
        record.revision = 42u32.into();
        record.active = true.into();
    }

    pub fn initialize_fixed(data: &mut [u8]) {
        write_fixed(data);
    }

    pub fn parse_fixed(data: &[u8]) -> FixedView<'_> {
        FixedView(<Fixed as zeropod::ZeroPodFixed>::from_bytes(data).unwrap())
    }

    pub fn validate_fixed(data: &[u8]) {
        <Fixed as zeropod::ZeroPodFixed>::validate(data).unwrap();
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
        let values: [zeropod::pod::PodU64; 16] =
            core::array::from_fn(|index| (base + index as u64).into());
        let mut record = CompactMut::new(data).unwrap();

        record.sequence = 42u64.into();
        record.revision = 7u32.into();
        record.set_label(label).unwrap();
        record.set_values(&values[..value_count]).unwrap();

        record.commit().unwrap()
    }

    pub fn parse_compact(data: &[u8]) -> CompactView<'_> {
        CompactView(CompactRef::new(data).unwrap())
    }

    pub fn validate_compact(data: &[u8]) {
        <Compact as zeropod::ZeroPodCompact>::validate(data).unwrap();
    }

    impl CompactView<'_> {
        pub fn access(&self, last: usize) -> (usize, u64, u64) {
            let label = self.0.label();
            let values = self.0.values();

            (label.len(), values[0].get(), values[last].get())
        }
    }
}

fn fixtures() -> Fixtures {
    let mut current_fixed = [0u8; FIXED_SIZE];
    let mut previous_fixed = [0u8; FIXED_SIZE];
    let mut upstream_fixed = [0u8; FIXED_SIZE];
    let mut current_small = [0u8; COMPACT_CAPACITY];
    let mut previous_small = [0u8; COMPACT_CAPACITY];
    let mut upstream_small = [0u8; COMPACT_CAPACITY];
    let mut current_max = [0u8; COMPACT_CAPACITY];
    let mut previous_max = [0u8; COMPACT_CAPACITY];
    let mut upstream_max = [0u8; COMPACT_CAPACITY];

    current::write_fixed(&mut current_fixed);
    previous::write_fixed(&mut previous_fixed);
    upstream::write_fixed(&mut upstream_fixed);

    let current_small_len =
        current::write_compact(&mut current_small, SMALL_LABEL, SMALL_VALUE_COUNT, 1);
    let previous_small_len =
        previous::write_compact(&mut previous_small, SMALL_LABEL, SMALL_VALUE_COUNT, 1);
    let upstream_small_len =
        upstream::write_compact(&mut upstream_small, SMALL_LABEL, SMALL_VALUE_COUNT, 1);
    let current_max_len = current::write_compact(&mut current_max, MAX_LABEL, MAX_VALUE_COUNT, 1);
    let previous_max_len =
        previous::write_compact(&mut previous_max, MAX_LABEL, MAX_VALUE_COUNT, 1);
    let upstream_max_len =
        upstream::write_compact(&mut upstream_max, MAX_LABEL, MAX_VALUE_COUNT, 1);

    Fixtures {
        current: FixtureSet {
            fixed: current_fixed,
            small: current_small,
            small_len: current_small_len,
            max: current_max,
            max_len: current_max_len,
        },
        previous: FixtureSet {
            fixed: previous_fixed,
            small: previous_small,
            small_len: previous_small_len,
            max: previous_max,
            max_len: previous_max_len,
        },
        upstream: FixtureSet {
            fixed: upstream_fixed,
            small: upstream_small,
            small_len: upstream_small_len,
            max: upstream_max,
            max_len: upstream_max_len,
        },
    }
}

fn assert_wire_parity(fixtures: &Fixtures) {
    assert_eq!(current::fixed_size(), FIXED_SIZE);
    assert_eq!(previous::fixed_size(), FIXED_SIZE);
    assert_eq!(upstream::fixed_size(), FIXED_SIZE);
    assert_eq!(current::compact_header_size(), COMPACT_HEADER_SIZE);
    assert_eq!(previous::compact_header_size(), COMPACT_HEADER_SIZE);
    assert_eq!(upstream::compact_header_size(), COMPACT_HEADER_SIZE);

    for fixture in [&fixtures.current, &fixtures.previous, &fixtures.upstream] {
        assert_eq!(fixture.small_len, SMALL_COMPACT_SIZE);
        assert_eq!(fixture.max_len, MAX_COMPACT_SIZE);
    }

    assert_eq!(fixtures.current.fixed, fixtures.previous.fixed);
    assert_eq!(fixtures.current.fixed, fixtures.upstream.fixed);
    assert_eq!(
        &fixtures.current.small[..fixtures.current.small_len],
        &fixtures.previous.small[..fixtures.previous.small_len]
    );
    assert_eq!(
        &fixtures.current.small[..fixtures.current.small_len],
        &fixtures.upstream.small[..fixtures.upstream.small_len]
    );
    assert_eq!(
        &fixtures.current.max[..fixtures.current.max_len],
        &fixtures.previous.max[..fixtures.previous.max_len]
    );
    assert_eq!(
        &fixtures.current.max[..fixtures.current.max_len],
        &fixtures.upstream.max[..fixtures.upstream.max_len]
    );
}

fn report_allocation(name: &str, operation: impl FnOnce()) {
    let region = Region::new(&INSTRUMENTED_SYSTEM);

    operation();

    let stats = region.change();
    eprintln!(
        "api comparison allocation: {name}: allocations={}, bytes={}",
        stats.allocations, stats.bytes_allocated
    );
}

fn report_facts(fixtures: &Fixtures) {
    let current_sizes = current::view_sizes();
    let previous_sizes = previous::view_sizes();
    let upstream_sizes = upstream::view_sizes();

    eprintln!(
        "api comparison layout: fixed={FIXED_SIZE}B, compact header={COMPACT_HEADER_SIZE}B, \
         compact small={}B, compact max={}B",
        fixtures.current.small_len, fixtures.current.max_len,
    );
    eprintln!(
        "api comparison writer sizes: current ref={}B/patch={}B, previous ref={}B/mut={}B, \
         upstream ref={}B/mut={}B",
        current_sizes.0,
        current_sizes.1,
        previous_sizes.0,
        previous_sizes.1,
        upstream_sizes.0,
        upstream_sizes.1,
    );

    report_allocation("current fixed write", || {
        let mut data = [0u8; FIXED_SIZE];
        current::write_fixed(&mut data);
        black_box(data);
    });
    report_allocation("previous fixed write", || {
        let mut data = [0u8; FIXED_SIZE];
        previous::write_fixed(&mut data);
        black_box(data);
    });
    report_allocation("upstream fixed write", || {
        let mut data = [0u8; FIXED_SIZE];
        upstream::write_fixed(&mut data);
        black_box(data);
    });
    report_allocation("current fixed initialize", || {
        let mut data = [0u8; FIXED_SIZE];
        current::initialize_fixed(&mut data);
        black_box(data);
    });
    report_allocation("previous fixed initialize equivalent", || {
        let mut data = [0u8; FIXED_SIZE];
        previous::initialize_fixed(&mut data);
        black_box(data);
    });
    report_allocation("upstream fixed initialize equivalent", || {
        let mut data = [0u8; FIXED_SIZE];
        upstream::initialize_fixed(&mut data);
        black_box(data);
    });

    for (name, write) in [
        (
            "current compact small update",
            current::write_compact as fn(&mut [u8], &str, usize, u64) -> usize,
        ),
        (
            "previous compact small update",
            previous::write_compact as fn(&mut [u8], &str, usize, u64) -> usize,
        ),
        (
            "upstream compact small update",
            upstream::write_compact as fn(&mut [u8], &str, usize, u64) -> usize,
        ),
    ] {
        report_allocation(name, || {
            let mut data = fixtures.current.small;
            let used = write(&mut data, SMALL_UPDATE_LABEL, SMALL_VALUE_COUNT, 10_000);
            black_box((data, used));
        });
    }

    for (name, write) in [
        (
            "current compact max update",
            current::write_compact as fn(&mut [u8], &str, usize, u64) -> usize,
        ),
        (
            "previous compact max update",
            previous::write_compact as fn(&mut [u8], &str, usize, u64) -> usize,
        ),
        (
            "upstream compact max update",
            upstream::write_compact as fn(&mut [u8], &str, usize, u64) -> usize,
        ),
    ] {
        report_allocation(name, || {
            let mut data = fixtures.current.small;
            let used = write(&mut data, MAX_LABEL, MAX_VALUE_COUNT, 10_000);
            black_box((data, used));
        });
    }
}

fn bench_fixed(c: &mut Criterion, fixtures: &Fixtures) {
    let mut parse = c.benchmark_group("fixed/parse");
    parse.throughput(Throughput::Bytes(FIXED_SIZE as u64));
    parse.bench_function("pinapod-current", |bench| {
        bench.iter(|| black_box(current::parse_fixed(black_box(&fixtures.current.fixed))));
    });
    parse.bench_function("pinapod-previous-71ad8be", |bench| {
        bench.iter(|| black_box(previous::parse_fixed(black_box(&fixtures.previous.fixed))));
    });
    parse.bench_function("zeropod-upstream-78e6e5f", |bench| {
        bench.iter(|| black_box(upstream::parse_fixed(black_box(&fixtures.upstream.fixed))));
    });
    parse.finish();

    let mut validate = c.benchmark_group("fixed/validate");
    validate.throughput(Throughput::Bytes(FIXED_SIZE as u64));
    validate.bench_function("pinapod-current", |bench| {
        bench.iter(|| current::validate_fixed(black_box(&fixtures.current.fixed)));
    });
    validate.bench_function("pinapod-previous-71ad8be", |bench| {
        bench.iter(|| previous::validate_fixed(black_box(&fixtures.previous.fixed)));
    });
    validate.bench_function("zeropod-upstream-78e6e5f", |bench| {
        bench.iter(|| upstream::validate_fixed(black_box(&fixtures.upstream.fixed)));
    });
    validate.finish();

    let current_view = current::parse_fixed(&fixtures.current.fixed);
    let previous_view = previous::parse_fixed(&fixtures.previous.fixed);
    let upstream_view = upstream::parse_fixed(&fixtures.upstream.fixed);
    let mut read = c.benchmark_group("fixed/read");
    read.throughput(Throughput::Bytes(FIXED_SIZE as u64));
    read.bench_function("pinapod-current", |bench| {
        bench.iter(|| black_box(current_view.read()));
    });
    read.bench_function("pinapod-previous-71ad8be", |bench| {
        bench.iter(|| black_box(previous_view.read()));
    });
    read.bench_function("zeropod-upstream-78e6e5f", |bench| {
        bench.iter(|| black_box(upstream_view.read()));
    });
    read.finish();

    let mut write = c.benchmark_group("fixed/write");
    write.throughput(Throughput::Bytes(FIXED_SIZE as u64));
    write.bench_function("pinapod-current", |bench| {
        bench.iter_batched_ref(
            || [0u8; FIXED_SIZE],
            |data| {
                current::write_fixed(data);
                black_box(data);
            },
            BatchSize::SmallInput,
        );
    });
    write.bench_function("pinapod-previous-71ad8be", |bench| {
        bench.iter_batched_ref(
            || [0u8; FIXED_SIZE],
            |data| {
                previous::write_fixed(data);
                black_box(data);
            },
            BatchSize::SmallInput,
        );
    });
    write.bench_function("zeropod-upstream-78e6e5f", |bench| {
        bench.iter_batched_ref(
            || [0u8; FIXED_SIZE],
            |data| {
                upstream::write_fixed(data);
                black_box(data);
            },
            BatchSize::SmallInput,
        );
    });
    write.finish();

    let mut initialize = c.benchmark_group("fixed/initialize");
    initialize.throughput(Throughput::Bytes(FIXED_SIZE as u64));
    initialize.bench_function("pinapod-current", |bench| {
        bench.iter_batched_ref(
            || [0u8; FIXED_SIZE],
            |data| {
                current::initialize_fixed(data);
                black_box(data);
            },
            BatchSize::SmallInput,
        );
    });
    initialize.bench_function("pinapod-previous-71ad8be", |bench| {
        bench.iter_batched_ref(
            || [0u8; FIXED_SIZE],
            |data| {
                previous::initialize_fixed(data);
                black_box(data);
            },
            BatchSize::SmallInput,
        );
    });
    initialize.bench_function("zeropod-upstream-78e6e5f", |bench| {
        bench.iter_batched_ref(
            || [0u8; FIXED_SIZE],
            |data| {
                upstream::initialize_fixed(data);
                black_box(data);
            },
            BatchSize::SmallInput,
        );
    });
    initialize.finish();
}

struct CompactCase<'data> {
    name: &'data str,
    value_count: usize,
    next_label: &'data str,
    current_data: &'data [u8],
    previous_data: &'data [u8],
    upstream_data: &'data [u8],
    current_seed: [u8; COMPACT_CAPACITY],
    previous_seed: [u8; COMPACT_CAPACITY],
    upstream_seed: [u8; COMPACT_CAPACITY],
}

fn bench_compact_case(c: &mut Criterion, case: &CompactCase<'_>) {
    let case_bytes = case.current_data.len() as u64;
    let current_view = current::parse_compact(case.current_data);
    let previous_view = previous::parse_compact(case.previous_data);
    let upstream_view = upstream::parse_compact(case.upstream_data);

    let mut parse = c.benchmark_group(format!("compact/{}/parse", case.name));
    parse.throughput(Throughput::Bytes(case_bytes));
    parse.bench_function("pinapod-current", |bench| {
        bench.iter(|| black_box(current::parse_compact(black_box(case.current_data))));
    });
    parse.bench_function("pinapod-previous-71ad8be", |bench| {
        bench.iter(|| black_box(previous::parse_compact(black_box(case.previous_data))));
    });
    parse.bench_function("zeropod-upstream-78e6e5f", |bench| {
        bench.iter(|| black_box(upstream::parse_compact(black_box(case.upstream_data))));
    });
    parse.finish();

    let mut validate = c.benchmark_group(format!("compact/{}/validate", case.name));
    validate.throughput(Throughput::Bytes(case_bytes));
    validate.bench_function("pinapod-current", |bench| {
        bench.iter(|| current::validate_compact(black_box(case.current_data)));
    });
    validate.bench_function("pinapod-previous-71ad8be", |bench| {
        bench.iter(|| previous::validate_compact(black_box(case.previous_data)));
    });
    validate.bench_function("zeropod-upstream-78e6e5f", |bench| {
        bench.iter(|| upstream::validate_compact(black_box(case.upstream_data)));
    });
    validate.finish();

    let mut access = c.benchmark_group(format!("compact/{}/access", case.name));
    access.throughput(Throughput::Bytes(case_bytes));
    access.bench_function("pinapod-current", |bench| {
        bench.iter(|| black_box(current_view.access(case.value_count - 1)));
    });
    access.bench_function("pinapod-previous-71ad8be", |bench| {
        bench.iter(|| black_box(previous_view.access(case.value_count - 1)));
    });
    access.bench_function("zeropod-upstream-78e6e5f", |bench| {
        bench.iter(|| black_box(upstream_view.access(case.value_count - 1)));
    });
    access.finish();

    let mut update = c.benchmark_group(format!("compact/{}/update", case.name));
    update.throughput(Throughput::Bytes(case_bytes));
    update.bench_function("pinapod-current", |bench| {
        bench.iter_batched_ref(
            || case.current_seed,
            |data| {
                let used = current::write_compact(data, case.next_label, case.value_count, 10_000);
                black_box((data, used));
            },
            BatchSize::SmallInput,
        );
    });
    update.bench_function("pinapod-previous-71ad8be", |bench| {
        bench.iter_batched_ref(
            || case.previous_seed,
            |data| {
                let used = previous::write_compact(data, case.next_label, case.value_count, 10_000);
                black_box((data, used));
            },
            BatchSize::SmallInput,
        );
    });
    update.bench_function("zeropod-upstream-78e6e5f", |bench| {
        bench.iter_batched_ref(
            || case.upstream_seed,
            |data| {
                let used = upstream::write_compact(data, case.next_label, case.value_count, 10_000);
                black_box((data, used));
            },
            BatchSize::SmallInput,
        );
    });
    update.finish();
}

fn bench_compact_tail_scaling(c: &mut Criterion) {
    for value_count in [1, 4, 8, 16] {
        let mut current_data = [0u8; COMPACT_CAPACITY];
        let mut previous_data = [0u8; COMPACT_CAPACITY];
        let mut upstream_data = [0u8; COMPACT_CAPACITY];

        let current_len = current::write_compact(&mut current_data, SMALL_LABEL, value_count, 1);
        let previous_len = previous::write_compact(&mut previous_data, SMALL_LABEL, value_count, 1);
        let upstream_len = upstream::write_compact(&mut upstream_data, SMALL_LABEL, value_count, 1);

        assert_eq!(
            current_len,
            COMPACT_HEADER_SIZE + SMALL_LABEL.len() + value_count * 8
        );
        assert_eq!(current_len, previous_len);
        assert_eq!(current_len, upstream_len);
        assert_eq!(&current_data[..current_len], &previous_data[..previous_len]);
        assert_eq!(&current_data[..current_len], &upstream_data[..upstream_len]);

        let name = format!("tails/{value_count}");
        bench_compact_case(
            c,
            &CompactCase {
                name: &name,
                value_count,
                next_label: SMALL_UPDATE_LABEL,
                current_data: &current_data[..current_len],
                previous_data: &previous_data[..previous_len],
                upstream_data: &upstream_data[..upstream_len],
                current_seed: current_data,
                previous_seed: previous_data,
                upstream_seed: upstream_data,
            },
        );
    }
}

fn api_comparison(c: &mut Criterion) {
    let fixtures = fixtures();

    assert_wire_parity(&fixtures);
    report_facts(&fixtures);
    bench_fixed(c, &fixtures);
    bench_compact_case(
        c,
        &CompactCase {
            name: "small",
            value_count: SMALL_VALUE_COUNT,
            next_label: SMALL_UPDATE_LABEL,
            current_data: &fixtures.current.small[..fixtures.current.small_len],
            previous_data: &fixtures.previous.small[..fixtures.previous.small_len],
            upstream_data: &fixtures.upstream.small[..fixtures.upstream.small_len],
            current_seed: fixtures.current.small,
            previous_seed: fixtures.previous.small,
            upstream_seed: fixtures.upstream.small,
        },
    );
    bench_compact_case(
        c,
        &CompactCase {
            name: "max",
            value_count: MAX_VALUE_COUNT,
            next_label: MAX_LABEL,
            current_data: &fixtures.current.max[..fixtures.current.max_len],
            previous_data: &fixtures.previous.max[..fixtures.previous.max_len],
            upstream_data: &fixtures.upstream.max[..fixtures.upstream.max_len],
            current_seed: fixtures.current.small,
            previous_seed: fixtures.previous.small,
            upstream_seed: fixtures.upstream.small,
        },
    );
    bench_compact_tail_scaling(c);
}

criterion_group!(benches, api_comparison);
criterion_main!(benches);
