#[inline(always)]
pub fn pack_rgba_to_rgb_fast(src: &[u8], w: usize, h: usize) -> (Vec<u8>, usize) {
    debug_assert_eq!(src.len(), w * h * 4);
    let mut out = vec![0u8; w * h * 3];
    let mut si = 0usize;
    let mut di = 0usize;
    while si < src.len() {
        out[di] = src[si];
        out[di + 1] = src[si + 1];
        out[di + 2] = src[si + 2];
        si += 4;
        di += 3;
    }
    (out, w * 3)
}

#[inline(always)]
pub fn pack_rgba_to_cmyk_fast(src: &[u8], w: usize, h: usize) -> (Vec<u8>, usize) {
    debug_assert_eq!(src.len(), w * h * 4);
    let mut out = vec![0u8; w * h * 4];
    let mut si = 0usize;
    let mut di = 0usize;
    while si < src.len() {
        out[di] = src[si + 2]; // C ← B
        out[di + 1] = src[si + 1]; // M ← G
        out[di + 2] = src[si]; // Y ← R
        out[di + 3] = src[si + 3]; // K ← A
        si += 4;
        di += 4;
    }
    (out, w * 4)
}