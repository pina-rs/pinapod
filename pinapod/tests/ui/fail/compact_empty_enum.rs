use pinapod::PinaPod;

// A zero-variant compact enum has no tag value, no payload grammar, and no
// representable allocation: `enum_min_size` would leave `MIN_SIZE` at
// `usize::MAX` above a zero `MAX_SIZE`. The derive must reject it with a
// focused diagnostic rather than emit that degenerate contract and let
// opaque errors surface from the generated module.
#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
#[repr(u8)]
enum EmptyCompact {}

fn main() {}
