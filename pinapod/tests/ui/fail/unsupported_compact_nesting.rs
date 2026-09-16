use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct UnsupportedNesting {
    values: Option<pinapod::Vec<pinapod::String<8>, 4>>,
}

fn main() {}
