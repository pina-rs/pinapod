use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct PrefixWidths {
    one: pinapod::PodString<255, 1>,
    two: pinapod::PodVec<u8, 65_535, 2>,
    four: pinapod::PodString<65_536, 4>,
    eight: pinapod::PodVec<u8, 65_536, 8>,
}

fn main() {}
