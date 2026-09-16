use pinapod::PinaPod;

const LAST: u8 = 9;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
#[repr(u8)]
enum CompactKind {
    First = 1,
    Second = Self::First as u8 + 1,
    Last = LAST,
}

fn main() {
    let _ = CompactKind::Second;
}
