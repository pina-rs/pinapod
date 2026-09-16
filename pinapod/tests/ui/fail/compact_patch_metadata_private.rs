use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct Journal {
    revision: u64,
    entries: pinapod::Vec<u64, 8>,
}

fn main() {
    let mut patch = JournalPatch::new();

    patch.entries = None;
}
