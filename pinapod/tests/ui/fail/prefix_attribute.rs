use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct InvalidPrefixAttribute {
    #[pinapod(prefix = u16)]
    entries: pinapod::Vec<u64, 1024>,
}

fn main() {}
