//! Manual `SchemaWrite` / `SchemaRead` impls for pod types that cannot use
//! derive (generic params, `MaybeUninit` fields).
//!
//! The inactive capacity of a container may be uninitialized or may contain
//! stale bytes read from an existing account. Writers therefore emit only
//! active values and canonical zero padding. This avoids reading uninitialized
//! memory, prevents stale-capacity disclosure, and makes equivalent logical
//! values encode identically.

use {
    super::{bool::PodBool, option::PodOption, string::PodString, vec::PodVec},
    crate::traits::ZcElem,
    wincode::{config::ConfigCore, TypeMeta},
};

macro_rules! static_encoded {
    ($type:ty) => {
        TypeMeta::Static {
            size: core::mem::size_of::<$type>(),
            zero_copy: false,
        }
    };
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
        _ => Err(wincode::error::WriteError::Custom(
            "Pinapod container elements require a fixed wincode representation matching their in-memory size",
        )),
    }
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
        __dst.write(__val);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PodVec
// ---------------------------------------------------------------------------

unsafe impl<T, const N: usize, const PFX: usize, C> wincode::SchemaWrite<C> for PodVec<T, N, PFX>
where
    T: ZcElem + wincode::SchemaWrite<C, Src = T>,
    C: ConfigCore,
{
    type Src = Self;

    fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
        require_fixed_wire_size::<T, C>()?;
        Ok(core::mem::size_of::<Self>())
    }

    fn write(
        mut __writer: impl wincode::io::Writer,
        src: &Self,
    ) -> wincode::error::WriteResult<()> {
        let element_size = require_fixed_wire_size::<T, C>()?;
        write_initialized_prefix(__writer.by_ref(), src, PFX)?;
        for value in src.as_slice() {
            <T as wincode::SchemaWrite<C>>::write(__writer.by_ref(), value)?;
        }
        write_zeroed_padding(__writer, (N - src.len()) * element_size)
    }
}

unsafe impl<'__de, T: ZcElem, const N: usize, const PFX: usize, C: ConfigCore>
    wincode::SchemaRead<'__de, C> for PodVec<T, N, PFX>
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

    fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
        require_fixed_wire_size::<T, C>()?;
        Ok(core::mem::size_of::<Self>())
    }

    fn write(
        mut __writer: impl wincode::io::Writer,
        src: &Self,
    ) -> wincode::error::WriteResult<()> {
        let value_size = require_fixed_wire_size::<T, C>()?;
        write_initialized_prefix(__writer.by_ref(), src, PFX)?;
        match src.get_ref() {
            Some(value) => <T as wincode::SchemaWrite<C>>::write(__writer, value),
            None if src.tag_valid() => write_zeroed_padding(__writer, value_size),
            None => Err(wincode::error::WriteError::Custom(
                "Pinapod option has an invalid tag",
            )),
        }
    }
}

unsafe impl<'__de, T: ZcElem, const PFX: usize, C: ConfigCore> wincode::SchemaRead<'__de, C>
    for PodOption<T, PFX>
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
            .map_err(|_| wincode::error::ReadError::InvalidValue("PodOption validation failed"))?;
        __dst.write(__val);
        Ok(())
    }
}
