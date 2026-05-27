# wblp

BLP1 image format encoder/decoder written in pure Rust.

BLP is Blizzard's texture format used in Warcraft III.

## Installation

### Library

```toml
[dependencies]
wblp = "0.1.10"
```

### CLI

```bash
cargo install wblp --features "blp2png png2blp"
```

## Quick Start

### Encoding

```rust
use wblp::BLPEncoder;

// Encode image file → BLP bytes
let blp_bytes = BLPEncoder::from_path("texture.png")?
    .quality(85)
    .encode()?;

// Encode and save directly
BLPEncoder::from_path("texture.png")?
    .save("output.blp")?;
```

### Decoding

```rust
use wblp::BLPDecoder;

// Decode BLP → PNG
BLPDecoder::from_path("texture.blp")?
    .decode()?
    .save_png("output.png")?;
```

## BLPEncoder

```rust
use wblp::BLPEncoder;

// From image file
BLPEncoder::from_path("input.png")?
    .quality(90)
    .mipmaps(true)
    .encode()?
    .save("output.blp")?;

// From RGBA ImageBuffer
let img = image::open("texture.png")?.to_rgba8();
BLPEncoder::from_image(&img)?
    .encode()?;

// From raw pixels (validates size)
let pixels: Vec<u8> = vec![255; 512 * 512 * 4];
BLPEncoder::from_pixels(512, 512, &pixels)?
    .encode()?;

// From image bytes (PNG, JPEG, etc.)
let image_bytes = std::fs::read("image.png")?;
BLPEncoder::from_image_bytes(&image_bytes)?
    .encode()?;
```
## BLPDecoder

```rust
use wblp::{BLPDecoder, BLPMetadata, BLPFormat};

// From file
BLPDecoder::from_path("texture.blp")?
    .decode()?
    .save_png("output.png")?;

// From BLP bytes
BLPDecoder::from_blp_bytes(&data)?
    .decode()?
    .to_png_bytes()?;

// All metadata in one call
let meta: BLPMetadata = decoder.metadata()?;
println!("{}x{}, {} mipmaps, alpha={}, format={:?}",
    meta.width, meta.height, meta.mipmaps, meta.has_alpha, meta.format);
```

### Mipmap Levels

```rust
// Specific level
let small = decoder.decode_mipmap(2)?;

// All levels
let mipmaps = decoder.decode_all_mipmaps()?;
```

### BLPImage

```rust
let img = decoder.decode()?;

// Save/Encode
img.save_png("output.png")?;
img.save_blp("output.blp")?;
let png_bytes = img.to_png_bytes()?;  // or img.to_png()
let blp_bytes = img.to_blp()?;

// Raw data
let rgba_bytes = img.as_rgba();
let owned_rgba = img.into_rgba();

// Alpha mask extraction (for shadows/team colors)
let alpha_mask = img.extract_alpha_mask();

// Iterate pixels
for pixel in img.pixels() {
    // ...
}
```

## Traits

```rust
use wblp::BLPImage;

// BLP bytes → BLPImage
let img: BLPImage = std::fs::read("texture.blp")?.try_into()?;

// BLPImage → PNG bytes
let png: Vec<u8> = img.try_into()?;
```

## Error Handling

All operations return `Result<T, BLPError>` and use the `?` operator for error propagation:

```rust
use wblp::{BLPDecoder, BLPEncoder, BLPError};

match encoder.encode() {
    Ok(bytes) => { /* success */ }
    Err(BLPError::EncodeFailed(msg)) => { /* compression failed */ }
    Err(BLPError::IoError(e)) => { /* file I/O error */ }
    Err(BLPError::CorruptedData(msg)) => { /* invalid BLP structure */ }
    Err(e) => { /* other error */ }
}
```

### Error Variants

| Variant | Cause |
|---------|-------|
| `IoError` | File access failures |
| `EncodeFailed` | JPEG compression failures |
| `CorruptedData` | Invalid BLP structure |

## Core Types

### Types

```rust
enum BLPFormat {
    JPEG,
    Direct,
}

struct BLPMetadata {
    width: u32,
    height: u32,
    mipmaps: usize,
    has_alpha: bool,
    format: BLPFormat,
}
```

### BLPEncoder

| Method | Description |
|--------|-------------|
| `from_path(path)` | Load from image file |
| `from_image(img)` | Load from ImageBuffer |
| `from_pixels(w, h, pixels)` | Load from raw RGBA pixels (validates size) |
| `from_image_bytes(data)` | Load from image bytes |
| `.quality(q)` | JPEG quality (1-100, default: 85) |
| `.mipmaps(bool)` | Generate mipmap chain (default: true) |
| `.encode()` | Encode to BLP bytes |
| `.save(path)` | Encode and save to file |

### BLPDecoder

| Method | Description |
|--------|-------------|
| `from_path(path)` | Load from file |
| `from_blp_bytes(data)` | Load from BLP bytes |
| `from_data(data)` | Load from owned bytes |
| `from_reader(reader)` | Load from any `Read` source |
| `decode()` | Decode to BLPImage |
| `decode_mipmap(level)` | Decode specific mipmap level |
| `decode_all_mipmaps()` | Decode all mipmap levels |
| `metadata()` | Get all metadata |

Deprecated (use `metadata()` instead):
- `dimensions()`, `has_alpha()`, `mipmap_count()`, `content_type()`

### BLPImage

| Method | Description |
|--------|-------------|
| `save_png(path)` | Save to PNG file |
| `save_blp(path)` | Save to BLP file |
| `to_png_bytes()` | Get PNG bytes (alias: `to_png()`) |
| `to_blp()` | Get BLP bytes |
| `as_rgba()` | Get raw RGBA bytes reference |
| `into_rgba()` | Get owned raw RGBA bytes |
| `extract_alpha_mask()` | Get alpha channel as grayscale bytes |
| `dimensions()` | Get (width, height) |
| `width()` | Get width |
| `height()` | Get height |
| `pixels()` | Iterate over pixels |

## CLI

The CLI tool converts between BLP and image formats.

### Features & Commands

| Feature | Available Commands |
|---------|-------------------|
| `blp2png` | `to-png`, `blp2-png-dir` |
| `png2blp` | `to-blp`, `png2-blp-dir` |

Build with `--features "blp2png png2blp"` to enable all commands.

### Installation

```bash
# Full featured (both directions)
cargo install wblp --features "blp2png png2blp"

# BLP to PNG only
cargo install wblp --features "blp2png"

# Image to BLP only
cargo install wblp --features "png2blp"

# Build from source
cargo build --release --features "blp2png png2blp"
```

### Commands

**With `blp2png` feature:**
```bash
wblp to-png texture.blp -o output.png
wblp to-png texture.blp -o output.png -m 1
wblp blp2-png-dir ./blp_textures -o ./png_output    # recursive
```

**With `png2blp` feature:**
```bash
wblp to-blp texture.png -o output.blp
wblp to-blp texture.png -o output.blp -q 90 --no-mipmaps
wblp png2-blp-dir ./images -o ./blp_output          # recursive
```

### Options

**Single file (blp2png):**
| Option | Description | Default |
|--------|-------------|---------|
| `-o, --output` | Output PNG file | `output.png` |
| `-m, --mipmap` | Mipmap level (0=base) | `0` |

**Single file (png2blp):**
| Option | Description | Default |
|--------|-------------|---------|
| `-o, --output` | Output BLP file | `output.blp` |
| `-q, --quality` | JPEG quality (1-100) | `85` |
| `--no-mipmaps` | Disable mipmap generation | `false` |

**Batch directory (blp2png):**
| Option | Description | Default |
|--------|-------------|---------|
| `-o, --output` | Output directory for PNGs | `png_output` |
| `-m, --mipmap` | Mipmap level (0=base) | `0` |

**Batch directory (png2blp):**
| Option | Description | Default |
|--------|-------------|---------|
| `-o, --output` | Output directory for BLP | `blp_output` |
| `-q, --quality` | JPEG quality (1-100) | `85` |
| `--no-mipmaps` | Disable mipmap generation | `false` |

## Features

- **Mipmap generation**: Automatic mipmap chain (up to 16 levels)
- **Alpha detection**: Auto-detects alpha channel
- **Format support**: PNG, JPEG, BMP, GIF, TIFF, WebP via `image` crate
- **Performance**: TurboJPEG for fast JPEG encoding
- **Zero-copy**: Methods that borrow data without allocation where possible

## Format Support

| Format | Support |
|--------|---------|
| BLP1 JPEG (CMYK with alpha) | ✓ |
| BLP1 JPEG (RGB) | ✓ |
| BLP1 Direct | ✓ |
| BLP2 | ✗ |

## Dependencies

- `turbojpeg` - JPEG encoder
- `image` - Image loading
- `zune-jpeg` - JPEG decoder
- `rayon` - Parallel mipmap decoding
- `thiserror` - Error handling
- `clap` - CLI argument parsing
- `walkdir` - Recursive directory traversal for batch commands

## License

MIT