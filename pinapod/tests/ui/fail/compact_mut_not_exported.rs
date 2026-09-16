use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct Journal {
    entries: pinapod::Vec<u64, 8>,
}

fn main() {
    let mut bytes = [0; Journal::HEADER_SIZE];
    let _ = __pinapod_compact_Journal::JournalMut::new(&mut bytes);
}
