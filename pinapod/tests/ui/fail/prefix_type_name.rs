use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct PrefixMustBeAWidth {
    values: pinapod::PodVec<u8, 8, u16>,
}

fn main() {}
