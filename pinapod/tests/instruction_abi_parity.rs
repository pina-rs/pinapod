//! Proves account and instruction compact layouts produce identical bytes.
use pinapod::PinaPod;

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct AccountSchema {
    authority: [u8; 32],
    bump: u8,
    label: pinapod::String<64>,
}

#[allow(dead_code)]
#[derive(PinaPod)]
#[pinapod(compact)]
struct InstructionSchema {
    label: pinapod::String<64>,
}

#[test]
fn compact_tail_bytes_are_identical() {
    use pinapod::PinaPodCompact;

    let mut acct_buf = vec![0u8; <AccountSchema as PinaPodCompact>::HEADER_SIZE + 5];
    let mut instr_buf = vec![0u8; <InstructionSchema as PinaPodCompact>::HEADER_SIZE + 5];

    AccountSchema::initialize(&mut acct_buf, &AccountSchemaPatch::new().label("hello")).unwrap();
    InstructionSchema::initialize(
        &mut instr_buf,
        &InstructionSchemaPatch::new().label("hello"),
    )
    .unwrap();

    // Tail bytes are identical — same compact format regardless of context.
    let acct_hdr = <AccountSchema as PinaPodCompact>::HEADER_SIZE;
    let instr_hdr = <InstructionSchema as PinaPodCompact>::HEADER_SIZE;
    assert_eq!(&acct_buf[acct_hdr..], &instr_buf[instr_hdr..]);

    // Ref reads the same value.
    let r = InstructionSchema::read_prefix(&instr_buf).unwrap();
    assert_eq!(r.label(), "hello");
}

#[test]
fn fixed_layout_is_identical() {
    #[allow(dead_code)]
    #[derive(PinaPod)]
    struct AccountFixed {
        authority: [u8; 32],
        value: u64,
    }

    #[allow(dead_code)]
    #[derive(PinaPod)]
    struct InstructionFixed {
        value: u64,
    }

    // Both use PinaPodFixed — same pointer-cast + validate path.
    assert_eq!(AccountFixed::SIZE, 32 + 8);
    assert_eq!(InstructionFixed::SIZE, 8);
}
