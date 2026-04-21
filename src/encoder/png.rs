use image::{
    codecs::png::PngEncoder,
    ImageBuffer, ImageEncoder, Rgba,
};
use std::io::{BufWriter, Write};
use std::ops::Deref;
use std::path::Path;
use std::fs::File;

use crate::error::{BLPError, Result};

/// Encode an RGBA image buffer as PNG bytes
pub fn encode_png<I: Deref<Target = [u8]>>(img: &ImageBuffer<Rgba<u8>, I>) -> Result<Vec<u8>> {
    let (width, height) = img.dimensions();
    let mut output = Vec::with_capacity((width * height) as usize);
    let encoder = PngEncoder::new(&mut output);
    encoder
        .write_image(img.as_raw(), width, height, image::ExtendedColorType::Rgba8)
        .map_err(|e| BLPError::EncodeFailed(e.to_string()))?;
    Ok(output)
}

/// Save an RGBA image buffer as a PNG file directly
pub fn save_png<P: AsRef<Path>, I: Deref<Target = [u8]>>(
    img: &ImageBuffer<Rgba<u8>, I>,
    path: P,
) -> Result<()> {
    let file = File::create(path)
        .map_err(|e| BLPError::EncodeFailed(e.to_string()))?;
    let mut writer = BufWriter::new(file);
    let (width, height) = img.dimensions();
    let encoder = PngEncoder::new(&mut writer);
    encoder
        .write_image(img.as_raw(), width, height, image::ExtendedColorType::Rgba8)
        .map_err(|e| BLPError::EncodeFailed(e.to_string()))?;
    writer.flush()
        .map_err(|e| BLPError::EncodeFailed(e.to_string()))?;
    Ok(())
}