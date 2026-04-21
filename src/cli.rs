use clap::Parser;
use std::path::PathBuf;
use std::fs;
use walkdir::WalkDir;
use indicatif::{ProgressBar, ProgressStyle};

#[derive(Parser, Debug)]
#[command(name = "wblp")]
#[command(version = "0.1.5")]
#[command(about = "BLP1 image format encoder/decoder for Warcraft III", long_about = None)]
enum Commands {
    /// Convert BLP to PNG
    #[cfg(feature = "blp2png")]
    ToPng {
        /// Input BLP file
        input: PathBuf,
        /// Output PNG file
        #[arg(short, long, default_value = "output.png")]
        output: PathBuf,
        /// Mipmap level to decode (0 = base, 1+ = smaller levels)
        #[arg(short, long, default_value_t = 0)]
        mipmap: usize,
    },
    /// Convert image to BLP
    #[cfg(feature = "png2blp")]
    ToBlp {
        /// Input image file (PNG, JPEG, etc.)
        input: PathBuf,
        /// Output BLP file
        #[arg(short, long, default_value = "output.blp")]
        output: PathBuf,
        /// JPEG quality (1-100)
        #[arg(short, long, default_value_t = 85)]
        quality: u8,
        /// Disable mipmap generation
        #[arg(long)]
        no_mipmaps: bool,
    },
    /// Convert all BLP files in a directory to PNG
    #[cfg(feature = "blp2png")]
    Blp2PngDir {
        /// Input directory containing BLP files
        input: PathBuf,
        /// Output directory for PNG files
        #[arg(short, long, default_value = "png_output")]
        output: PathBuf,
        /// Mipmap level to decode (0 = base, 1+ = smaller levels)
        #[arg(short, long, default_value_t = 0)]
        mipmap: usize,
    },
    /// Convert all image files in a directory to BLP
    #[cfg(feature = "png2blp")]
    Png2BlpDir {
        /// Input directory containing image files
        input: PathBuf,
        /// Output directory for BLP files
        #[arg(short, long, default_value = "blp_output")]
        output: PathBuf,
        /// JPEG quality (1-100)
        #[arg(short, long, default_value_t = 85)]
        quality: u8,
        /// Disable mipmap generation
        #[arg(long)]
        no_mipmaps: bool,
    },
}

#[cfg(feature = "blp2png")]
fn convert_blp_to_png_file(input: &PathBuf, output: &PathBuf, mipmap: usize) -> Result<(), Box<dyn std::error::Error>> {
    use wblp::BLPDecoder;

    let decoder = BLPDecoder::from_path(input)?;
    let _meta = decoder.metadata()?;

    let img = if mipmap == 0 {
        decoder.decode()?
    } else {
        decoder.decode_mipmap(mipmap)?
    };

    img.save_png(output)?;
    Ok(())
}

#[cfg(feature = "png2blp")]
fn convert_image_to_blp_file(input: &PathBuf, output: &PathBuf, quality: u8, no_mipmaps: bool) -> Result<(), Box<dyn std::error::Error>> {
    let encoder = wblp::BLPEncoder::from_path(input)?
        .quality(quality)
        .mipmaps(!no_mipmaps);

    encoder.save(output)?;
    Ok(())
}

#[cfg(feature = "blp2png")]
pub fn run_to_png(input: PathBuf, output: PathBuf, mipmap: usize) -> Result<(), Box<dyn std::error::Error>> {
    println!("Decoding BLP: {} → PNG: {} (mipmap level: {})", input.display(), output.display(), mipmap);

    convert_blp_to_png_file(&input, &output, mipmap)?;
    println!("Saved: {} ({} bytes)", output.display(), std::fs::metadata(&output)?.len());
    Ok(())
}

#[cfg(feature = "png2blp")]
pub fn run_to_blp(input: PathBuf, output: PathBuf, quality: u8, no_mipmaps: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Encoding: {} → BLP: {} (quality={}, mipmaps={})",
        input.display(), output.display(), quality, !no_mipmaps);

    convert_image_to_blp_file(&input, &output, quality, no_mipmaps)?;
    println!("Saved: {} ({} bytes)", output.display(), std::fs::metadata(&output)?.len());
    Ok(())
}

#[cfg(feature = "blp2png")]
pub fn run_blp2png_dir(input: PathBuf, output: PathBuf, mipmap: usize) -> Result<(), Box<dyn std::error::Error>> {
    println!("Batch BLP → PNG: {} → {} (recursive, parallel)", input.display(), output.display());

    if !output.exists() {
        fs::create_dir_all(&output)?;
    }

    let entries: Vec<_> = WalkDir::new(&input)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().map(|ext| ext == "blp").unwrap_or(false))
        .map(|e| e.path().to_path_buf())
        .collect();

    if entries.is_empty() {
        println!("No .blp files found in {}", input.display());
        return Ok(());
    }

    // Create all output directories first
    for in_path in &entries {
        let rel_path = in_path.strip_prefix(&input).unwrap_or(in_path);
        let out_path = output.join(rel_path.with_extension("png"));
        if let Some(parent) = out_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
    }

    println!("Found {} BLP file(s), processing in parallel...", entries.len());

    let pb = ProgressBar::new(entries.len() as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40}] {pos}/{len} {msg}")
        .unwrap()
        .progress_chars("█▉▊▋▌▍▎▏  "));

    use rayon::prelude::*;
    let results: Vec<_> = entries.par_iter().map(|in_path| -> (PathBuf, String, Result<u64, String>) {
        let rel_path = in_path.strip_prefix(&input).unwrap_or(in_path).to_path_buf();
        let out_path = output.join(rel_path.with_extension("png"));
        let stem = rel_path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        pb.inc(1);
        match convert_blp_to_png_file(in_path, &out_path, mipmap) {
            Ok(_) => (rel_path, stem, std::fs::metadata(&out_path).map(|m| m.len()).map_err(|e| e.to_string())),
            Err(e) => (rel_path, stem, Err(e.to_string())),
        }
    }).collect();

    pb.finish_with_message("done");

    let mut success = 0;
    let mut failed = 0;
    for (_, _, result) in results {
        match result {
            Ok(_) => success += 1,
            Err(_) => failed += 1,
        }
    }

    println!("\n{} succeeded, {} failed", success, failed);
    Ok(())
}

#[cfg(feature = "png2blp")]
pub fn run_png2blp_dir(input: PathBuf, output: PathBuf, quality: u8, no_mipmaps: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("Batch PNG → BLP: {} → {} (recursive, parallel)", input.display(), output.display());

    if !output.exists() {
        fs::create_dir_all(&output)?;
    }

    let extensions = ["png", "jpg", "jpeg", "bmp", "gif", "tiff", "webp"];
    let entries: Vec<_> = WalkDir::new(&input)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file() && e.path().extension()
                .map(|ext| extensions.contains(&ext.to_str().unwrap_or("").to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    if entries.is_empty() {
        println!("No image files found in {}", input.display());
        return Ok(());
    }

    // Create all output directories first
    for in_path in &entries {
        let rel_path = in_path.strip_prefix(&input).unwrap_or(in_path);
        let out_path = output.join(rel_path.with_extension("blp"));
        if let Some(parent) = out_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
    }

    println!("Found {} image file(s), processing in parallel...", entries.len());

    let pb = ProgressBar::new(entries.len() as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40}] {pos}/{len} {msg}")
        .unwrap()
        .progress_chars("█▉▊▋▌▍▎▏  "));

    use rayon::prelude::*;
    let results: Vec<_> = entries.par_iter().map(|in_path| -> (PathBuf, String, Result<u64, String>) {
        let rel_path = in_path.strip_prefix(&input).unwrap_or(in_path).to_path_buf();
        let out_path = output.join(rel_path.with_extension("blp"));
        let stem = rel_path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        pb.inc(1);
        match convert_image_to_blp_file(in_path, &out_path, quality, no_mipmaps) {
            Ok(_) => (rel_path, stem, std::fs::metadata(&out_path).map(|m| m.len()).map_err(|e| e.to_string())),
            Err(e) => (rel_path, stem, Err(e.to_string())),
        }
    }).collect();

    pb.finish_with_message("done");

    let mut success = 0;
    let mut failed = 0;
    for (_, _, result) in results {
        match result {
            Ok(_) => success += 1,
            Err(_) => failed += 1,
        }
    }

    println!("\n{} succeeded, {} failed", success, failed);
    Ok(())
}

pub fn run() {
    let cmd = Commands::parse();

    #[cfg(feature = "blp2png")]
    if let Commands::ToPng { input, output, mipmap } = cmd {
        if let Err(e) = run_to_png(input, output, mipmap) {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    #[cfg(feature = "png2blp")]
    if let Commands::ToBlp { input, output, quality, no_mipmaps } = cmd {
        if let Err(e) = run_to_blp(input, output, quality, no_mipmaps) {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    #[cfg(feature = "blp2png")]
    if let Commands::Blp2PngDir { input, output, mipmap } = cmd {
        if let Err(e) = run_blp2png_dir(input, output, mipmap) {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    #[cfg(feature = "png2blp")]
    if let Commands::Png2BlpDir { input, output, quality, no_mipmaps } = cmd {
        if let Err(e) = run_png2blp_dir(input, output, quality, no_mipmaps) {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
        return;
    }
}