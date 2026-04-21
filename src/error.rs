use thiserror::Error;

#[derive(Debug, Error)]
pub enum BLPError {
    #[error("Invalid magic number: expected BLP1, got {0:?}")]
    InvalidMagic([u8; 4]),

    #[error("Unsupported alpha bit depth: {0}")]
    UnsupportedAlpha(u32),

    #[error("JPEG decode failed: {0}")]
    JpegDecodeFailed(String),

    #[error("Corrupted data: {0}")]
    CorruptedData(&'static str),

    #[error("Invalid mipmap offset at level {0}")]
    InvalidMipmapOffset(usize),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Encode failed: {0}")]
    EncodeFailed(String),
}

pub type Result<T> = std::result::Result<T, BLPError>;
