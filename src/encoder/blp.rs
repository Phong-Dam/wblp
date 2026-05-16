//! BLP1 encoder with mipmap support
//!
//! ## Encoding API (Method Chaining)
//!
//! ```rust
//! use wblp::BLPEncoder;
//!
//! // Method chaining style
//! let blp_bytes = BLPEncoder::from_path("input.png")?
//!     .quality(85)
//!     .encode()?;
//!
//! // With output file
//! BLPEncoder::from_path("input.png")?
//!     .quality(90)
//!     .save("output.blp")?;
//!
//! // From RGBA ImageBuffer
//! BLPEncoder::from_image(&rgba_img)?
//!     .quality(85)
//!     .encode()?;
//! ```

use crate::error::BLPError;
use image::{ImageBuffer, Rgba, imageops};
use std::ffi::CStr;
use std::path::Path;
use std::ptr;
use turbojpeg::{libc, raw};

use crate::encoder::utils::pack_rgba_to_cmyk_fast::pack_rgba_to_cmyk_fast;
use crate::encoder::utils::read_be_u16::read_be_u16;
use crate::encoder::utils::rebuild_minimal_jpeg_header::rebuild_minimal_jpeg_header;

const MAX_MIPS: usize = 16;

#[derive(Clone)]
pub struct Mip {
    pub w: u32,
    pub h: u32,
    pub visible: bool,
    pub encode_ms: f64,
}

pub struct Ctx {
    pub bytes: Vec<u8>,
    pub mips: Vec<Mip>,
    pub has_alpha: bool,
    pub encode_ms_total: f64,
}

/// BLP Encoder with method chaining API
///
/// # Example
/// ```rust
/// use wblp::BLPEncoder;
///
/// let encoder = BLPEncoder::from_path("input.png")?
///     .quality(85)
///     .mipmaps(true);
///
/// let blp_bytes = encoder.encode()?;
/// ```
#[derive(Clone)]
pub struct BLPEncoder {
    img: Option<ImageBuffer<Rgba<u8>, Vec<u8>>>,
    quality: u8,
    generate_mipmaps: bool,
}

impl BLPEncoder {
    /// Create encoder from image file path
    ///
    /// Supports: PNG, JPEG, BMP, GIF, TIFF, WebP, and more
    pub fn from_path<P: AsRef<Path>>(path: P) -> std::result::Result<Self, BLPError> {
        let img = image::open(path.as_ref())
            .map_err(|e| BLPError::EncodeFailed(format!("failed to open image: {}", e)))?;
        Ok(Self {
            img: Some(img.to_rgba8()),
            quality: 85,
            generate_mipmaps: true,
        })
    }

    /// Create encoder from RGBA ImageBuffer
    pub fn from_image(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> std::result::Result<Self, BLPError> {
        Ok(Self {
            img: Some(img.clone()),
            quality: 85,
            generate_mipmaps: true,
        })
    }

    /// Create encoder from raw RGBA pixels
    pub fn from_pixels(width: u32, height: u32, pixels: &[u8]) -> std::result::Result<Self, BLPError> {
        let expected = (width as usize) * (height as usize) * 4;
        if pixels.len() != expected {
            return Err(BLPError::EncodeFailed(format!(
                "pixel buffer size mismatch: expected {} bytes ({}x{}x4), got {}",
                expected, width, height, pixels.len()
            )));
        }
        let img = ImageBuffer::from_raw(width, height, pixels.to_vec())
            .ok_or_else(|| BLPError::EncodeFailed("invalid pixel dimensions".to_string()))?;
        Ok(Self {
            img: Some(img),
            quality: 85,
            generate_mipmaps: true,
        })
    }

    /// Create encoder from image bytes (PNG, JPEG, etc.)
    ///
    /// Useful when you have image data in memory as bytes
    pub fn from_image_bytes(data: &[u8]) -> std::result::Result<Self, BLPError> {
        let img = image::load_from_memory(data)
            .map_err(|e| BLPError::EncodeFailed(format!("failed to decode image: {}", e)))?;
        Ok(Self {
            img: Some(img.to_rgba8()),
            quality: 85,
            generate_mipmaps: true,
        })
    }

    /// Set JPEG quality (1-100, default: 85)
    pub fn quality(mut self, quality: u8) -> Self {
        self.quality = quality;
        self
    }

    /// Enable/disable mipmap generation (default: true)
    pub fn mipmaps(mut self, enabled: bool) -> Self {
        self.generate_mipmaps = enabled;
        self
    }

    /// Encode to BLP bytes
    pub fn encode(&self) -> std::result::Result<Vec<u8>, BLPError> {
        let img = self.img.as_ref()
            .ok_or_else(|| BLPError::EncodeFailed("no image loaded".to_string()))?;
        encode_rgba_to_blp_with_mipmaps(img, self.quality, self.generate_mipmaps)
    }

    /// Encode and save to file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> std::result::Result<(), BLPError> {
        let bytes = self.encode()?;
        std::fs::write(path.as_ref(), &bytes)
            .map_err(|e| BLPError::IoError(e))?;
        Ok(())
    }

    /// Get encoder info as string
    pub fn info(&self) -> String {
        match &self.img {
            Some(img) => format!("{}x{}, quality={}, mipmaps={}",
                img.width(), img.height(), self.quality, self.generate_mipmaps),
            None => "no image".to_string(),
        }
    }
}

/// Check if image has alpha channel
fn has_alpha(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> bool {
    img.pixels().any(|p| p[3] != 255)
}

/// Encode RGBA to BLP with configurable mipmaps
fn encode_rgba_to_blp_with_mipmaps(
    img: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    quality: u8,
    generate_mipmaps: bool,
) -> std::result::Result<Vec<u8>, BLPError> {
    let alpha = has_alpha(img);

    let mip_images: Vec<ImageBuffer<Rgba<u8>, Vec<u8>>> = if generate_mipmaps {
        generate_mipmap_chain(img)
    } else {
        vec![img.clone()]
    };

    let mut encoded_mips: Vec<Vec<u8>> = Vec::new();
    for mip_img in &mip_images {
        let jpeg_data = compress_rgba_to_jpeg(mip_img, quality)?;
        encoded_mips.push(jpeg_data);
    }

    build_blp1(img.dimensions(), &encoded_mips, alpha)
}

/// Generate mipmap chain from base image
fn generate_mipmap_chain(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> Vec<ImageBuffer<Rgba<u8>, Vec<u8>>> {
    let mut mips = Vec::new();
    let mut w = img.width();
    let mut h = img.height();

    mips.push(img.clone());

    loop {
        if w == 1 && h == 1 {
            break;
        }
        w = (w / 2).max(1);
        h = (h / 2).max(1);

        let resized = imageops::resize(&mips[0], w, h, imageops::FilterType::Triangle);
        mips.push(resized);

        if mips.len() >= MAX_MIPS {
            break;
        }
    }

    mips
}

/// Encode any image file to BLP bytes
pub fn encode_file_to_blp<P: AsRef<Path>>(path: P, quality: u8) -> std::result::Result<Vec<u8>, BLPError> {
    let img = image::open(path.as_ref())
        .map_err(|e| BLPError::EncodeFailed(format!("failed to open image: {}", e)))?;
    let rgba = img.to_rgba8();
    encode_rgba_to_blp(&rgba, quality)
}

/// Encode and save to BLP file directly
///
/// # Errors
/// Returns `BLPError::EncodeFailed` on encode failure,
/// `BLPError::IoError` on file write failure
///
/// # Example
/// ```rust
/// encode_to_blp_file("input.png", "output.blp", 85)?;
/// ```
pub fn encode_to_blp_file<P: AsRef<Path>, Q: AsRef<Path>>(
    input: P,
    output: Q,
    quality: u8,
) -> std::result::Result<(), BLPError> {
    let bytes = encode_file_to_blp(input, quality)?;
    std::fs::write(output.as_ref(), &bytes)
        .map_err(|e| BLPError::IoError(e))?;
    Ok(())
}

/// Encode RGBA ImageBuffer to BLP bytes
///
/// Automatically generates mipmap chain and detects alpha channel.
///
/// # Arguments
/// * `img` - RGBA image buffer
/// * `quality` - JPEG quality (1-100, recommended 80-95)
///
/// # Example
/// ```rust
/// use wblp::encode_rgba_to_blp;
/// use image::ImageBuffer;
/// use image::Rgba;
///
/// let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(512, 512);
/// let blp_bytes = encode_rgba_to_blp(&img, 85)?;
/// ```
pub fn encode_rgba_to_blp(
    img: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    quality: u8,
) -> std::result::Result<Vec<u8>, BLPError> {
    let has_alpha = img.pixels().any(|p| p[3] != 255);

    let mut mip_images: Vec<ImageBuffer<Rgba<u8>, Vec<u8>>> = Vec::new();
    let mut w = img.width();
    let mut h = img.height();

    loop {
        if mip_images.len() >= MAX_MIPS {
            break;
        }
        if w == 0 || h == 0 {
            break;
        }

        if mip_images.is_empty() {
            mip_images.push(img.clone());
        } else {
            let resized = imageops::resize(
                &mip_images[0],
                w.max(1),
                h.max(1),
                imageops::FilterType::Triangle,
            );
            mip_images.push(resized);
        }

        if w == 1 && h == 1 {
            break;
        }
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }

    let mut encoded_mips: Vec<Vec<u8>> = Vec::new();
    for mip_img in &mip_images {
        let jpeg_data = compress_rgba_to_jpeg(mip_img, quality)?;
        encoded_mips.push(jpeg_data);
    }

    build_blp1(img.dimensions(), &encoded_mips, has_alpha)
}

/// Encode raw RGBA pixels to BLP bytes
///
/// # Arguments
/// * `rgba` - Raw RGBA pixel data
/// * `width` - Image width
/// * `height` - Image height
/// * `quality` - JPEG quality (1-100)
///
/// # Example
/// ```rust
/// let pixels: Vec<u8> = vec![255; 512 * 512 * 4];
/// let blp_bytes = encode_raw_rgba(&pixels, 512, 512, 85)?;
/// ```
pub fn encode_raw_rgba(
    rgba: &[u8],
    width: u32,
    height: u32,
    quality: u8,
) -> std::result::Result<Vec<u8>, BLPError> {
    let img = ImageBuffer::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| BLPError::EncodeFailed("invalid rgba dimensions".to_string()))?;
    encode_rgba_to_blp(&img, quality)
}

fn compress_rgba_to_jpeg(
    img: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    quality: u8,
) -> std::result::Result<Vec<u8>, BLPError> {
    let (w, h) = img.dimensions();
    let wz = w as usize;
    let hz = h as usize;

    let src = img.as_raw();
    let (packed, pitch) = pack_rgba_to_cmyk_fast(src, wz, hz);

    let handle = unsafe { raw::tj3Init(raw::TJINIT_TJINIT_COMPRESS as libc::c_int) };
    if handle.is_null() {
        return Err(BLPError::EncodeFailed("tj3.init failed".to_string()));
    }

    let quality = quality.clamp(1, 100);
    let jpeg_raw = unsafe {
        struct Guard(raw::tjhandle);
        impl Drop for Guard {
            fn drop(&mut self) {
                if !self.0.is_null() {
                    unsafe { raw::tj3Destroy(self.0) };
                }
            }
        }
        let _g = Guard(handle);

        if raw::tj3Set(handle, raw::TJPARAM_TJPARAM_QUALITY as libc::c_int, quality as libc::c_int) != 0 {
            return Err(tj3_err(handle, "tj3.quality"));
        }
        if raw::tj3Set(handle, raw::TJPARAM_TJPARAM_SUBSAMP as libc::c_int, raw::TJSAMP_TJSAMP_444 as libc::c_int) != 0 {
            return Err(tj3_err(handle, "tj3.subsamp"));
        }
        if raw::tj3Set(handle, raw::TJPARAM_TJPARAM_OPTIMIZE as libc::c_int, 0) != 0 {
            return Err(tj3_err(handle, "tj3.optimize"));
        }
        if raw::tj3Set(
            handle,
            raw::TJPARAM_TJPARAM_COLORSPACE as libc::c_int,
            raw::TJCS_TJCS_CMYK as libc::c_int,
        ) != 0
        {
            return Err(tj3_err(handle, "tj3.colorspace"));
        }

        let mut out_ptr: *mut libc::c_uchar = ptr::null_mut();
        let mut out_size: raw::size_t = 0;
        let r = raw::tj3Compress8(
            handle,
            packed.as_ptr(),
            wz as libc::c_int,
            pitch as libc::c_int,
            hz as libc::c_int,
            raw::TJPF_TJPF_CMYK as libc::c_int,
            &mut out_ptr,
            &mut out_size,
        );
        if r != 0 {
            return Err(tj3_err(handle, "tj3.compress"));
        }
        let slice = std::slice::from_raw_parts(out_ptr, out_size as usize);
        let vec = slice.to_vec();
        raw::tj3Free(out_ptr as *mut libc::c_void);
        vec
    };

    let (head_len, _) = split_header_and_scan(&jpeg_raw)?;
    let header_clean = rebuild_minimal_jpeg_header(&jpeg_raw[..head_len])?;

    let mut out = Vec::with_capacity(jpeg_raw.len());
    out.extend_from_slice(&header_clean);
    out.extend_from_slice(&jpeg_raw[head_len..]);

    Ok(out)
}

fn build_blp1(
    (w, h): (u32, u32),
    encoded_mips: &[Vec<u8>],
    has_alpha: bool,
) -> std::result::Result<Vec<u8>, BLPError> {
    if encoded_mips.is_empty() {
        return Err(BLPError::EncodeFailed("no_mips".to_string()));
    }

    // Find common header prefix across all mips
    let common_header = find_common_prefix(encoded_mips);

    // Verify all mips share this prefix
    for (i, enc) in encoded_mips.iter().enumerate() {
        if !enc.starts_with(&common_header) {
            return Err(BLPError::EncodeFailed(format!("mip{}_header_mismatch", i)));
        }
    }

    let common_header_len = common_header.len() as u32;

    let mut bytes = Vec::new();

    // BLP1 header (156 bytes)
    bytes.extend_from_slice(b"BLP1");
    bytes.extend_from_slice(&0u32.to_le_bytes()); // compression = 0 (JPEG)
    bytes.extend_from_slice(&(if has_alpha { 8u32 } else { 0u32 }).to_le_bytes()); // flags
    bytes.extend_from_slice(&u32::to_le_bytes(w));
    bytes.extend_from_slice(&u32::to_le_bytes(h));
    bytes.extend_from_slice(&0u32.to_le_bytes()); // extra
    bytes.extend_from_slice(&(if encoded_mips.len() > 1 { 1u32 } else { 0u32 }).to_le_bytes()); // has_mipmaps

    // Mipmap offsets (16 entries)
    let pos_offsets = bytes.len();
    bytes.resize(bytes.len() + MAX_MIPS * 4, 0);

    // Mipmap sizes (16 entries)
    let pos_sizes = bytes.len();
    bytes.resize(bytes.len() + MAX_MIPS * 4, 0);

    // JPEG header size
    bytes.extend_from_slice(&common_header_len.to_le_bytes());

    // JPEG header
    bytes.extend_from_slice(&common_header);

    // Encode each mip and write payload
    for (i, enc) in encoded_mips.iter().enumerate() {
        if i >= MAX_MIPS {
            break;
        }
        let payload = &enc[common_header.len()..];
        let offset = bytes.len();
        let size = payload.len();

        // Write offset
        let off_pos = pos_offsets + i * 4;
        bytes[off_pos..off_pos + 4].copy_from_slice(&(offset as u32).to_le_bytes());

        // Write size
        let sz_pos = pos_sizes + i * 4;
        bytes[sz_pos..sz_pos + 4].copy_from_slice(&(size as u32).to_le_bytes());

        bytes.extend_from_slice(payload);
    }

    Ok(bytes)
}

fn find_common_prefix(mips: &[Vec<u8>]) -> Vec<u8> {
    if mips.is_empty() {
        return Vec::new();
    }
    let first = &mips[0];
    let mut prefix_len = 0;
    for i in 0..first.len() {
        let b = first[i];
        if mips.iter().all(|m| m.get(i) == Some(&b)) {
            prefix_len = i + 1;
        } else {
            break;
        }
    }
    first[..prefix_len].to_vec()
}

fn split_header_and_scan(jpeg: &[u8]) -> std::result::Result<(usize, usize), BLPError> {
    if jpeg.len() < 4 || jpeg[0] != 0xFF || jpeg[1] != 0xD8 {
        return Err(BLPError::EncodeFailed("jpeg.bad_soi".to_string()));
    }
    let mut i = 2usize;
    loop {
        while i < jpeg.len() && jpeg[i] == 0xFF {
            i += 1;
        }
        if i >= jpeg.len() {
            return Err(BLPError::EncodeFailed("jpeg.truncated".to_string()));
        }
        let m = jpeg[i];
        i += 1;
        match m {
            0xD9 => return Err(BLPError::EncodeFailed("jpeg.eoi_before_sos".to_string())),
            0xD0..=0xD7 | 0x01 => {} // no length
            0xDA => {
                if i + 2 > jpeg.len() {
                    return Err(BLPError::EncodeFailed("jpeg.sos_len".to_string()));
                }
                let seg_len = read_be_u16(&jpeg[i..i + 2])? as usize;
                let seg_end = i + seg_len;
                if seg_end > jpeg.len() {
                    return Err(BLPError::EncodeFailed("jpeg.sos_trunc".to_string()));
                }
                let head_len = seg_end;
                let mut j = head_len;
                while j + 1 < jpeg.len() {
                    if jpeg[j] == 0xFF {
                        let n = jpeg[j + 1];
                        if n == 0x00 || (0xD0..=0xD7).contains(&n) {
                            j += 2;
                            continue;
                        }
                        if n == 0xD9 {
                            return Ok((head_len, j - head_len));
                        }
                    }
                    j += 1;
                }
                return Err(BLPError::EncodeFailed("jpeg.eoi_not_found".to_string()));
            }
            _ => {
                if i + 2 > jpeg.len() {
                    return Err(BLPError::EncodeFailed("jpeg.seg_len".to_string()));
                }
                let seg_len = read_be_u16(&jpeg[i..i + 2])? as usize;
                let seg_end = i + seg_len;
                if seg_end > jpeg.len() {
                    return Err(BLPError::EncodeFailed("jpeg.seg_trunc".to_string()));
                }
                i = seg_end;
            }
        }
    }
}

fn tj3_err(handle: raw::tjhandle, key: &'static str) -> BLPError {
    let msg = unsafe {
        let p = raw::tj3GetErrorStr(handle);
        if p.is_null() {
            "unknown".to_string()
        } else {
            CStr::from_ptr(p).to_string_lossy().into_owned()
        }
    };
    BLPError::EncodeFailed(format!("{}: {}", key, msg))
}