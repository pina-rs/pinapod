use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct Journal {
    pub entries: pinapod::Vec<u64, 8>,
}

fn main() {
    let bytes = [0; Journal::HEADER_SIZE];
    let _ = JournalRef { data: &bytes };
}
