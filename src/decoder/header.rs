use crate::error::{BLPError, Result};

/// BLP1 Header (156 bytes / 0x9C)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BLP1Header {
    pub magic: [u8; 4],          // 0x00: "BLP1"
    pub content: u32,              // 0x04: 0=JPEG, 1=Direct
    pub alpha_bitdepth: u32,       // 0x08: Alpha bits per pixel: 0, 1, 4, or 8
    pub width: u32,               // 0x0C: Image width in pixels
    pub height: u32,              // 0x10: Image height in pixels
    pub extra: u32,               // 0x14: Typically 4 or 5
    pub has_mipmaps: u32,         // 0x18: 0=no mipmaps, nonzero=present
    pub mipmap_offsets: [u32; 16], // 0x1C: Byte offsets to each mipmap level
    pub mipmap_sizes: [u32; 16],   // 0x5C: Byte sizes of each mipmap level
}

/// Content encoding type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    JPEG = 0,
    Direct = 1,
}

impl ContentType {
    pub fn from_u32(val: u32) -> Option<Self> {
        match val {
            0 => Some(Self::JPEG),
            1 => Some(Self::Direct),
            _ => None,
        }
    }
}

impl BLP1Header {
    pub const HEADER_SIZE: usize = 156;
    pub const PALETTE_REGION_SIZE: usize = 1024;

    /// Validate and parse header from little-endian binary data
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < Self::HEADER_SIZE {
            return Err(BLPError::CorruptedData("insufficient header data"));
        }

        // Verify magic is "BLP1"
        let magic = [data[0], data[1], data[2], data[3]];
        if &magic != b"BLP1" {
            return Err(BLPError::InvalidMagic(magic));
        }

        // SAFETY: data.len() >= 156 ensures we read within bounds.
        // BLP1Header is repr(C) with known layout matching the binary format.
        let header = unsafe {
            let ptr = data.as_ptr().cast::<BLP1Header>();
            ptr.read_unaligned()
        };

        // Validate dimensions (prevent allocation attacks)
        if header.width == 0 || header.height == 0 {
            return Err(BLPError::CorruptedData("zero dimensions"));
        }

        if header.width > 8192 || header.height > 8192 {
            return Err(BLPError::CorruptedData("dimensions too large"));
        }

        // Validate alpha bit depth (cap at 8 if >= 8)
        let alpha_bitdepth = if header.alpha_bitdepth >= 8 { 8 } else { header.alpha_bitdepth };
        match alpha_bitdepth {
            0 | 1 | 4 | 8 => {}
            _ => return Err(BLPError::UnsupportedAlpha(header.alpha_bitdepth)),
        }

        Ok(header)
    }

    /// Get the content encoding type
    pub fn content_type(&self) -> Option<ContentType> {
        ContentType::from_u32(self.content)
    }

    /// Calculate the number of pixels for the base mipmap level (level 0)
    pub fn base_pixel_count(&self) -> u32 {
        self.width * self.height
    }

    /// Get the file offset where palette/JPEG header region starts
    pub fn palette_offset(&self) -> usize {
        Self::HEADER_SIZE
    }

    /// Get the mipmap data offset for a given level
    pub fn mipmap_offset(&self, level: usize) -> Option<u32> {
        if level >= 16 {
            return None;
        }
        let offset = self.mipmap_offsets[level];
        if offset == 0 {
            None
        } else {
            Some(offset)
        }
    }

    /// Get the mipmap data size for a given level
    pub fn mipmap_size(&self, level: usize) -> Option<u32> {
        if level >= 16 {
            return None;
        }
        let size = self.mipmap_sizes[level];
        if size == 0 {
            None
        } else {
            Some(size)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_size() {
        assert_eq!(std::mem::size_of::<BLP1Header>(), 156);
    }
}
