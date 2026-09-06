#![cfg(feature = "fixed")]

use {
    core::mem::{align_of, size_of},
    fixed::types::{
        I0F128, I0F16, I0F32, I0F64, I0F8, I16F16, U0F128, U0F16, U0F32, U0F64, U0F8, U24F8,
    },
    pinapod::{
        pod::{PodI128, PodI16, PodI32, PodI64, PodU128, PodU16, PodU32, PodU64},
        ZcElem, ZcField, ZeroPod, ZeroPodCompact, ZeroPodFixed,
    },
    std::vec::Vec as StdVec,
};

fn assert_mapping<T, Pod>()
where
    T: ZcField<Pod = Pod>,
    Pod: ZcElem,
{
    assert_eq!(T::POD_SIZE, size_of::<Pod>());
    assert_eq!(align_of::<Pod>(), 1);
}

#[test]
fn every_fixed_width_maps_to_an_alignment_one_integer_pod() {
    assert_mapping::<I0F8, i8>();
    assert_mapping::<I0F16, PodI16>();
    assert_mapping::<I0F32, PodI32>();
    assert_mapping::<I0F64, PodI64>();
    assert_mapping::<I0F128, PodI128>();
    assert_mapping::<U0F8, u8>();
    assert_mapping::<U0F16, PodU16>();
    assert_mapping::<U0F32, PodU32>();
    assert_mapping::<U0F64, PodU64>();
    assert_mapping::<U0F128, PodU128>();
}

#[allow(dead_code)]
#[derive(ZeroPod)]
struct FixedPointAccount {
    pub price: I16F16,
    pub quantity: U24F8,
    pub previous_price: Option<I16F16>,
}

#[test]
fn fixed_schema_roundtrips_values_and_uses_little_endian_bits() {
    let price = I16F16::from_num(-12.75);
    let quantity = U24F8::from_num(125.5);
    let previous_price = I16F16::from_num(-13.0);
    let mut bytes = [0u8; FixedPointAccount::SIZE];

    {
        let account = FixedPointAccount::from_bytes_mut(&mut bytes).unwrap();
        account.price = price.to_bits().into();
        account.quantity = quantity.to_bits().into();
        account
            .previous_price
            .set(Some(previous_price.to_bits().into()));
    }

    assert_eq!(&bytes[..4], &price.to_bits().to_le_bytes());
    assert_eq!(&bytes[4..8], &quantity.to_bits().to_le_bytes());

    let account = FixedPointAccount::from_bytes(&bytes).unwrap();
    assert_eq!(I16F16::from_bits(account.price.get()), price);
    assert_eq!(U24F8::from_bits(account.quantity.get()), quantity);
    assert_eq!(
        account
            .previous_price
            .get()
            .map(|stored| I16F16::from_bits(stored.get())),
        Some(previous_price),
    );
}

#[allow(dead_code)]
#[derive(ZeroPod)]
#[pinapod(compact)]
struct FixedPointBook {
    pub mark_price: I16F16,
    pub bids: pinapod::Vec<I16F16, 8>,
    pub venue: pinapod::String<16>,
    pub asks: pinapod::Vec<U24F8, 8>,
}

#[test]
fn compact_schema_roundtrips_multiple_dynamic_fixed_point_fields() {
    let mut bytes = [0u8; 256];
    let bid_values = [
        I16F16::from_num(10.25),
        I16F16::from_num(10.5),
        I16F16::from_num(10.75),
    ];
    let ask_values = [U24F8::from_num(11.0), U24F8::from_num(11.25)];
    let bids = bid_values.map(|value| PodI32::from(value.to_bits()));
    let asks = ask_values.map(|value| PodU32::from(value.to_bits()));

    let encoded_size = {
        let mut book = FixedPointBookMut::new(&mut bytes).unwrap();
        book.mark_price = I16F16::from_num(10.5).to_bits().into();
        book.set_bids(&bids).unwrap();
        book.set_venue("Pina DEX").unwrap();
        book.set_asks(&asks).unwrap();
        book.commit().unwrap()
    };

    assert_eq!(
        encoded_size,
        FixedPointBook::HEADER_SIZE
            + bids.len() * size_of::<PodI32>()
            + "Pina DEX".len()
            + asks.len() * size_of::<PodU32>(),
    );

    let book = FixedPointBookRef::new(&bytes[..encoded_size]).unwrap();
    assert_eq!(
        I16F16::from_bits(book.mark_price.get()),
        I16F16::from_num(10.5),
    );
    assert_eq!(
        book.bids()
            .iter()
            .map(|value| I16F16::from_bits(value.get()))
            .collect::<StdVec<_>>(),
        bid_values,
    );
    assert_eq!(book.venue(), "Pina DEX");
    assert_eq!(
        book.asks()
            .iter()
            .map(|value| U24F8::from_bits(value.get()))
            .collect::<StdVec<_>>(),
        ask_values,
    );
}

#[test]
fn resizing_one_fixed_point_tail_preserves_the_others() {
    let mut bytes = [0u8; 256];
    let initial_bids = [PodI32::from(I16F16::from_num(9.5).to_bits())];
    let asks = [
        PodU32::from(U24F8::from_num(11.0).to_bits()),
        PodU32::from(U24F8::from_num(11.5).to_bits()),
    ];

    {
        let mut book = FixedPointBookMut::new(&mut bytes).unwrap();
        book.set_bids(&initial_bids).unwrap();
        book.set_venue("market").unwrap();
        book.set_asks(&asks).unwrap();
        book.commit().unwrap();
    }

    let expanded_bids = [
        PodI32::from(I16F16::from_num(9.0).to_bits()),
        PodI32::from(I16F16::from_num(9.25).to_bits()),
        PodI32::from(I16F16::from_num(9.5).to_bits()),
        PodI32::from(I16F16::from_num(9.75).to_bits()),
    ];
    let encoded_size = {
        let mut book = FixedPointBookMut::new(&mut bytes).unwrap();
        book.set_bids(&expanded_bids).unwrap();
        book.commit().unwrap()
    };

    let book = FixedPointBookRef::new(&bytes[..encoded_size]).unwrap();
    assert_eq!(book.bids(), expanded_bids);
    assert_eq!(book.venue(), "market");
    assert_eq!(book.asks(), asks);
}
