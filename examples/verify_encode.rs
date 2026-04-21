use wblp::{BLPDecoder, encode_rgba_to_blp};

fn main() {
    let blp_path = std::path::Path::new("test.blp");
    let output_path = std::path::Path::new("test_blp.blp");

    // Decode original
    let decoder = BLPDecoder::from_path(blp_path).unwrap();
    let img = decoder.decode().unwrap();
    let img_buf = img.into_image();
    println!("Original: {}x{}", img_buf.width(), img_buf.height());

    // Encode and save
    let bytes = encode_rgba_to_blp(&img_buf, 85).unwrap();
    std::fs::write(output_path, &bytes).unwrap();
    println!("Encoded: {} ({} bytes)", output_path.display(), bytes.len());

    // Decode the encoded file to verify
    let decoder2 = BLPDecoder::from_path(output_path).unwrap();
    let img2 = decoder2.decode().unwrap();
    let img_buf2 = img2.into_image();
    println!("Re-decoded: {}x{}", img_buf2.width(), img_buf2.height());

    if img_buf.width() == img_buf2.width() && img_buf.height() == img_buf2.height() {
        println!("SUCCESS: Dimensions match!");
    } else {
        println!("FAIL: Dimensions don't match!");
    }
}