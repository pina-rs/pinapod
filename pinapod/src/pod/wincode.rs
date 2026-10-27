//! Manual `SchemaWrite` / `SchemaRead` impls for pod types that cannot use
//! derive (generic params, `MaybeUninit` fields).
//!
//! The inactive capacity of a container may contain stale bytes read from an
//! existing account. Writers therefore emit only active values and canonical
//! zero padding. This prevents stale-capacity disclosure and makes equivalent
//! logical values encode identically.
//! Readers decode active values through their nested codecs and reject nonzero
//! inactive storage, including padding inside nested containers.
//!
//! Every writer also validates the value it is about to emit — prefix, tag,
//! and content alike — through [`ZcValidate`](crate::ZcValidate), the same
//! walk its reader runs. A value whose metadata is in range but whose content
//! is invalid (non-UTF-8 string bytes, an out-of-domain `PodBool` element)
//! therefore fails at the writer instead of producing bytes the matching
//! reader refuses: the canonical-output contract holds for every accepted
//! serialization, not only for already-valid values. Because a pod value can
//! only reach a writer with invalid content through an unchecked reference,
//! this is a hardening boundary rather than a reachable defect; it exists so
//! the writer and reader enforce one contract.

use wincode::TypeMeta;
use wincode::config::ConfigCore;

use super::bool::PodBool;
use super::option::PodOption;
use super::string::PodString;
use super::vec::PodVecRepr;
use crate::traits::ZcElem;

macro_rules! static_encoded {
	($type:ty) => {
		TypeMeta::Static {
			size: core::mem::size_of::<$type>(),
			zero_copy: false,
		}
	};
}

/// Run the stored-value validation walk a reader would run, reporting
/// failures with the writer's boundary message.
fn validate_wire_value<T: crate::ZcValidate>(
	src: &T,
	message: &'static str,
) -> wincode::error::WriteResult<()> {
	crate::ZcValidate::validate_ref(src).map_err(|_| wincode::error::WriteError::Custom(message))
}

unsafe impl<C: ConfigCore> wincode::SchemaWrite<C> for PodBool {
	type Src = Self;

	const TYPE_META: TypeMeta = static_encoded!(Self);

	fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
		Ok(core::mem::size_of::<Self>())
	}

	fn write(
		mut __writer: impl wincode::io::Writer,
		src: &Self,
	) -> wincode::error::WriteResult<()> {
		validate_wire_value(src, "PinaPod bool content failed validation")?;
		__writer.write(src.as_ref())?;
		Ok(())
	}
}

unsafe impl<'__de, C: ConfigCore> wincode::SchemaRead<'__de, C> for PodBool {
	type Dst = Self;

	const TYPE_META: TypeMeta = static_encoded!(Self);

	fn read(
		mut __reader: impl wincode::io::Reader<'__de>,
		__dst: &mut core::mem::MaybeUninit<Self>,
	) -> wincode::error::ReadResult<()> {
		let __bytes = __reader.take_scoped(core::mem::size_of::<Self>())?;
		let __val = unsafe { core::ptr::read_unaligned(__bytes.as_ptr().cast::<Self>()) };
		<Self as crate::ZcValidate>::validate_ref(&__val)
			.map_err(|_| wincode::error::ReadError::InvalidValue("PodBool validation failed"))?;
		__dst.write(__val);
		Ok(())
	}
}

fn write_initialized_prefix<T>(
	mut writer: impl wincode::io::Writer,
	value: &T,
	prefix_len: usize,
) -> wincode::error::WriteResult<()> {
	// SAFETY: Every caller restricts this read to a leading `[u8; PFX]`
	// length or tag field, which is always initialized and has alignment one.
	let prefix = unsafe { core::slice::from_raw_parts(value as *const T as *const u8, prefix_len) };
	writer.write(prefix)?;
	Ok(())
}

fn write_zeroed_padding(
	mut writer: impl wincode::io::Writer,
	mut remaining: usize,
) -> wincode::error::WriteResult<()> {
	const ZEROS: [u8; 64] = [0; 64];

	while remaining != 0 {
		let count = remaining.min(ZEROS.len());
		writer.write(&ZEROS[..count])?;
		remaining -= count;
	}

	Ok(())
}

fn require_fixed_wire_size<T, C>() -> wincode::error::WriteResult<usize>
where
	T: wincode::SchemaWrite<C, Src = T>,
	C: ConfigCore,
{
	match <T as wincode::SchemaWrite<C>>::TYPE_META {
		TypeMeta::Static { size, .. } if size == core::mem::size_of::<T>() => Ok(size),
		_ => {
			Err(wincode::error::WriteError::Custom(
				"PinaPod container elements require a fixed wincode representation matching their \
				 in-memory size",
			))
		}
	}
}

/// Decode a raw little-endian prefix without applying Wincode's integer config.
fn read_prefix<'de, const PFX: usize>(
	mut reader: impl wincode::io::Reader<'de>,
) -> wincode::error::ReadResult<u64> {
	let mut bytes = [0; 8];

	if !matches!(PFX, 1 | 2 | 4 | 8) {
		return Err(wincode::error::ReadError::InvalidValue(
			"invalid PinaPod prefix width",
		));
	}

	reader.copy_into_slice(&mut bytes[..PFX])?;
	Ok(u64::from_le_bytes(bytes))
}

/// Require the same fixed stride the container writer requires.
fn require_fixed_read_size<'de, T, C>() -> wincode::error::ReadResult<usize>
where
	T: wincode::SchemaRead<'de, C, Dst = T>,
	C: ConfigCore,
{
	match <T as wincode::SchemaRead<'de, C>>::TYPE_META {
		TypeMeta::Static { size, .. } if size == core::mem::size_of::<T>() => Ok(size),
		_ => {
			Err(wincode::error::ReadError::InvalidValue(
				"PinaPod container elements require a fixed wincode representation matching their \
				 in-memory size",
			))
		}
	}
}

/// Consume canonical padding without decoding inactive elements.
fn read_zeroed_padding<'de>(
	mut reader: impl wincode::io::Reader<'de>,
	mut remaining: usize,
) -> wincode::error::ReadResult<()> {
	let mut bytes = [0; 64];

	while remaining != 0 {
		let count = remaining.min(bytes.len());
		reader.copy_into_slice(&mut bytes[..count])?;

		if bytes[..count].iter().any(|byte| *byte != 0) {
			return Err(wincode::error::ReadError::InvalidValue(
				"nonzero PinaPod inactive storage",
			));
		}

		remaining -= count;
	}

	Ok(())
}

// ---------------------------------------------------------------------------
// PodString
// ---------------------------------------------------------------------------
unsafe impl<const N: usize, const PFX: usize, C: ConfigCore> wincode::SchemaWrite<C>
	for PodString<N, PFX>
{
	type Src = Self;

	const TYPE_META: TypeMeta = static_encoded!(Self);

	fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
		Ok(core::mem::size_of::<Self>())
	}

	fn write(
		mut __writer: impl wincode::io::Writer,
		src: &Self,
	) -> wincode::error::WriteResult<()> {
		// The prefix is emitted verbatim, so a corrupt stored prefix (one that
		// decodes above the capacity, or wider than `usize`) would serialize
		// into a wire value that fails on re-read. Reject it at the boundary,
		// the same way the option writer rejects an invalid tag.
		if src.decode_len() > src.capacity() {
			return Err(wincode::error::WriteError::Custom(
				"PinaPod string length prefix exceeds its capacity",
			));
		}
		// The content walk the reader runs: an in-range prefix over
		// non-UTF-8 bytes fails here instead of emitting bytes that fail on
		// re-read.
		validate_wire_value(src, "PinaPod string content failed validation")?;

		write_initialized_prefix(__writer.by_ref(), src, PFX)?;
		__writer.write(src.as_bytes())?;
		write_zeroed_padding(__writer, N - src.len())
	}
}

unsafe impl<'__de, const N: usize, const PFX: usize, C: ConfigCore> wincode::SchemaRead<'__de, C>
	for PodString<N, PFX>
{
	type Dst = Self;

	const TYPE_META: TypeMeta = static_encoded!(Self);

	fn read(
		mut __reader: impl wincode::io::Reader<'__de>,
		__dst: &mut core::mem::MaybeUninit<Self>,
	) -> wincode::error::ReadResult<()> {
		let __bytes = __reader.take_scoped(core::mem::size_of::<Self>())?;
		let __val = unsafe { core::ptr::read_unaligned(__bytes.as_ptr() as *const Self) };
		<Self as crate::ZcValidate>::validate_ref(&__val)
			.map_err(|_| wincode::error::ReadError::InvalidValue("PodString validation failed"))?;

		if __bytes[PFX + __val.len()..].iter().any(|byte| *byte != 0) {
			return Err(wincode::error::ReadError::InvalidValue(
				"nonzero PinaPod inactive storage",
			));
		}

		__dst.write(__val);
		Ok(())
	}
}

// ---------------------------------------------------------------------------
// PodVec
// ---------------------------------------------------------------------------
unsafe impl<T, const N: usize, const PFX: usize, C> wincode::SchemaWrite<C>
	for PodVecRepr<T, N, PFX>
where
	T: ZcElem + wincode::SchemaWrite<C, Src = T>,
	C: ConfigCore,
{
	type Src = Self;

	const TYPE_META: TypeMeta = match <T as wincode::SchemaWrite<C>>::TYPE_META {
		TypeMeta::Static { size, .. } if size == core::mem::size_of::<T>() => {
			static_encoded!(Self)
		}

		_ => TypeMeta::Dynamic,
	};

	fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
		require_fixed_wire_size::<T, C>()?;
		Ok(core::mem::size_of::<Self>())
	}

	fn write(
		mut __writer: impl wincode::io::Writer,
		src: &Self,
	) -> wincode::error::WriteResult<()> {
		let element_size = require_fixed_wire_size::<T, C>()?;
		// Same boundary check as the string writer: the prefix is emitted
		// verbatim, so a stored count above the capacity must fail here rather
		// than round-trip into bytes the reader rejects.
		if src.decode_len() > src.capacity() {
			return Err(wincode::error::WriteError::Custom(
				"PinaPod vector length prefix exceeds its capacity",
			));
		}
		// The element walk the reader runs: a valid count over out-of-domain
		// elements (a `PodBool` byte above one, non-UTF-8 nested string bytes)
		// fails here instead of emitting bytes that fail on re-read.
		validate_wire_value(src, "PinaPod vector content failed validation")?;

		write_initialized_prefix(__writer.by_ref(), src, PFX)?;

		for value in src.as_slice() {
			<T as wincode::SchemaWrite<C>>::write(__writer.by_ref(), value)?;
		}

		write_zeroed_padding(__writer, (N - src.len()) * element_size)
	}
}

unsafe impl<'__de, T, const N: usize, const PFX: usize, C> wincode::SchemaRead<'__de, C>
	for PodVecRepr<T, N, PFX>
where
	T: ZcElem + wincode::SchemaRead<'__de, C, Dst = T>,
	C: ConfigCore,
{
	type Dst = Self;

	const TYPE_META: TypeMeta = match <T as wincode::SchemaRead<'__de, C>>::TYPE_META {
		TypeMeta::Static { size, .. } if size == core::mem::size_of::<T>() => static_encoded!(Self),
		_ => TypeMeta::Dynamic,
	};

	fn read(
		mut __reader: impl wincode::io::Reader<'__de>,
		__dst: &mut core::mem::MaybeUninit<Self>,
	) -> wincode::error::ReadResult<()> {
		let element_size = require_fixed_read_size::<T, C>()?;
		let len = read_prefix::<PFX>(__reader.by_ref())?;
		let len = usize::try_from(len).ok().filter(|len| *len <= N).ok_or(
			wincode::error::ReadError::InvalidValue("PodVec length exceeds capacity"),
		)?;
		let mut __val = Self::default();

		for _ in 0..len {
			let value = <T as wincode::SchemaRead<'__de, C>>::get(__reader.by_ref())?;
			__val.try_push(value).map_err(|_| {
				wincode::error::ReadError::InvalidValue("PodVec length exceeds capacity")
			})?;
		}

		read_zeroed_padding(__reader, (N - len) * element_size)?;
		<Self as crate::ZcValidate>::validate_ref(&__val)
			.map_err(|_| wincode::error::ReadError::InvalidValue("PodVec validation failed"))?;
		__dst.write(__val);
		Ok(())
	}
}

// ---------------------------------------------------------------------------
// PodOption
// ---------------------------------------------------------------------------
unsafe impl<T, const PFX: usize, C> wincode::SchemaWrite<C> for PodOption<T, PFX>
where
	T: ZcElem + wincode::SchemaWrite<C, Src = T>,
	C: ConfigCore,
{
	type Src = Self;

	const TYPE_META: TypeMeta = match <T as wincode::SchemaWrite<C>>::TYPE_META {
		TypeMeta::Static { size, .. } if size == core::mem::size_of::<T>() => {
			static_encoded!(Self)
		}

		_ => TypeMeta::Dynamic,
	};

	fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
		require_fixed_wire_size::<T, C>()?;
		Ok(core::mem::size_of::<Self>())
	}

	fn write(
		mut __writer: impl wincode::io::Writer,
		src: &Self,
	) -> wincode::error::WriteResult<()> {
		let value_size = require_fixed_wire_size::<T, C>()?;
		// The tag is emitted verbatim, so an invalid one must fail before any
		// byte is written, matching the string and vector writers' rejection
		// ordering: a rejected serialization leaves the destination alone.
		if !src.tag_valid() {
			return Err(wincode::error::WriteError::Custom(
				"PinaPod option has an invalid tag",
			));
		}
		// The payload walk the reader runs: a valid tag over an invalid
		// payload (an out-of-domain `PodBool`, non-UTF-8 nested string bytes)
		// fails here instead of emitting bytes that fail on re-read.
		validate_wire_value(src, "PinaPod option content failed validation")?;

		write_initialized_prefix(__writer.by_ref(), src, PFX)?;

		match src.get_ref() {
			Some(value) => <T as wincode::SchemaWrite<C>>::write(__writer, value),

			// `tag_valid` accepted the tag, so `None` can only mean the tag
			// encodes absent and the inactive payload stays unemitted.
			None => write_zeroed_padding(__writer, value_size),
		}
	}
}

unsafe impl<'__de, T, const PFX: usize, C> wincode::SchemaRead<'__de, C> for PodOption<T, PFX>
where
	T: ZcElem + wincode::SchemaRead<'__de, C, Dst = T>,
	C: ConfigCore,
{
	type Dst = Self;

	const TYPE_META: TypeMeta = match <T as wincode::SchemaRead<'__de, C>>::TYPE_META {
		TypeMeta::Static { size, .. } if size == core::mem::size_of::<T>() => static_encoded!(Self),
		_ => TypeMeta::Dynamic,
	};

	fn read(
		mut __reader: impl wincode::io::Reader<'__de>,
		__dst: &mut core::mem::MaybeUninit<Self>,
	) -> wincode::error::ReadResult<()> {
		let value_size = require_fixed_read_size::<T, C>()?;
		let __val = match read_prefix::<PFX>(__reader.by_ref())? {
			0 => {
				read_zeroed_padding(__reader, value_size)?;
				Self::none()
			}
			1 => Self::some(<T as wincode::SchemaRead<'__de, C>>::get(__reader)?),
			_ => {
				return Err(wincode::error::ReadError::InvalidValue(
					"PodOption validation failed",
				));
			}
		};
		<Self as crate::ZcValidate>::validate_ref(&__val)
			.map_err(|_| wincode::error::ReadError::InvalidValue("PodOption validation failed"))?;
		__dst.write(__val);
		Ok(())
	}
}
