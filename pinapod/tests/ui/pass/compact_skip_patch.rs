use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct ProtectedAccount {
    #[pinapod(skip_accessor, skip_patch)]
    discriminator: [u8; 8],
    revision: u64,
    entries: pinapod::Vec<u64, 8>,
}

fn main() {
    let entries: [pinapod::pod::PodU64; 2] = [1_u64.into(), 2_u64.into()];
    let _ = ProtectedAccountPatch::new()
        .revision(3)
        .replace_entries(&entries);
}
