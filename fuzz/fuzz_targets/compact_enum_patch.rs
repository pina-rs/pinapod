//! Fuzzes compact enum patches across repr widths: `initialize` and `update`
//! commits, rejection of over-capacity payloads, failure atomicity, and
//! round-trips through a fresh validated read.
//!
//! Payload lengths decode above capacity on purpose, so `Overflow`
//! rejections and their leave-the-buffer-untouched contract are part of the
//! fuzzed surface rather than unit-test-only paths.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pinapod::PinaPod;
use pinapod::PinaPodCompact;
use pinapod::pod::PodU16;

#[allow(dead_code)]
#[derive(PinaPod)]
struct FixedEventPayload {
	pub amount: u64,
	pub enabled: bool,
}

#[allow(dead_code)]
#[repr(u8)]
#[derive(PinaPod)]
#[pinapod(compact)]
enum CompactEvent {
	Empty = 0,
	Label(pinapod::String<8>) = 1,
	Points(pinapod::Vec<u16, 3>) = 2,
	Fixed(FixedEventPayload) = 3,
}

#[allow(dead_code)]
#[repr(u16)]
#[derive(PinaPod)]
#[pinapod(compact)]
enum WideCompactEvent {
	Empty = 0,
	Label(pinapod::String<8>) = 300,
}

/// A tiny cursor over the fuzz input; an exhausted input keeps yielding the
/// zero byte so every decode stays total.
struct Plan<'a> {
	bytes: &'a [u8],
}

impl Plan<'_> {
	fn byte(&mut self) -> u8 {
		let split = self.bytes.len().min(1);
		let (head, tail) = self.bytes.split_at(split);
		self.bytes = tail;
		head.first().copied().unwrap_or(0)
	}
}

fn ascii(seed: u8, count: usize) -> String {
	(0..count)
		.map(|index| b'a' + seed.wrapping_add(index as u8) % 26)
		.map(|byte| byte as char)
		.collect()
}

enum EventPlan {
	Empty,
	Label(String),
	Points(Vec<PodU16>),
	Fixed(u64, bool),
}

impl EventPlan {
	fn kind(&self) -> &'static str {
		match self {
			Self::Empty => "empty",
			Self::Label(_) => "label",
			Self::Points(_) => "points",
			Self::Fixed(..) => "fixed",
		}
	}
}

/// `label` may reach 12 over a capacity of 8 and `points` may reach 5 over 3,
/// so over-capacity rejections are reachable.
fn decode_event(plan: &mut Plan<'_>) -> EventPlan {
	match plan.byte() % 4 {
		0 => EventPlan::Empty,
		1 => EventPlan::Label(ascii(plan.byte(), plan.byte() as usize % 12)),
		2 => {
			let count = plan.byte() as usize % 5;
			EventPlan::Points(
				(0..count)
					.map(|index| PodU16::from(1_000 + index as u16))
					.collect(),
			)
		}
		_ => {
			let amount = plan.byte() as u64 | (plan.byte() as u64) << 8;
			EventPlan::Fixed(amount, plan.byte() % 2 == 1)
		}
	}
}

/// Builds the patch over `scratch`, which must hold exactly one
/// `FixedEventPayload` representation for the fixed-payload variant.
fn as_patch<'a>(plan: &'a EventPlan, scratch: &'a mut [u8]) -> Option<CompactEventPatch<'a>> {
	match plan {
		EventPlan::Empty => Some(CompactEventPatch::Empty),
		EventPlan::Label(label) => Some(CompactEventPatch::Label(label)),
		EventPlan::Points(points) => Some(CompactEventPatch::Points(points)),
		EventPlan::Fixed(amount, enabled) => {
			let payload = FixedEventPayload::read_exact_mut(scratch).ok()?;
			payload.amount = (*amount).into();
			payload.enabled = (*enabled).into();
			Some(CompactEventPatch::Fixed(payload))
		}
	}
}

fn assert_event_roundtrip(data: &[u8], plan: &EventPlan) {
	match CompactEvent::read_prefix(data).expect("committed bytes must validate") {
		CompactEventRef::Empty => assert_eq!(plan.kind(), "empty"),
		CompactEventRef::Label(label) => {
			match plan {
				EventPlan::Label(expected) => assert_eq!(label, expected.as_str()),
				other => panic!("committed a label but planned {}", other.kind()),
			}
		}
		CompactEventRef::Points(points) => {
			match plan {
				EventPlan::Points(expected) => assert_eq!(points, expected.as_slice()),
				other => panic!("committed points but planned {}", other.kind()),
			}
		}
		CompactEventRef::Fixed(payload) => {
			match plan {
				EventPlan::Fixed(amount, enabled) => {
					assert_eq!(payload.amount.get(), *amount);
					assert_eq!(payload.enabled.get(), *enabled);
				}
				other => panic!("committed a fixed payload but planned {}", other.kind()),
			}
		}
	}
}

fuzz_target!(|data: &[u8]| {
	let mut plan = Plan { bytes: data };
	let first = decode_event(&mut plan);
	let second = decode_event(&mut plan);

	let mut buffer = vec![0u8; <CompactEvent as PinaPodCompact>::MAX_SIZE];
	let mut scratch = vec![0u8; FixedEventPayload::SIZE];

	if let Some(patch) = as_patch(&first, &mut scratch) {
		match CompactEvent::initialize(&mut buffer, &patch) {
			Ok(encoded_len) => {
				assert!(encoded_len <= buffer.len());
				assert_event_roundtrip(&buffer[..encoded_len], &first);
			}
			Err(_) => {
				assert!(
					buffer.iter().all(|byte| *byte == 0),
					"a rejected initialize must leave the destination zeroed"
				);
			}
		}
	}

	// `updated_len` and `update` must agree, and a rejection must not mutate.
	let snapshot = buffer.clone();
	if let Some(patch) = as_patch(&second, &mut scratch) {
		match patch.updated_len(&buffer) {
			Ok(announced) => {
				let committed = CompactEvent::update(&mut buffer, &patch)
					.expect("update must succeed when the preflight announced a length");
				assert_eq!(
					announced, committed,
					"preflight must match the commit length"
				);
				assert!(committed <= buffer.len());
				assert_event_roundtrip(&buffer[..committed], &second);
			}
			Err(_) => {
				assert_eq!(
					CompactEvent::update(&mut buffer, &patch),
					Err(pinapod::PinaPodError::Overflow),
					"only an over-capacity payload can be rejected over initialized bytes"
				);
				assert_eq!(
					buffer, snapshot,
					"a rejected update must leave the buffer byte-identical"
				);
			}
		}
	}

	// The wide-repr enum gets the same initialize-and-round-trip treatment.
	let label = ascii(plan.byte(), plan.byte() as usize % 12);
	let wide_patch = WideCompactEventPatch::Label(&label);
	let mut wide_buffer = vec![0u8; <WideCompactEvent as PinaPodCompact>::MAX_SIZE];
	match WideCompactEvent::initialize(&mut wide_buffer, &wide_patch) {
		Ok(encoded_len) => {
			match WideCompactEvent::read_prefix(&wide_buffer[..encoded_len])
				.expect("committed bytes must validate")
			{
				WideCompactEventRef::Label(actual) => assert_eq!(actual, label.as_str()),
				WideCompactEventRef::Empty => panic!("committed empty but planned a label"),
			}
		}
		Err(_) => {
			assert!(
				wide_buffer.iter().all(|byte| *byte == 0),
				"a rejected initialize must leave the destination zeroed"
			)
		}
	}
});
