//! Fuzzes the fixed-container mutators: every operation sequence on a
//! `PodString` and `PodVec` must keep lengths inside capacity, keep active
//! string bytes valid UTF-8, and never panic.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pinapod::{pod::PodOption, ZcValidate};

const STRING_CAP: usize = 16;
const VEC_CAP: usize = 12;

/// Maps a byte onto ASCII so every pushed string is valid UTF-8.
fn ascii(byte: u8, len: usize) -> String {
    core::iter::repeat(b'a' + byte % 26)
        .take(len)
        .collect::<Vec<u8>>()
        .into_iter()
        .map(|byte| byte as char)
        .collect()
}

fuzz_target!(|data: &[u8]| {
    let mut text = pinapod::String::<STRING_CAP>::default();
    let mut values = pinapod::Vec::<u8, VEC_CAP>::default();
    let mut flag = PodOption::<u8>::none();

    for (step, byte) in data.iter().enumerate() {
        match byte % 10 {
            0 => {
                let len = (byte >> 4) as usize % (STRING_CAP + 1);
                let _ = text.try_set(&ascii(step as u8, len));
            }
            1 => {
                let len = (byte >> 4) as usize % 5;
                let _ = text.try_push_str(&ascii(step as u8, len));
            }
            2 => text.truncate((byte >> 4) as usize % (STRING_CAP + 1)),
            3 => text.clear(),
            4 => {
                let _ = values.try_push(*byte);
            }
            5 => {
                if let Some(popped) = values.pop() {
                    std::hint::black_box(popped);
                }
            }
            6 => values.truncate((byte >> 4) as usize % (VEC_CAP + 1)),
            7 => {
                let count = ((byte >> 4) as usize % 4).min(data.len());
                let _ = values.try_extend_from_slice(&data[..count]);
            }
            8 => {
                if let Some(removed) = values.swap_remove((byte >> 4) as usize % (VEC_CAP + 1)) {
                    std::hint::black_box(removed);
                }
            }
            _ => {
                let present = *byte % 3 == 0;
                flag.set(present.then_some(*byte));
                assert_eq!(flag.is_some(), present);
            }
        }

        assert!(text.len() <= STRING_CAP);
        assert!(values.len() <= VEC_CAP);
        assert!(ZcValidate::validate_ref(&text).is_ok());
        assert!(ZcValidate::validate_ref(&values).is_ok());
    }
});
