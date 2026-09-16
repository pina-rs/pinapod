use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(crate = pinapod)]
struct InvalidFixedCapacity {
    value: pinapod::PodString<256, 1>,
}

fn main() {}
