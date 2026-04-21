use image::{ImageBuffer, Rgba};
use std::io::Cursor;
use zune_jpeg::JpegDecoder;
use zune_core::options::DecoderOptions as ZuneOptions;
use crate::error::{BLPError, Result};
use crate::decoder::header::BLP1Header;

pub fn decode_jpeg(
    header: &BLP1Header,
    data: &[u8],
    level: usize,
) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>> {
    let width = (header.width >> level).max(1);
    let height = (header.height >> level).max(1);

    let palette_offset = header.palette_offset(); // = 156

    if data.len() < palette_offset + 4 {
        return Err(BLPError::CorruptedData("BLP file too small for JPEG header size"));
    }

    // Read JPEG header size (little-endian u32)
    let jpeg_header_size = u32::from_le_bytes([
        data[palette_offset],
        data[palette_offset + 1],
        data[palette_offset + 2],
        data[palette_offset + 3],
    ]) as usize;

    let jpeg_header_start = palette_offset + 4;
    let jpeg_header_end = jpeg_header_start + jpeg_header_size;

    if data.len() < jpeg_header_end {
        return Err(BLPError::CorruptedData("BLP file too small for JPEG header"));
    }

    if level >= 16 {
        return Err(BLPError::InvalidMipmapOffset(level));
    }

    let mip_offset = header.mipmap_offsets[level] as usize;
    let mip_size = header.mipmap_sizes[level] as usize;

    if mip_offset == 0 || mip_size == 0 {
        return Err(BLPError::InvalidMipmapOffset(level));
    }

    if data.len() < mip_offset + mip_size {
        return Err(BLPError::CorruptedData("BLP file too small for mipmap data"));
    }


    let jpeg_header = &data[jpeg_header_start..jpeg_header_end];
    let mip_data = &data[mip_offset..mip_offset + mip_size];


    let mut jpeg_data = Vec::with_capacity(jpeg_header_size + mip_size);
    jpeg_data.extend_from_slice(jpeg_header);
    jpeg_data.extend_from_slice(mip_data);


    let options = ZuneOptions::default()
        .jpeg_set_out_colorspace(zune_core::colorspace::ColorSpace::CMYK);

    let mut decoder = JpegDecoder::new_with_options(Cursor::new(jpeg_data.as_slice()), options);

    decoder.decode_headers()
        .map_err(|e| BLPError::JpegDecodeFailed(format!("JPEG header decode failed: {e}")))?;

    let pixels = decoder.decode()
        .map_err(|e| BLPError::JpegDecodeFailed(format!("JPEG decode failed: {e}")))?;

    let expected_len = (width * height) as usize;
    let bytes_per_pixel = if pixels.len() == expected_len * 4 {
        4
    } else if pixels.len() == expected_len * 3 {
        3
    } else {
        return Err(BLPError::JpegDecodeFailed(format!(
            "Unexpected JPEG output size: got {} bytes, expected {} for {}x{} (3 or 4 bpp)",
            pixels.len(),
            expected_len * 4,
            width,
            height
        )));
    };

    let mut imgbuf: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);
    let force_opaque = header.alpha_bitdepth == 0;

    match bytes_per_pixel {
        4 => {

            for (chunk, px) in pixels.chunks_exact(4).zip(imgbuf.pixels_mut()) {
                let a = if force_opaque { 255 } else { chunk[3] };
                *px = Rgba([
                    chunk[2], // R = original B
                    chunk[1], // G = original G
                    chunk[0], // B = original R
                    a,      // A = opaque
                ]);
            }
        }
        3 => {

            for (chunk, px) in pixels.chunks_exact(3).zip(imgbuf.pixels_mut()) {
                *px = Rgba([
                    chunk[2], // R = original B
                    chunk[1], // G = original G
                    chunk[0], // B = original R
                    255,      // A = opaque
                ]);
            }
        }
        _ => {
            return Err(BLPError::JpegDecodeFailed(format!(
                "Unsupported bytes per pixel: {}",
                bytes_per_pixel
            )));
        }
    }

    Ok(imgbuf)
}
