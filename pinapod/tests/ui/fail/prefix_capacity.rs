use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct InvalidCapacity {
    value: pinapod::PodString<256, 1>,
}

fn main() {}
