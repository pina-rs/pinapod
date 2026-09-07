#![allow(unsafe_code)]

use {
    core::marker::PhantomData,
    pinapod::{PinaPod, PinaPodError, ZcElem, ZcField, ZcValidate},
};

#[repr(C)]
#[derive(Clone, Copy)]
struct LifetimeMarker<'a> {
    byte: u8,
    marker: PhantomData<&'a ()>,
}

impl ZcValidate for LifetimeMarker<'_> {
    fn validate_ref(_: &Self) -> Result<(), PinaPodError> {
        Ok(())
    }
}

// SAFETY: the marker is alignment one, contains no padding, and every byte
// pattern is valid because `PhantomData` has no representation.
unsafe impl ZcElem for LifetimeMarker<'_> {}

// SAFETY: the schema type is its own complete alignment-one representation.
unsafe impl ZcField for LifetimeMarker<'_> {
    type Pod = Self;
}

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct Borrowed<'a> {
    marker: LifetimeMarker<'a>,
    text: pinapod::String<8>,
}

fn main() {
    let bytes = [0; Borrowed::<'static>::HEADER_SIZE];
    let _ = BorrowedRef::<'_, 'static>::new(&bytes);
}
