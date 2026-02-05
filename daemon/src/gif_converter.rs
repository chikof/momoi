//! GIF to WebM converter
//!
//! This module converts animated GIF files to WebM video format for efficient playback.
//! WebM provides better compression and performance compared to frame-by-frame GIF rendering.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Convert an animated GIF to WebM format
///
/// # Arguments
///
/// * `gif_path` - Path to the input GIF file
///
/// # Returns
///
/// Path to the converted WebM file in a temporary directory
///
/// # Errors
///
/// Returns an error if:
/// - The GIF file doesn't exist
/// - FFmpeg is not available
/// - Conversion fails
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// # fn main() -> anyhow::Result<()> {
/// let webm_path = convert_gif_to_webm(Path::new("animated.gif"))?;
/// println!("Converted to: {}", webm_path.display());
/// # Ok(())
/// # }
/// ```
pub fn convert_gif_to_webm(gif_path: impl AsRef<Path>) -> Result<PathBuf> {
    let gif_path = gif_path.as_ref();

    // Validate input exists
    if !gif_path.exists() {
        anyhow::bail!("GIF file does not exist: {}", gif_path.display());
    }

    // Create a cache directory for converted videos
    let cache_dir = dirs::cache_dir()
        .context("Failed to get cache directory")?
        .join("momoi")
        .join("gif_conversions");

    std::fs::create_dir_all(&cache_dir).context("Failed to create cache directory")?;

    // Generate output path based on input hash for caching
    let gif_hash = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        gif_path.to_string_lossy().hash(&mut hasher);

        // Also include file modification time in hash
        if let Ok(metadata) = std::fs::metadata(gif_path) {
            if let Ok(modified) = metadata.modified() {
                format!("{:x}", hasher.finish())
                    + &format!("{:?}", modified)
                        .chars()
                        .filter(|c| c.is_ascii_alphanumeric())
                        .collect::<String>()
            } else {
                format!("{:x}", hasher.finish())
            }
        } else {
            format!("{:x}", hasher.finish())
        }
    };

    let webm_path = cache_dir.join(format!("{}.webm", gif_hash));

    // Check if already converted
    if webm_path.exists() {
        log::info!("Using cached WebM conversion: {}", webm_path.display());
        return Ok(webm_path);
    }

    log::info!(
        "Converting GIF to WebM: {} -> {}",
        gif_path.display(),
        webm_path.display()
    );

    // Convert using FFmpeg with VP9 codec for better quality
    // -vf scale flags=lanczos for better quality scaling
    // -c:v libvpx-vp9 for VP9 encoding (better than VP8)
    // -crf 30 controls quality (0-63, lower is better, 30 is good balance)
    // -b:v 0 enables constant quality mode
    // -pix_fmt yuva420p preserves alpha channel if present
    // -auto-alt-ref 0 disables alternate reference frames (can cause issues)
    // -an removes audio (GIFs don't have audio anyway)
    let output = Command::new("ffmpeg")
        .arg("-i")
        .arg(gif_path)
        .arg("-c:v")
        .arg("libvpx-vp9")
        .arg("-crf")
        .arg("30")
        .arg("-b:v")
        .arg("0")
        .arg("-pix_fmt")
        .arg("yuva420p")
        .arg("-auto-alt-ref")
        .arg("0")
        .arg("-an")
        .arg("-loglevel")
        .arg("error")
        .arg("-y") // Overwrite output file if it exists
        .arg(&webm_path)
        .output()
        .context("Failed to execute ffmpeg")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("FFmpeg conversion failed: {}", stderr);
    }

    log::info!(
        "Successfully converted GIF to WebM: {}",
        webm_path.display()
    );

    Ok(webm_path)
}

/// Check if a file is an animated GIF
///
/// This function checks both the file extension and whether the GIF has multiple frames.
///
/// # Arguments
///
/// * `path` - Path to check
///
/// # Returns
///
/// `true` if the file is an animated GIF, `false` otherwise
pub fn is_animated_gif(path: impl AsRef<Path>) -> Result<bool> {
    use image::AnimationDecoder;
    use image::codecs::gif::GifDecoder;

    let path = path.as_ref();

    // Check extension first for quick rejection
    if let Some(ext) = path.extension() {
        if ext.to_string_lossy().to_lowercase() != "gif" {
            return Ok(false);
        }
    } else {
        return Ok(false);
    }

    // Open file and check frame count
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let decoder = GifDecoder::new(reader)?;
    let frames = decoder.into_frames();

    // If it has more than 1 frame, it's animated
    let frame_count = frames.count();
    Ok(frame_count > 1)
}

/// Clean up old cached WebM conversions
///
/// Removes cached WebM files older than the specified age.
///
/// # Arguments
///
/// * `max_age` - Maximum age of cached files in seconds
pub fn cleanup_cache(max_age: u64) -> Result<()> {
    let cache_dir = dirs::cache_dir()
        .context("Failed to get cache directory")?
        .join("momoi")
        .join("gif_conversions");

    if !cache_dir.exists() {
        return Ok(());
    }

    let now = std::time::SystemTime::now();
    let mut removed_count = 0;

    for entry in std::fs::read_dir(&cache_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) != Some("webm") {
            continue;
        }

        if let Ok(metadata) = entry.metadata() {
            if let Ok(modified) = metadata.modified() {
                if let Ok(age) = now.duration_since(modified) {
                    if age.as_secs() > max_age {
                        if std::fs::remove_file(&path).is_ok() {
                            removed_count += 1;
                            log::debug!("Removed old cached WebM: {}", path.display());
                        }
                    }
                }
            }
        }
    }

    if removed_count > 0 {
        log::info!("Cleaned up {} old cached WebM files", removed_count);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gif_extension_check() {
        // This just tests the extension check part
        assert!(!is_animated_gif("/tmp/test.png").unwrap_or(false));
        assert!(!is_animated_gif("/tmp/test.jpg").unwrap_or(false));
        // Note: We can't fully test GIF detection without actual GIF files
    }
}
