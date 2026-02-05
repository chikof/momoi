use anyhow::{Context, Result};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use std::path::Path;

/// Manages wallpaper loading, caching, and processing
pub struct WallpaperManager {
    cache: std::collections::HashMap<String, CachedImage>,
}

/// A cached image with metadata
struct CachedImage {
    image: DynamicImage,
    path: String,
}

impl WallpaperManager {
    pub fn new() -> Self {
        Self {
            cache: std::collections::HashMap::new(),
        }
    }

    /// Check if a file is a GIF (will be converted to video)
    pub fn is_gif(path: impl AsRef<Path>) -> bool {
        let path = path.as_ref();

        // Check extension
        if let Some(ext) = path.extension() {
            ext.to_string_lossy().to_lowercase() == "gif"
        } else {
            false
        }
    }

    /// Check if a file is a video
    pub fn is_video(path: impl AsRef<Path>) -> bool {
        let path = path.as_ref();

        // Check extension for video formats
        if let Some(ext) = path.extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            matches!(
                ext_lower.as_str(),
                "mp4" | "webm" | "mkv" | "avi" | "mov" | "flv" | "wmv" | "m4v" | "ogv"
            )
        } else {
            false
        }
    }

    /// Load an image from a file path
    pub fn load_image(&mut self, path: impl AsRef<Path>) -> Result<&DynamicImage> {
        let path = path.as_ref();
        let path_str = path.to_string_lossy().to_string();

        // Check cache first
        if !self.cache.contains_key(&path_str) {
            log::info!("Loading image: {}", path.display());

            let image =
                image::open(path).context(format!("Failed to load image: {}", path.display()))?;

            log::info!(
                "Loaded image: {}x{} ({})",
                image.width(),
                image.height(),
                path.display()
            );

            self.cache.insert(
                path_str.clone(),
                CachedImage {
                    image,
                    path: path_str.clone(),
                },
            );
        }

        Ok(&self.cache.get(&path_str).unwrap().image)
    }

    /// Scale/fit an image to the target dimensions
    pub fn scale_image(
        &self,
        image: &DynamicImage,
        target_width: u32,
        target_height: u32,
        mode: common::ScaleMode,
    ) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>> {
        // let (img_width, img_height) = (image.width(), image.height());

        match mode {
            common::ScaleMode::Center => self.center_image(image, target_width, target_height),
            common::ScaleMode::Fill => self.fill_image(image, target_width, target_height),
            common::ScaleMode::Fit => self.fit_image(image, target_width, target_height),
            common::ScaleMode::Stretch => self.stretch_image(image, target_width, target_height),
            common::ScaleMode::Tile => self.tile_image(image, target_width, target_height),
        }
    }

    /// Center image without scaling
    fn center_image(
        &self,
        image: &DynamicImage,
        target_width: u32,
        target_height: u32,
    ) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>> {
        let mut output = ImageBuffer::from_pixel(target_width, target_height, Rgba([0, 0, 0, 255]));

        let rgba_image = image.to_rgba8();
        let (img_width, img_height) = rgba_image.dimensions();

        let x_offset = (target_width.saturating_sub(img_width)) / 2;
        let y_offset = (target_height.saturating_sub(img_height)) / 2;

        image::imageops::overlay(&mut output, &rgba_image, x_offset as i64, y_offset as i64);

        Ok(output)
    }

    /// Scale to fill entire output (may crop)
    fn fill_image(
        &self,
        image: &DynamicImage,
        target_width: u32,
        target_height: u32,
    ) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>> {
        let (img_width, img_height) = (image.width(), image.height());
        let target_ratio = target_width as f32 / target_height as f32;
        let img_ratio = img_width as f32 / img_height as f32;

        let (scale_width, scale_height) = if target_ratio > img_ratio {
            // Target is wider, scale to width
            let scale = target_width as f32 / img_width as f32;
            (target_width, (img_height as f32 * scale) as u32)
        } else {
            // Target is taller, scale to height
            let scale = target_height as f32 / img_height as f32;
            ((img_width as f32 * scale) as u32, target_height)
        };

        // Resize image
        let resized = self.resize_image_fast(image, scale_width, scale_height)?;

        // Crop to target size if needed
        if scale_width != target_width || scale_height != target_height {
            let x_offset = (scale_width.saturating_sub(target_width)) / 2;
            let y_offset = (scale_height.saturating_sub(target_height)) / 2;

            Ok(
                image::imageops::crop_imm(
                    &resized,
                    x_offset,
                    y_offset,
                    target_width,
                    target_height,
                )
                .to_image(),
            )
        } else {
            Ok(resized)
        }
    }

    /// Scale to fit within output (may have letterboxing)
    fn fit_image(
        &self,
        image: &DynamicImage,
        target_width: u32,
        target_height: u32,
    ) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>> {
        let (img_width, img_height) = (image.width(), image.height());
        let target_ratio = target_width as f32 / target_height as f32;
        let img_ratio = img_width as f32 / img_height as f32;

        log::debug!(
            "Fit mode: image {}x{} (ratio {:.2}), target {}x{} (ratio {:.2})",
            img_width,
            img_height,
            img_ratio,
            target_width,
            target_height,
            target_ratio
        );

        let (scale_width, scale_height) = if target_ratio > img_ratio {
            // Target is wider than image, scale to height
            let scale = target_height as f32 / img_height as f32;
            let scaled = ((img_width as f32 * scale) as u32, target_height);
            log::debug!(
                "Target wider: scaling to height, result: {}x{}",
                scaled.0,
                scaled.1
            );
            scaled
        } else {
            // Target is taller than image (or same), scale to width
            let scale = target_width as f32 / img_width as f32;
            let scaled = (target_width, (img_height as f32 * scale) as u32);
            log::debug!(
                "Target taller: scaling to width, result: {}x{}",
                scaled.0,
                scaled.1
            );
            scaled
        };

        // Resize image
        let resized = self.resize_image_fast(image, scale_width, scale_height)?;

        // Center on black background
        let mut output = ImageBuffer::from_pixel(target_width, target_height, Rgba([0, 0, 0, 255]));
        let x_offset = (target_width.saturating_sub(scale_width)) / 2;
        let y_offset = (target_height.saturating_sub(scale_height)) / 2;

        log::debug!("Centering at offset ({}, {})", x_offset, y_offset);

        image::imageops::overlay(&mut output, &resized, x_offset as i64, y_offset as i64);

        Ok(output)
    }

    /// Stretch to fill output
    fn stretch_image(
        &self,
        image: &DynamicImage,
        target_width: u32,
        target_height: u32,
    ) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>> {
        self.resize_image_fast(image, target_width, target_height)
    }

    /// Tile the image
    fn tile_image(
        &self,
        image: &DynamicImage,
        target_width: u32,
        target_height: u32,
    ) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>> {
        let mut output = ImageBuffer::from_pixel(target_width, target_height, Rgba([0, 0, 0, 255]));
        let rgba_image = image.to_rgba8();
        let (img_width, img_height) = rgba_image.dimensions();

        let tiles_x = target_width.div_ceil(img_width);
        let tiles_y = target_height.div_ceil(img_height);

        for ty in 0..tiles_y {
            for tx in 0..tiles_x {
                let x = tx * img_width;
                let y = ty * img_height;
                image::imageops::overlay(&mut output, &rgba_image, x as i64, y as i64);
            }
        }

        Ok(output)
    }

    /// Fast image resizing using fast_image_resize
    fn resize_image_fast(
        &self,
        image: &DynamicImage,
        target_width: u32,
        target_height: u32,
    ) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>> {
        use fast_image_resize as fr;

        let src_image = image.to_rgba8();
        let (src_width, src_height) = src_image.dimensions();

        // Create source image for fast_image_resize
        let src = fr::images::Image::from_vec_u8(
            TryInto::try_into(src_width)?,
            TryInto::try_into(src_height)?,
            src_image.into_raw(),
            fr::PixelType::U8x4,
        )
        .context("Failed to create source image")?;

        // Create destination image
        let mut dst = fr::images::Image::new(
            TryInto::try_into(target_width)?,
            TryInto::try_into(target_height)?,
            fr::PixelType::U8x4,
        );

        // Resize
        let mut resizer = fr::Resizer::new();
        resizer
            .resize(
                &src,
                &mut dst,
                &fr::ResizeOptions::new()
                    .resize_alg(fr::ResizeAlg::Convolution(fr::FilterType::Lanczos3)),
            )
            .context("Failed to resize image")?;

        // Convert back to ImageBuffer
        ImageBuffer::from_raw(target_width, target_height, dst.into_vec())
            .context("Failed to create output image buffer")
    }

    /// Convert RGBA image to ARGB8888 format for Wayland
    pub fn rgba_to_argb8888(&self, rgba: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> Vec<u8> {
        let mut argb = Vec::with_capacity(rgba.len());

        for pixel in rgba.pixels() {
            let r = pixel[0];
            let g = pixel[1];
            let b = pixel[2];
            let a = pixel[3];

            // Wayland expects ARGB8888 in native byte order
            argb.extend_from_slice(&[b, g, r, a]);
        }

        argb
    }

    /// Clear the image cache
    pub fn clear_cache(&mut self) {
        log::info!("Clearing image cache ({} entries)", self.cache.len());
        self.cache.clear();
    }

    /// Get cache size
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgba_to_argb8888() {
        let manager = WallpaperManager::new();

        // Create a simple 2x2 RGBA image
        let mut img = ImageBuffer::new(2, 2);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255])); // Red
        img.put_pixel(1, 0, Rgba([0, 255, 0, 255])); // Green
        img.put_pixel(0, 1, Rgba([0, 0, 255, 255])); // Blue
        img.put_pixel(1, 1, Rgba([255, 255, 255, 255])); // White

        let argb = manager.rgba_to_argb8888(&img);

        // Check ARGB8888 format (BGRA in memory)
        assert_eq!(argb[0], 0); // B
        assert_eq!(argb[1], 0); // G
        assert_eq!(argb[2], 255); // R
        assert_eq!(argb[3], 255); // A
    }
}
