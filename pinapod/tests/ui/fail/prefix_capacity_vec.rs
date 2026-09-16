use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct InvalidCapacity {
    values: pinapod::PodVec<u8, 256, 1>,
}

fn main() {}
