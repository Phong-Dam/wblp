use wblp::{BLPDecoder, encode_rgba_to_blp};

fn main() {
    let blp_path = std::path::Path::new("test.blp");
    let output_path = std::path::Path::new("test_blp.blp");

    let decoder = BLPDecoder::from_path(blp_path).unwrap();
    let img = decoder.decode().unwrap();
    let img_buf = img.into_image();

    println!("Encoding: {}x{}", img_buf.width(), img_buf.height());

    let bytes = encode_rgba_to_blp(&img_buf, 85).unwrap();

    std::fs::write(output_path, &bytes).unwrap();
    println!("Created: {} ({} bytes)", output_path.display(), bytes.len());
}