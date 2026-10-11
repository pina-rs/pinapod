use pinapod::PinaPod;

// A zero-variant fixed enum has no discriminant to store and no match arm to
// validate: the derive must reject it with a focused diagnostic rather than
// emit a representation for a type with no values.
#[derive(PinaPod)]
#[pinapod(crate = pinapod)]
#[repr(u8)]
enum EmptyFixed {}

fn main() {}
