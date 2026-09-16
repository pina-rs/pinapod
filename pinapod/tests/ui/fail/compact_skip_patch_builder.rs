use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct ProtectedAccount {
    #[pinapod(skip_accessor, skip_patch)]
    discriminator: [u8; 8],
    revision: u64,
    label: pinapod::String<8>,
}

fn main() {
    let _ = ProtectedAccountPatch::new().discriminator([0; 8]);
}
