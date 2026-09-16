use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct SupportedCompactFields {
    maybe_revision: Option<u64>,
    title: pinapod::String<32>,
    values: pinapod::Vec<u64, 8>,
    note: Option<pinapod::String<64>>,
    maybe_values: Option<pinapod::Vec<u16, 8>>,
    labels: pinapod::Vec<pinapod::String<12>, 4>,
    summary: pinapod::String<16>,
}

fn main() {
    let _ = SupportedCompactFieldsPatch::new().maybe_revision(Some(13_u64));
    let _ = SupportedCompactFieldsPatch::new().maybe_revision(None);
    let _ = SupportedCompactFieldsPatch::new().maybe_revision(pinapod::pod::PodOption::some(
        pinapod::pod::PodU64::from(13_u64),
    ));
}
