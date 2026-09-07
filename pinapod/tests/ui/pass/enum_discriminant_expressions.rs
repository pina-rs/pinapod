use pinapod::PinaPod;

const LAST: u8 = 9;

#[derive(PinaPod)]
#[pinapod(crate = pinapod)]
#[repr(u8)]
enum FixedKind {
    First = 1,
    Second = Self::First as u8 + 1,
    Last = LAST,
}

fn main() {
    let _ = FixedKind::Second;
}
