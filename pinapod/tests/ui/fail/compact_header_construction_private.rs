use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct Journal {
    pub revision: u64,
    pub entries: pinapod::Vec<u64, 8>,
}

fn main() {
    let _ = JournalHeader {
        revision: 0_u64.into(),
        __entries_len: [0; 2],
    };
}
