use pod_storage::{PinaPod, PinaPodCompact, String, Vec};

#[derive(PinaPod)]
struct FixedValue {
    count: u64,
}

#[derive(PinaPod)]
#[pinapod(compact)]
struct CompactValue {
    label: String<8>,
    values: Vec<u64, 4>,
}

fn main() {
    let fixed_bytes = [0; FixedValue::SIZE];
    let compact_bytes = [0; CompactValue::HEADER_SIZE];

    FixedValue::read_exact(&fixed_bytes).expect("fixed schema should use the renamed dependency");
    CompactValueRef::new(&compact_bytes).expect("compact schema should use the renamed dependency");
}
