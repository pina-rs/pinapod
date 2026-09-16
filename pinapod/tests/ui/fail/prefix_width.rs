use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct InvalidPrefixWidth {
    values: pinapod::PodVec<u64, 8, 3>,
}

fn main() {}
