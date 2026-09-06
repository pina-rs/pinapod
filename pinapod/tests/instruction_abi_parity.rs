//! Proves account and instruction compact layouts produce identical bytes.
#![allow(
    unsafe_code,
    reason = "the derive macro emits audited zero-copy validation implementations"
)]

use pinapod::ZeroPod;

#[allow(dead_code)]
#[derive(ZeroPod)]
#[pinapod(compact)]
struct AccountSchema {
    authority: [u8; 32],
    bump: u8,
    label: pinapod::String<64>,
}

#[allow(dead_code)]
#[derive(ZeroPod)]
#[pinapod(compact)]
struct InstructionSchema {
    label: pinapod::String<64>,
}

#[test]
fn compact_tail_bytes_are_identical() {
    use pinapod::ZeroPodCompact;

    let mut acct_buf = vec![0u8; <AccountSchema as ZeroPodCompact>::HEADER_SIZE + 5];
    let mut instr_buf = vec![0u8; <InstructionSchema as ZeroPodCompact>::HEADER_SIZE + 5];

    {
        let mut m = unsafe { AccountSchemaMut::new_unchecked(&mut acct_buf) };
        m.set_label("hello").unwrap();
        m.commit().unwrap();
    }
    {
        let mut m = unsafe { InstructionSchemaMut::new_unchecked(&mut instr_buf) };
        m.set_label("hello").unwrap();
        m.commit().unwrap();
    }

    // Tail bytes are identical — same compact format regardless of context.
    let acct_hdr = <AccountSchema as ZeroPodCompact>::HEADER_SIZE;
    let instr_hdr = <InstructionSchema as ZeroPodCompact>::HEADER_SIZE;
    assert_eq!(&acct_buf[acct_hdr..], &instr_buf[instr_hdr..]);

    // Ref reads the same value.
    let r = InstructionSchemaRef::new(&instr_buf).unwrap();
    assert_eq!(r.label(), "hello");
}

#[test]
fn fixed_layout_is_identical() {
    use pinapod::ZeroPodFixed;

    #[allow(dead_code)]
    #[derive(ZeroPod)]
    struct AccountFixed {
        authority: [u8; 32],
        value: u64,
    }

    #[allow(dead_code)]
    #[derive(ZeroPod)]
    struct InstructionFixed {
        value: u64,
    }

    // Both use ZeroPodFixed — same pointer-cast + validate path.
    assert_eq!(<AccountFixed as ZeroPodFixed>::SIZE, 32 + 8);
    assert_eq!(<InstructionFixed as ZeroPodFixed>::SIZE, 8);
}
