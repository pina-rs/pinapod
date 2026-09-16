use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct InlineOnly {
    #[pinapod(skip_patch)]
    discriminator: [u8; 8],
    revision: u64,
}

fn main() {
    let _ = InlineOnlyPatch::new().revision(1);
}
