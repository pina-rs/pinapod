use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(crate = pinapod)]
struct InvalidFixedCapacity {
    values: pinapod::PodVec<u8, 256, 1>,
}

fn main() {}
