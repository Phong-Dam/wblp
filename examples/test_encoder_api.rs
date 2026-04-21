use wblp::{BLPEncoder, BLPDecoder};

fn main() {
    // Test 1: Decode BLP then encode with method chaining
    println!("=== Test 1: Method Chaining API ===");
    let decoder = BLPDecoder::from_path("test.blp").expect("Failed to open test.blp");
    let img = decoder.decode().expect("Failed to decode");
    let img_buf = img.into_image();

    let blp_bytes = BLPEncoder::from_image(&img_buf)
        .expect("Failed to create encoder")
        .quality(85)
        .mipmaps(true)
        .encode()
        .expect("Failed to encode");

    println!("Encoded: {} bytes", blp_bytes.len());

    // Test 2: Decode the result to verify
    println!("\n=== Test 2: Decode Verification ===");
    let decoder2 = BLPDecoder::from_blp_bytes(&blp_bytes).expect("Failed to create decoder");
    let dims = decoder2.dimensions().expect("Failed to get dimensions");
    println!("Decoded dimensions: {}x{}", dims.0, dims.1);

    // Test 3: Save to file
    println!("\n=== Test 3: Save to File ===");
    BLPEncoder::from_image(&img_buf)
        .expect("Failed to create encoder")
        .quality(85)
        .save("test_blp.blp")
        .expect("Failed to save");
    println!("Saved to test_blp.blp");

    // Test 4: Info
    println!("\n=== Test 4: Encoder Info ===");
    let info = BLPEncoder::from_image(&img_buf)
        .expect("Failed to create encoder")
        .info();
    println!("Encoder info: {}", info);

    println!("\nAll tests passed!");
}