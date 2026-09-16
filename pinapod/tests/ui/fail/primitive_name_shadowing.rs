use pinapod::PinaPod;

#[allow(non_camel_case_types)]
struct u16(bool);

#[derive(PinaPod)]
#[pinapod(crate = pinapod)]
struct ShadowedPrimitive {
    value: u16,
}

fn main() {}
