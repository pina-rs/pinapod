use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct OptionArrayNestedPrefixWidth {
	values: Option<[pinapod::PodVec<u8, 8, 0>; 2]>,
}

fn main() {}
