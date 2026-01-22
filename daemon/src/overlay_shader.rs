use anyhow::Result;
use std::time::Instant;

/// Overlay shader types that render on top of existing wallpapers
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverlayShader {
    /// Darkens edges (vignette effect)
    Vignette { strength: f32 },
    /// Horizontal scanlines (CRT effect)
    Scanlines { intensity: f32, line_width: f32 },
    /// Film grain/noise
    FilmGrain { intensity: f32 },
    /// RGB chromatic aberration
    ChromaticAberration { offset: f32 },
    /// CRT curvature and effects
    CRT {
        curvature: f32,
        scanline_intensity: f32,
    },
    /// Pixelate effect
    Pixelate { pixel_size: u32 },
    /// Color tint overlay
    ColorTint {
        r: f32,
        g: f32,
        b: f32,
        strength: f32,
    },
}

impl OverlayShader {
    /// Parse overlay shader from string with parameters
    pub fn from_str(s: &str, params: &OverlayParams) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "vignette" => Some(OverlayShader::Vignette {
                strength: params.strength.unwrap_or(0.7),
            }),
            "scanlines" => Some(OverlayShader::Scanlines {
                intensity: params.intensity.unwrap_or(0.3),
                line_width: params.line_width.unwrap_or(2.0),
            }),
            "film-grain" | "filmgrain" | "grain" => Some(OverlayShader::FilmGrain {
                intensity: params.intensity.unwrap_or(0.1),
            }),
            "chromatic" | "chromatic-aberration" => Some(OverlayShader::ChromaticAberration {
                offset: params.offset.unwrap_or(2.0),
            }),
            "crt" => Some(OverlayShader::CRT {
                curvature: params.curvature.unwrap_or(0.15),
                scanline_intensity: params.intensity.unwrap_or(0.3),
            }),
            "pixelate" => Some(OverlayShader::Pixelate {
                pixel_size: params.pixel_size.unwrap_or(8),
            }),
            "tint" | "color-tint" | "color_tint" => Some(OverlayShader::ColorTint {
                r: params.r.unwrap_or(1.0),
                g: params.g.unwrap_or(0.8),
                b: params.b.unwrap_or(0.6),
                strength: params.strength.unwrap_or(0.3),
            }),
            _ => None,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            OverlayShader::Vignette { .. } => "vignette",
            OverlayShader::Scanlines { .. } => "scanlines",
            OverlayShader::FilmGrain { .. } => "film-grain",
            OverlayShader::ChromaticAberration { .. } => "chromatic-aberration",
            OverlayShader::CRT { .. } => "crt",
            OverlayShader::Pixelate { .. } => "pixelate",
            OverlayShader::ColorTint { .. } => "tint",
        }
    }

    /// Convert to common::OverlayEffect for GPU rendering
    pub fn to_common_effect(&self) -> common::OverlayEffect {
        match self {
            OverlayShader::Vignette { .. } => common::OverlayEffect::Vignette,
            OverlayShader::Scanlines { .. } => common::OverlayEffect::Scanlines,
            OverlayShader::FilmGrain { .. } => common::OverlayEffect::FilmGrain,
            OverlayShader::ChromaticAberration { .. } => common::OverlayEffect::ChromaticAberration,
            OverlayShader::CRT { .. } => common::OverlayEffect::Crt,
            OverlayShader::Pixelate { .. } => common::OverlayEffect::Pixelate,
            OverlayShader::ColorTint { .. } => common::OverlayEffect::ColorTint,
        }
    }

    /// Convert to common::OverlayParams for GPU rendering
    pub fn to_common_params(&self) -> common::OverlayParams {
        match self {
            OverlayShader::Vignette { strength } => common::OverlayParams {
                strength: Some(*strength),
                ..Default::default()
            },
            OverlayShader::Scanlines {
                intensity,
                line_width,
            } => common::OverlayParams {
                intensity: Some(*intensity),
                line_width: Some(*line_width),
                ..Default::default()
            },
            OverlayShader::FilmGrain { intensity } => common::OverlayParams {
                intensity: Some(*intensity),
                ..Default::default()
            },
            OverlayShader::ChromaticAberration { offset } => common::OverlayParams {
                offset: Some(*offset),
                ..Default::default()
            },
            OverlayShader::CRT {
                curvature,
                scanline_intensity,
            } => common::OverlayParams {
                curvature: Some(*curvature),
                intensity: Some(*scanline_intensity),
                ..Default::default()
            },
            OverlayShader::Pixelate { pixel_size } => common::OverlayParams {
                pixel_size: Some(*pixel_size),
                ..Default::default()
            },
            OverlayShader::ColorTint { r, g, b, strength } => common::OverlayParams {
                r: Some(*r),
                g: Some(*g),
                b: Some(*b),
                strength: Some(*strength),
                ..Default::default()
            },
        }
    }
}

/// Parameters for overlay shaders
#[derive(Debug, Clone, Default)]
pub struct OverlayParams {
    pub strength: Option<f32>,
    pub intensity: Option<f32>,
    pub line_width: Option<f32>,
    pub offset: Option<f32>,
    pub curvature: Option<f32>,
    pub pixel_size: Option<u32>,
    pub r: Option<f32>,
    pub g: Option<f32>,
    pub b: Option<f32>,
}

/// Overlay shader manager
pub struct OverlayManager {
    overlay: OverlayShader,
    time: Instant,
    frame: u64,
}

impl OverlayManager {
    /// Create new overlay manager
    pub fn new(overlay: OverlayShader) -> Self {
        OverlayManager {
            overlay,
            time: Instant::now(),
            frame: 0,
        }
    }

    /// Get reference to the overlay shader
    pub fn overlay(&self) -> &OverlayShader {
        &self.overlay
    }

    /// Get elapsed time since overlay was created
    pub fn elapsed_time(&self) -> f32 {
        self.time.elapsed().as_secs_f32()
    }

    /// Apply overlay effect to existing ARGB buffer
    pub fn apply_overlay(&mut self, buffer: &mut [u8], width: u32, height: u32) -> Result<()> {
        self.frame += 1;
        let time = self.time.elapsed().as_secs_f32();

        // Log first few frames to see parameters
        if self.frame <= 3 {
            log::info!(
                "Overlay frame #{}: {} ({}x{}, {} bytes) - time: {:.2}s",
                self.frame,
                self.overlay.name(),
                width,
                height,
                buffer.len(),
                time
            );
        }

        match self.overlay {
            OverlayShader::Vignette { strength } => {
                if self.frame == 1 {
                    log::info!("Vignette parameters: strength={}", strength);
                }
                self.apply_vignette(buffer, width, height, strength);
            }
            OverlayShader::Scanlines {
                intensity,
                line_width,
            } => {
                if self.frame == 1 {
                    log::info!("Scanlines parameters: intensity={}, line_width={}", intensity, line_width);
                }
                self.apply_scanlines(buffer, width, height, intensity, line_width);
            }
            OverlayShader::FilmGrain { intensity } => {
                if self.frame == 1 {
                    log::info!("FilmGrain parameters: intensity={}", intensity);
                }
                self.apply_film_grain(buffer, width, height, intensity, time);
            }
            OverlayShader::ChromaticAberration { offset } => {
                if self.frame == 1 {
                    log::info!("ChromaticAberration parameters: offset={}", offset);
                }
                self.apply_chromatic_aberration(buffer, width, height, offset);
            }
            OverlayShader::CRT {
                curvature,
                scanline_intensity,
            } => {
                if self.frame == 1 {
                    log::info!("CRT parameters: curvature={}, scanline_intensity={}", curvature, scanline_intensity);
                }
                self.apply_crt(buffer, width, height, curvature, scanline_intensity);
            }
            OverlayShader::Pixelate { pixel_size } => {
                if self.frame == 1 {
                    log::info!("Pixelate parameters: pixel_size={}", pixel_size);
                }
                self.apply_pixelate(buffer, width, height, pixel_size);
            }
            OverlayShader::ColorTint { r, g, b, strength } => {
                if self.frame == 1 {
                    log::info!("ColorTint parameters: r={}, g={}, b={}, strength={}", r, g, b, strength);
                }
                self.apply_color_tint(buffer, width, height, r, g, b, strength);
            }
        }

        Ok(())
    }

    /// Apply vignette effect (darken edges)
    fn apply_vignette(&self, buffer: &mut [u8], width: u32, height: u32, strength: f32) {
        let center_x = width as f32 / 2.0;
        let center_y = height as f32 / 2.0;
        let max_dist = (center_x * center_x + center_y * center_y).sqrt();

        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;

                let dx = x as f32 - center_x;
                let dy = y as f32 - center_y;
                let dist = (dx * dx + dy * dy).sqrt();
                let vignette = 1.0 - (dist / max_dist * strength).min(1.0);

                buffer[idx] = (buffer[idx] as f32 * vignette) as u8; // B
                buffer[idx + 1] = (buffer[idx + 1] as f32 * vignette) as u8; // G
                buffer[idx + 2] = (buffer[idx + 2] as f32 * vignette) as u8; // R
            }
        }
    }

    /// Apply scanlines effect
    fn apply_scanlines(
        &self,
        buffer: &mut [u8],
        width: u32,
        height: u32,
        intensity: f32,
        line_width: f32,
    ) {
        for y in 0..height {
            let scanline = ((y as f32 / line_width).sin() * 0.5 + 0.5) * intensity;
            let darken = 1.0 - scanline;

            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;

                buffer[idx] = (buffer[idx] as f32 * darken) as u8;
                buffer[idx + 1] = (buffer[idx + 1] as f32 * darken) as u8;
                buffer[idx + 2] = (buffer[idx + 2] as f32 * darken) as u8;
            }
        }
    }

    /// Apply film grain effect
    fn apply_film_grain(
        &self,
        buffer: &mut [u8],
        width: u32,
        height: u32,
        intensity: f32,
        time: f32,
    ) {
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;

                // Pseudo-random noise
                let seed = (x as f32 * 12.9898 + y as f32 * 78.233 + time * 43758.5453).sin();
                let noise = (seed.fract() - 0.5) * intensity * 255.0;

                buffer[idx] = (buffer[idx] as f32 + noise).clamp(0.0, 255.0) as u8;
                buffer[idx + 1] = (buffer[idx + 1] as f32 + noise).clamp(0.0, 255.0) as u8;
                buffer[idx + 2] = (buffer[idx + 2] as f32 + noise).clamp(0.0, 255.0) as u8;
            }
        }
    }

    /// Apply chromatic aberration (RGB split)
    fn apply_chromatic_aberration(&self, buffer: &mut [u8], width: u32, height: u32, offset: f32) {
        let mut new_buffer = buffer.to_vec();
        let offset_i = offset as i32;

        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;

                // Sample red from offset position
                let rx = (x as i32 + offset_i).clamp(0, width as i32 - 1) as u32;
                let r_idx = ((y * width + rx) * 4) as usize;

                // Sample blue from opposite offset
                let bx = (x as i32 - offset_i).clamp(0, width as i32 - 1) as u32;
                let b_idx = ((y * width + bx) * 4) as usize;

                new_buffer[idx] = buffer[b_idx]; // B (shifted left)
                new_buffer[idx + 1] = buffer[idx + 1]; // G (unchanged)
                new_buffer[idx + 2] = buffer[r_idx + 2]; // R (shifted right)
            }
        }

        buffer.copy_from_slice(&new_buffer);
    }

    /// Apply CRT effect (combines scanlines and slight curvature simulation)
    fn apply_crt(
        &self,
        buffer: &mut [u8],
        width: u32,
        height: u32,
        _curvature: f32,
        scanline_intensity: f32,
    ) {
        // Apply scanlines
        self.apply_scanlines(buffer, width, height, scanline_intensity, 2.0);

        // Add slight vignette
        self.apply_vignette(buffer, width, height, 0.3);

        // Add subtle RGB separation at edges
        let center_x = width as f32 / 2.0;
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;

                let edge_dist = ((x as f32 - center_x).abs() / center_x).powf(2.0);
                if edge_dist > 0.7 {
                    // Slight color shift at edges
                    let shift = ((edge_dist - 0.7) * 50.0) as u8;
                    buffer[idx] = buffer[idx].saturating_sub(shift);
                    buffer[idx + 2] = buffer[idx + 2].saturating_add(shift);
                }
            }
        }
    }

    /// Apply pixelate effect
    fn apply_pixelate(&self, buffer: &mut [u8], width: u32, height: u32, pixel_size: u32) {
        let mut new_buffer = buffer.to_vec();

        for block_y in (0..height).step_by(pixel_size as usize) {
            for block_x in (0..width).step_by(pixel_size as usize) {
                // Calculate average color of block
                let mut avg_b = 0u32;
                let mut avg_g = 0u32;
                let mut avg_r = 0u32;
                let mut count = 0u32;

                for y in block_y..(block_y + pixel_size).min(height) {
                    for x in block_x..(block_x + pixel_size).min(width) {
                        let idx = ((y * width + x) * 4) as usize;
                        avg_b += buffer[idx] as u32;
                        avg_g += buffer[idx + 1] as u32;
                        avg_r += buffer[idx + 2] as u32;
                        count += 1;
                    }
                }

                if count > 0 {
                    avg_b /= count;
                    avg_g /= count;
                    avg_r /= count;

                    // Fill block with average color
                    for y in block_y..(block_y + pixel_size).min(height) {
                        for x in block_x..(block_x + pixel_size).min(width) {
                            let idx = ((y * width + x) * 4) as usize;
                            new_buffer[idx] = avg_b as u8;
                            new_buffer[idx + 1] = avg_g as u8;
                            new_buffer[idx + 2] = avg_r as u8;
                        }
                    }
                }
            }
        }

        buffer.copy_from_slice(&new_buffer);
    }

    /// Apply color tint overlay
    fn apply_color_tint(
        &self,
        buffer: &mut [u8],
        width: u32,
        height: u32,
        r: f32,
        g: f32,
        b: f32,
        strength: f32,
    ) {
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;

                let orig_b = buffer[idx] as f32;
                let orig_g = buffer[idx + 1] as f32;
                let orig_r = buffer[idx + 2] as f32;

                buffer[idx] = (orig_b * (1.0 - strength) + orig_b * b * strength) as u8;
                buffer[idx + 1] = (orig_g * (1.0 - strength) + orig_g * g * strength) as u8;
                buffer[idx + 2] = (orig_r * (1.0 - strength) + orig_r * r * strength) as u8;
            }
        }
    }
}
