use image::{ImageBuffer, Rgba};
use crate::error::{BLPError, Result};
use crate::decoder::header::BLP1Header;
use crate::decoder::palette::Palette;

/// Decode 1-bit alpha data: 8 pixels packed per byte, LSB first
fn decode_alpha_1bit(data: &[u8], pixel_count: usize) -> Vec<u8> {
    // Pre-allocate once - enables LLVM auto-vectorization
    let mut alpha = vec![0u8; pixel_count];
    for i in 0..pixel_count {
        // Branchless: multiply by 0xFF to expand 0/1 to 0/255
        alpha[i] = ((data[i / 8] >> (i % 8)) & 1) * 0xFF;
    }
    alpha
}

/// Decode 4-bit alpha data: 2 pixels per byte
fn decode_alpha_4bit(data: &[u8], pixel_count: usize) -> Vec<u8> {
    let mut alpha = vec![0u8; pixel_count];
    for i in 0..pixel_count {
        let byte_idx = i / 2;
        let shift = if i % 2 == 0 { 4 } else { 0 };
        let value = (data[byte_idx] >> shift) & 0x0F;
        alpha[i] = value.wrapping_mul(17); // Scale 0-15 to 0-255
    }
    alpha
}

/// Decode 8-bit alpha data: direct values
fn decode_alpha_8bit(data: &[u8], _pixel_count: usize) -> Vec<u8> {
    data.to_vec()
}

/// Decode Direct (palettized) content with optional alpha at specified mipmap level
pub fn decode_direct(
    header: &BLP1Header,
    data: &[u8],
    level: usize,
) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>> {
    // Get dimensions for this mipmap level
    let width = (header.width >> level).max(1);
    let height = (header.height >> level).max(1);
    let pixel_count = (width * height) as usize;

    // Parse palette
    let palette = Palette::parse(&data[header.palette_offset()..])
        .map_err(|_| BLPError::CorruptedData("failed to parse palette"))?;

    // Get mipmap offset and size for specified level
    if level >= 16 {
        return Err(BLPError::InvalidMipmapOffset(level));
    }

    let mip_offset = header.mipmap_offsets[level] as usize;
    let mip_size = header.mipmap_sizes[level] as usize;

    if mip_offset == 0 || mip_size == 0 {
        return Err(BLPError::InvalidMipmapOffset(level));
    }

    if data.len() < mip_offset + mip_size {
        return Err(BLPError::CorruptedData("insufficient mipmap data"));
    }

    let mip_data = &data[mip_offset..mip_offset + mip_size];
    let index_data = &mip_data[..pixel_count.min(mip_data.len())];

    // Prepare output buffer
    let mut pixels = vec![0u8; pixel_count * 4];
    let mut output_idx = 0;

    // Decode alpha if present (alpha bitdepth capped at 8)
    let alpha_bitdepth = header.alpha_bitdepth.min(8);
    let alpha_data = if alpha_bitdepth > 0 {
        let alpha_offset = pixel_count;
        let alpha_size = match alpha_bitdepth {
            1 => (pixel_count + 7) / 8,
            4 => (pixel_count + 1) / 2,
            8 => pixel_count,
            _ => return Err(BLPError::UnsupportedAlpha(alpha_bitdepth)),
        };

        if mip_data.len() < alpha_offset + alpha_size {
            return Err(BLPError::CorruptedData("insufficient alpha data"));
        }

        let raw_alpha = &mip_data[alpha_offset..alpha_offset + alpha_size];
        match alpha_bitdepth {
            1 => decode_alpha_1bit(raw_alpha, pixel_count),
            4 => decode_alpha_4bit(raw_alpha, pixel_count),
            8 => decode_alpha_8bit(raw_alpha, pixel_count),
            _ => return Err(BLPError::UnsupportedAlpha(alpha_bitdepth)),
        }
    } else {
        vec![0xFF; pixel_count]
    };

    // Decode pixels - map palette indices to RGBA
    for (i, &index) in index_data.iter().enumerate() {
        let color = palette.lookup(index);
        let alpha = alpha_data.get(i).copied().unwrap_or(0xFF);

        pixels[output_idx] = color.red;
        pixels[output_idx + 1] = color.green;
        pixels[output_idx + 2] = color.blue;
        pixels[output_idx + 3] = alpha;

        output_idx += 4;
    }

    ImageBuffer::from_raw(width, height, pixels)
        .ok_or_else(|| BLPError::CorruptedData("failed to create image buffer"))
}
