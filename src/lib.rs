mod decoder;
mod encoder;
mod error;

use once_cell::sync::OnceCell;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

pub use encoder::{encode_png, save_png, BLPEncoder, encode_file_to_blp, encode_rgba_to_blp, encode_to_blp_file, encode_raw_rgba};
pub use error::{BLPError, Result};
pub use decoder::{BLP1Header, ContentType};
pub use image::{ImageBuffer, Rgba};
pub use decoder::palette::RgbaColor;

/// BLP image format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BLPFormat {
    /// JPEG compressed (CMYK for BLP1)
    JPEG,
    /// Direct/palette format
    Direct,
}

impl std::fmt::Display for BLPFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BLPFormat::JPEG => write!(f, "JPEG"),
            BLPFormat::Direct => write!(f, "Direct"),
        }
    }
}

/// BLP image metadata
#[derive(Debug, Clone)]
pub struct BLPMetadata {
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// Number of mipmap levels
    pub mipmaps: usize,
    /// Whether the image has alpha channel
    pub has_alpha: bool,
    /// Image format (JPEG or Direct)
    pub format: BLPFormat,
}

#[derive(Debug)]
pub struct BLPDecoder {
    data: Vec<u8>,
    header: OnceCell<BLP1Header>,
}

impl BLPDecoder {
    /// Create decoder from file path
    ///
    /// # Errors
    /// Returns `BLPError::Io` if file cannot be read
    pub fn from_path<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let data = std::fs::read(path)
            .map_err(BLPError::IoError)?;
        Ok(Self { data, header: OnceCell::new() })
    }

    /// Create decoder from BLP byte slice
    pub fn from_blp_bytes(data: &[u8]) -> Result<Self> {
        Ok(Self { data: data.to_vec(), header: OnceCell::new() })
    }

    /// Create decoder from owned bytes (avoiding allocation)
    pub fn from_data(data: Vec<u8>) -> Self {
        Self { data, header: OnceCell::new() }
    }

    /// Create decoder from any `Read` source (file, network stream, stdin, etc.)
    pub fn from_reader<R: std::io::Read>(mut reader: R) -> Result<Self> {
        let mut data = Vec::new();
        std::io::Read::read_to_end(&mut reader, &mut data)
            .map_err(BLPError::IoError)?;
        Ok(Self { data, header: OnceCell::new() })
    }

    /// Get cached or parse header (parse once, reuse thereafter)
    fn header(&self) -> Result<&BLP1Header> {
        self.header.get_or_try_init(|| BLP1Header::parse(&self.data))
    }

    /// Get image dimensions without full decode
    #[deprecated(since = "0.1.5", note = "use metadata() instead")]
    pub fn dimensions(&self) -> Result<(u32, u32)> {
        let h = self.header()?;
        Ok((h.width, h.height))
    }

    /// Check if image has alpha channel
    #[deprecated(since = "0.1.5", note = "use metadata() instead")]
    pub fn has_alpha(&self) -> Result<bool> {
        let h = self.header()?;
        Ok(h.alpha_bitdepth > 0)
    }

    /// Get content type (JPEG or Direct)
    #[deprecated(since = "0.1.5", note = "use metadata().format instead")]
    pub fn content_type(&self) -> Result<&'static str> {
        Ok(match self.header()?.content_type() {
            Some(ContentType::JPEG) => "JPEG",
            Some(ContentType::Direct) => "Direct",
            None => "Unknown",
        })
    }

    /// Get number of mipmap levels
    #[deprecated(since = "0.1.5", note = "use metadata() instead")]
    pub fn mipmap_count(&self) -> Result<usize> {
        let h = self.header()?;
        Ok(h.mipmap_offsets.iter().zip(h.mipmap_sizes.iter())
            .filter(|(off, sz)| **off != 0 && **sz != 0)
            .count())
    }

    /// Get all metadata in one call
    pub fn metadata(&self) -> Result<BLPMetadata> {
        let h = self.header()?;
        let format = match h.content_type() {
            Some(ContentType::JPEG) => BLPFormat::JPEG,
            Some(ContentType::Direct) => BLPFormat::Direct,
            None => return Err(BLPError::CorruptedData("invalid content type")),
        };
        Ok(BLPMetadata {
            width: h.width,
            height: h.height,
            mipmaps: self.mipmap_count()?,
            has_alpha: h.alpha_bitdepth > 0,
            format,
        })
    }

    /// Get dimensions at a specific mipmap level
    pub fn mipmap_dimensions(&self, level: usize) -> Result<(u32, u32)> {
        let h = self.header()?;
        if level >= 16 {
            return Err(BLPError::CorruptedData("mipmap level out of range"));
        }
        Ok(((h.width >> level).max(1), (h.height >> level).max(1)))
    }

    /// Decode the BLP image to RGBA
    pub fn decode(&self) -> Result<BLPImage> {
        decode_blp_with_header(self.header()?, &self.data).map(BLPImage)
    }

    /// Decode a specific mipmap level
    pub fn decode_mipmap(&self, level: usize) -> Result<BLPImage> {
        decode_blp_mipmap_with_header(self.header()?, &self.data, level).map(BLPImage)
    }

    /// Decode all mipmap levels
    pub fn decode_all_mipmaps(&self) -> Result<Vec<BLPImage>> {
        let h = self.header()?;
        decode_blp_all_mipmaps_with_header(h, &self.data).map(|v| v.into_iter().map(BLPImage).collect())
    }

    /// Get raw BLP bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

/// Decoded BLP image - wrapper around ImageBuffer
#[derive(Debug, Clone)]
pub struct BLPImage(ImageBuffer<Rgba<u8>, Vec<u8>>);

impl BLPImage {
    /// Get image dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.0.width(), self.0.height())
    }

    /// Get width
    pub fn width(&self) -> u32 {
        self.0.width()
    }

    /// Get height
    pub fn height(&self) -> u32 {
        self.0.height()
    }

    /// Get raw pixel buffer reference
    pub fn as_image(&self) -> &ImageBuffer<Rgba<u8>, Vec<u8>> {
        &self.0
    }

    /// Get owned pixel buffer
    pub fn into_image(self) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        self.0
    }

    /// Get raw RGBA bytes reference
    pub fn as_rgba(&self) -> &[u8] {
        self.0.as_raw()
    }

    /// Convert to raw RGBA bytes
    pub fn into_rgba(self) -> Vec<u8> {
        self.0.into_raw()
    }

    /// Encode to PNG bytes
    pub fn encode_png(&self) -> Result<Vec<u8>> {
        encoder::encode_png(&self.0)
    }

    /// Save to PNG file
    pub fn save_png<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        encoder::save_png(&self.0, path)
    }

    /// Convert to PNG bytes (takes ownership)
    #[deprecated(since = "0.1.5", note = "use to_png_bytes() instead")]
    pub fn into_png(self) -> Result<Vec<u8>> {
        self.encode_png()
    }

    /// Get PNG bytes (borrowed self)
    pub fn to_png_bytes(&self) -> Result<Vec<u8>> {
        encoder::encode_png(&self.0)
    }

    /// Get pixel at (x, y)
    pub fn get_pixel(&self, x: u32, y: u32) -> Rgba<u8> {
        *self.0.get_pixel(x, y)
    }

    /// Iterate over pixels
    pub fn pixels(&self) -> impl Iterator<Item = Rgba<u8>> + '_ {
        self.0.pixels().map(|p| *p)
    }

    /// Create BLPImage from ImageBuffer
    pub fn from_image(img: ImageBuffer<Rgba<u8>, Vec<u8>>) -> Self {
        Self(img)
    }

    /// Encode to PNG bytes (convenience alias for to_png_bytes)
    pub fn to_png(&self) -> Result<Vec<u8>> {
        self.to_png_bytes()
    }

    /// Encode decoded image back to BLP bytes
    ///
    /// # Example
    /// ```rust
    /// let blp_bytes = decoded_img.to_blp()?;
    /// ```
    pub fn to_blp(&self) -> Result<Vec<u8>> {
        let encoder = BLPEncoder::from_image(&self.0)?;
        encoder.encode()
    }

    /// Encode decoded image to BLP and save to file
    pub fn save_blp<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        let encoder = BLPEncoder::from_image(&self.0)?;
        encoder.save(path)
    }

    /// Extract alpha channel as grayscale bytes
    ///
    /// Returns a Vec<u8> where each byte is the alpha value (0-255)
    /// for use as an alpha mask or shadow map.
    pub fn extract_alpha_mask(&self) -> Vec<u8> {
        self.pixels().map(|p| p[3]).collect()
    }
}

/// From BLP bytes to decoded image
impl TryFrom<&[u8]> for BLPImage {
    type Error = BLPError;

    fn try_from(data: &[u8]) -> Result<Self> {
        decode_blp(data).map(BLPImage)
    }
}

/// From owned ImageBuffer to BLPImage
impl From<ImageBuffer<Rgba<u8>, Vec<u8>>> for BLPImage {
    fn from(img: ImageBuffer<Rgba<u8>, Vec<u8>>) -> Self {
        BLPImage(img)
    }
}

/// From BLP bytes to decoded image (owned)
impl TryFrom<Vec<u8>> for BLPImage {
    type Error = BLPError;

    fn try_from(data: Vec<u8>) -> Result<Self> {
        decode_blp(&data).map(BLPImage)
    }
}

/// From file path to decoded image
impl TryFrom<&std::path::Path> for BLPImage {
    type Error = BLPError;

    fn try_from(path: &std::path::Path) -> Result<Self> {
        let data = std::fs::read(path)?;
        decode_blp(&data).map(BLPImage)
    }
}

/// From BLP image to PNG bytes
impl TryFrom<BLPImage> for Vec<u8> {
    type Error = BLPError;

    fn try_from(img: BLPImage) -> Result<Self> {
        img.encode_png()
    }
}

/// From decoded image to PNG bytes (convenience)
impl From<BLPImage> for ImageBuffer<Rgba<u8>, Vec<u8>> {
    fn from(img: BLPImage) -> Self {
        img.into_image()
    }
}

/// BLP Decoder from String path
impl TryFrom<String> for BLPDecoder {
    type Error = BLPError;

    fn try_from(path: String) -> Result<Self> {
        BLPDecoder::from_path(&path)
    }
}

fn decode_blp(data: &[u8]) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>> {
    decode_blp_mipmap(data, 0)
}

fn decode_blp_mipmap(
    data: &[u8],
    level: usize,
) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>> {
    let header = BLP1Header::parse(data)?;
    decode_blp_mipmap_with_header(&header, data, level)
}

fn decode_blp_mipmap_with_header(
    header: &BLP1Header,
    data: &[u8],
    level: usize,
) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>> {
    if level >= 16 {
        return Err(BLPError::CorruptedData("mipmap level out of range"));
    }

    match header.content_type() {
        Some(ContentType::Direct) => crate::decoder::decode_direct(header, data, level),
        Some(ContentType::JPEG) => crate::decoder::decode_jpeg(header, data, level),
        None => Err(BLPError::CorruptedData("invalid content type")),
    }
}

fn decode_blp_with_header(
    header: &BLP1Header,
    data: &[u8],
) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>> {
    decode_blp_mipmap_with_header(header, data, 0)
}

fn decode_blp_all_mipmaps(
    data: &[u8],
) -> Result<Vec<ImageBuffer<Rgba<u8>, Vec<u8>>>> {
    let header = BLP1Header::parse(data)?;
    decode_blp_all_mipmaps_with_header(&header, data)
}

fn decode_blp_all_mipmaps_with_header(
    header: &BLP1Header,
    data: &[u8],
) -> Result<Vec<ImageBuffer<Rgba<u8>, Vec<u8>>>> {
    let levels: Vec<usize> = (0..16)
        .filter(|&i| header.mipmap_offsets[i] != 0 && header.mipmap_sizes[i] != 0)
        .collect();

    if levels.is_empty() {
        return Err(BLPError::CorruptedData("no valid mipmaps found"));
    }

    let results: Vec<Result<ImageBuffer<Rgba<u8>, Vec<u8>>>> = levels
        .into_par_iter()
        .map(|level| decode_blp_mipmap_with_header(header, data, level))
        .collect();

    let mut mipmaps = Vec::with_capacity(results.len());
    for result in results {
        mipmaps.push(result?);
    }

    Ok(mipmaps)
}
