use crate::error::BLPError;
use super::read_be_u16::read_be_u16;

/// Rebuild minimal JPEG header: SOI + essential segments + SOF + SOS
pub fn rebuild_minimal_jpeg_header(header: &[u8]) -> std::result::Result<Vec<u8>, BLPError> {
    if header.len() < 4 || header[0] != 0xFF || header[1] != 0xD8 {
        return Err(BLPError::EncodeFailed("jpeg.bad_soi".to_string()));
    }

    let mut pos = 2usize;
    let mut others: Vec<(usize, usize)> = Vec::new();
    let mut sof_seg: Option<(usize, usize)> = None;
    let mut sos_seg: Option<(usize, usize)> = None;

    while pos < header.len() {
        while pos < header.len() && header[pos] == 0xFF {
            pos += 1;
        }
        if pos >= header.len() {
            break;
        }

        let id = header[pos];
        let start = pos - 1;
        pos += 1;

        // Stand-alone markers: TEM (0x01) and RST0..RST7 (0xD0..0xD7)
        if id == 0x01 || (0xD0..=0xD7).contains(&id) {
            others.push((start, pos));
            continue;
        }

        // Markers with length (2 bytes BE after id)
        if pos + 2 > header.len() {
            return Err(BLPError::EncodeFailed("jpeg.seg_len".to_string()));
        }
        let seg_len = read_be_u16(&header[pos..pos + 2])? as usize;
        let end = pos + seg_len;
        if end > header.len() {
            return Err(BLPError::EncodeFailed("jpeg.seg_trunc".to_string()));
        }

        if id == 0xDA {
            // SOS - last in header, include and break
            sos_seg = Some((start, end));
            break;
        } else if (0xE0..=0xEF).contains(&id) || id == 0xFE {
            // APPn and COM - skip
        } else if (0xC0..=0xCF).contains(&id) && id != 0xC4 && id != 0xC8 {
            // SOF* (except DHT=C4 and JPG=C8): take only first
            if sof_seg.is_none() {
                sof_seg = Some((start, end));
            }
        } else {
            others.push((start, end));
        }

        pos = end;
    }

    let (sos_s, sos_e) = sos_seg.ok_or_else(|| BLPError::EncodeFailed("jpeg.sos_missing".to_string()))?;
    let (sof_s, sof_e) = sof_seg.ok_or_else(|| BLPError::EncodeFailed("jpeg.sof_missing".to_string()))?;

    let mut out = Vec::with_capacity(header.len());
    out.extend_from_slice(&header[..2]); // SOI
    for (s, e) in others {
        out.extend_from_slice(&header[s..e]);
    }
    out.extend_from_slice(&header[sof_s..sof_e]); // first SOF
    out.extend_from_slice(&header[sos_s..sos_e]); // SOS
    Ok(out)
}