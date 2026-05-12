//! Re-implementations of cbor4ii's `pub(crate)` `peek_one`/`pull_one` so we can produce our own
//! `DecodeError::Eof` instead of going through the upstream constructor.

use cbor4ii::core::dec;
use cbor4ii::core::error::Len;

use crate::error::DecodeError;

#[inline]
pub(crate) fn peek_one<'a, R: dec::Read<'a>>(
    name: &'static str,
    reader: &mut R,
) -> Result<u8, DecodeError<R::Error>> {
    let buf = reader.fill(1)?;
    let byte = buf.as_ref().first().copied().ok_or(DecodeError::Eof {
        name,
        expect: Len::Small(1),
    })?;
    Ok(byte)
}

#[inline]
pub(crate) fn pull_one<'a, R: dec::Read<'a>>(
    name: &'static str,
    reader: &mut R,
) -> Result<u8, DecodeError<R::Error>> {
    let byte = peek_one(name, reader)?;
    reader.advance(1);
    Ok(byte)
}
