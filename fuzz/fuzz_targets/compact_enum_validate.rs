//! Fuzzes compact enums across repr widths and payload kinds: unit variants,
//! string payloads, vector payloads, and fixed-struct payloads.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pinapod::{PinaPod, String, Vec};

#[allow(dead_code)]
#[derive(PinaPod)]
struct FixedEventPayload {
    amount: u64,
    enabled: bool,
}

#[allow(dead_code)]
#[repr(u8)]
#[derive(PinaPod)]
#[pinapod(compact)]
enum CompactEvent {
    Empty = 0,
    Label(String<8>) = 1,
    Points(Vec<u16, 3>) = 2,
    Fixed(FixedEventPayload) = 3,
}

#[allow(dead_code)]
#[repr(u16)]
#[derive(PinaPod)]
#[pinapod(compact)]
enum WideCompactEvent {
    Empty = 0,
    Label(String<8>) = 300,
}

fuzz_target!(|data: &[u8]| {
    if let Ok(view) = CompactEvent::read_prefix(data) {
        match &view {
            CompactEventRef::Empty => {}
            CompactEventRef::Label(label) => assert!(label.len() <= 8),
            CompactEventRef::Points(points) => assert!(points.len() <= 3),
            CompactEventRef::Fixed(_) => {}
        }
    }

    if let Ok(view) = WideCompactEvent::read_prefix(data) {
        match &view {
            WideCompactEventRef::Empty => {}
            WideCompactEventRef::Label(label) => assert!(label.len() <= 8),
        }
    }
});
