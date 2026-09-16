use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct ZeroSizedElements {
    values: pinapod::Vec<[u8; 0], 4>,
}

fn main() {}
