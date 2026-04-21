use crate::error::BLPError;

#[inline]
pub fn read_be_u16(b: &[u8]) -> std::result::Result<u16, BLPError> {
    if b.len() < 2 {
        return Err(BLPError::EncodeFailed("read_be_u16 underflow".to_string()));
    }
    Ok(((b[0] as u16) << 8) | b[1] as u16)
}