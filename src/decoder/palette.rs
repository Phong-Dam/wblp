/// BGRX palette entry (4 bytes) - matches BLP1 palette format
#[repr(C)]
pub(crate) struct BgrxEntry {
    pub blue: u8,
    pub green: u8,
    pub red: u8,
    pub alpha: u8, // padding, always 0xFF
}

/// RGBA color for output
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RgbaColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl RgbaColor {
    /// Create from BGRX palette entry with full opacity
    #[inline]
    pub(crate) fn from_bgrx(bgr: &BgrxEntry) -> Self {
        Self {
            red: bgr.red,
            green: bgr.green,
            blue: bgr.blue,
            alpha: 0xFF,
        }
    }
}

/// Palette containing 256 RGBA entries
#[derive(Debug, Clone)]
pub struct Palette {
    colors: [RgbaColor; 256],
}

impl Palette {
    /// Parse palette from BLP1 binary data (at offset 0x9C)
    /// Returns error if data is insufficient
    pub fn parse(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 1024 {
            return Err("insufficient palette data");
        }

        // Safely read the BGRX entries
        let bgrx_ptr = data[..1024].as_ptr().cast::<BgrxEntry>();
        let bgrx_entries = unsafe {
            std::slice::from_raw_parts(bgrx_ptr, 256)
        };

        let mut colors = [RgbaColor {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 0xFF,
        }; 256];

        for (i, bgrx) in bgrx_entries.iter().enumerate() {
            colors[i] = RgbaColor::from_bgrx(bgrx);
        }

        Ok(Self { colors })
    }

    /// Look up a color by palette index
    #[inline]
    pub fn lookup(&self, index: u8) -> RgbaColor {
        self.colors[index as usize]
    }
}
