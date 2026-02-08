use std::time::{Duration, Instant};

/// Transition effect types
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)] // All variants used via conversion from common::TransitionType
pub enum TransitionType {
    /// No transition, instant switch
    None,
    /// Fade in/out (alpha blending)
    Fade,
    /// Wipe from left to right
    WipeLeft,
    /// Wipe from right to left
    WipeRight,
    /// Wipe from top to bottom
    WipeTop,
    /// Wipe from bottom to top
    WipeBottom,
    /// Wipe at custom angle (degrees, 0=right, 90=down, 180=left, 270=up)
    WipeAngle(f32),
    /// Expand from center outward
    Center,
    /// Shrink from edges inward
    Outer,
    /// Random selection (will be converted to a specific type)
    Random,
}

impl Default for TransitionType {
    fn default() -> Self {
        Self::Fade
    }
}

impl From<&common::TransitionType> for TransitionType {
    fn from(t: &common::TransitionType) -> Self {
        match t {
            common::TransitionType::None => Self::None,
            common::TransitionType::Fade { .. } => Self::Fade,
            common::TransitionType::WipeLeft { .. } => Self::WipeLeft,
            common::TransitionType::WipeRight { .. } => Self::WipeRight,
            common::TransitionType::WipeTop { .. } => Self::WipeTop,
            common::TransitionType::WipeBottom { .. } => Self::WipeBottom,
            common::TransitionType::WipeAngle { angle_degrees, .. } => {
                Self::WipeAngle(*angle_degrees)
            }
            common::TransitionType::Center { .. } => Self::Center,
            common::TransitionType::Outer { .. } => Self::Outer,
            common::TransitionType::Random { .. } => {
                // Pick a random transition type
                use rand::Rng;

                let mut rng = rand::rng();
                let choice = rng.random_range(0..8);

                match choice {
                    0 => Self::Fade,
                    1 => Self::WipeLeft,
                    2 => Self::WipeRight,
                    3 => Self::WipeTop,
                    4 => Self::WipeBottom,
                    5 => Self::WipeAngle(45.0), // Diagonal
                    6 => Self::Center,
                    _ => Self::Outer,
                }
            }
        }
    }
}

/// Easing functions for smooth transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // All variants part of public easing API
pub enum EasingFunction {
    /// Linear interpolation (constant speed)
    Linear,
    /// Ease in (slow start, fast end)
    EaseIn,
    /// Ease out (fast start, slow end)
    EaseOut,
    /// Ease in-out (slow start and end, fast middle)
    EaseInOut,
}

impl Default for EasingFunction {
    fn default() -> Self {
        Self::EaseInOut
    }
}

impl EasingFunction {
    /// Apply easing to a linear progress value (0.0 to 1.0)
    pub fn apply(&self, t: f32) -> f32 {
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => t * (2.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
        }
    }
}

/// Manages a transition between two wallpapers
pub struct Transition {
    /// Type of transition effect
    transition_type: TransitionType,
    /// Easing function
    easing: EasingFunction,
    /// Total duration of transition
    duration: Duration,
    /// When the transition started
    start_time: Instant,
    /// Old wallpaper frame data (ARGB8888)
    old_frame: Vec<u8>,
    /// Dimensions of the frames
    width: u32,
    height: u32,
    /// Optional GPU renderer for accelerated transitions
    #[cfg(feature = "gpu")]
    gpu_renderer: Option<std::sync::Arc<crate::gpu::GpuRenderer>>,
}

impl Transition {
    /// Create a new transition
    pub fn new(
        transition_type: TransitionType,
        duration: Duration,
        old_frame: Vec<u8>,
        width: u32,
        height: u32,
        #[cfg(feature = "gpu")] gpu_renderer: Option<std::sync::Arc<crate::gpu::GpuRenderer>>,
    ) -> Self {
        Self {
            transition_type,
            easing: EasingFunction::default(),
            duration,
            start_time: Instant::now(),
            old_frame,
            width,
            height,
            #[cfg(feature = "gpu")]
            gpu_renderer,
        }
    }

    /// Get the current progress (0.0 to 1.0)
    fn raw_progress(&self) -> f32 {
        let elapsed = self.start_time.elapsed();
        if elapsed >= self.duration {
            1.0
        } else {
            elapsed.as_secs_f32() / self.duration.as_secs_f32()
        }
    }

    /// Get the eased progress (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        self.easing.apply(self.raw_progress())
    }

    /// Check if the transition is complete
    pub fn is_complete(&self) -> bool {
        self.start_time.elapsed() >= self.duration
    }

    /// Blend old and new frames based on current progress
    /// Returns the blended frame data, or None if GPU is warming up (first frame)
    pub fn blend_frames(&self, new_frame: &[u8]) -> Option<Vec<u8>> {
        let progress = self.progress();

        // Try GPU blending first if available
        #[cfg(feature = "gpu")]
        {
            if let Some(ref gpu) = self.gpu_renderer {
                // Map transition type to GPU transition type
                let gpu_transition_type = match self.transition_type {
                    TransitionType::None => return Some(new_frame.to_vec()),
                    TransitionType::Fade => 0,
                    TransitionType::WipeLeft => 1,
                    TransitionType::WipeRight => 2,
                    TransitionType::WipeTop => 3,
                    TransitionType::WipeBottom => 4,
                    TransitionType::Center => 5,
                    TransitionType::Outer => 6,
                    TransitionType::WipeAngle(_) | TransitionType::Random => {
                        // Fall back to CPU for unsupported types
                        return Some(self.blend_frames_cpu(new_frame, progress));
                    }
                };

                match gpu.blend_frames(
                    &self.old_frame,
                    new_frame,
                    self.width,
                    self.height,
                    progress,
                    gpu_transition_type,
                ) {
                    Ok(Some(blended)) => return Some(blended),
                    Ok(None) => return None, // GPU warming up, no frame yet
                    Err(e) => {
                        log::warn!("GPU transition blending failed: {}, falling back to CPU", e);
                    }
                }
            }
        }

        // CPU fallback
        Some(self.blend_frames_cpu(new_frame, progress))
    }

    /// CPU-based frame blending (fallback)
    fn blend_frames_cpu(&self, new_frame: &[u8], progress: f32) -> Vec<u8> {
        match self.transition_type {
            TransitionType::None => new_frame.to_vec(),
            TransitionType::Fade => self.blend_fade(new_frame, progress),
            TransitionType::WipeLeft => self.blend_wipe_horizontal(new_frame, progress, false),
            TransitionType::WipeRight => self.blend_wipe_horizontal(new_frame, progress, true),
            TransitionType::WipeTop => self.blend_wipe_vertical(new_frame, progress, false),
            TransitionType::WipeBottom => self.blend_wipe_vertical(new_frame, progress, true),
            TransitionType::WipeAngle(angle) => self.blend_wipe_angle(new_frame, progress, angle),
            TransitionType::Center => self.blend_center(new_frame, progress),
            TransitionType::Outer => self.blend_outer(new_frame, progress),
            TransitionType::Random => new_frame.to_vec(), // Should not reach here
        }
    }

    /// Fade transition: alpha blend between old and new
    fn blend_fade(&self, new_frame: &[u8], progress: f32) -> Vec<u8> {
        let mut result = vec![0u8; self.old_frame.len()];
        let inv = 1.0 - progress;

        // ARGB8888 format: 4 bytes per pixel, using chunks for auto-vectorization
        for ((old, new), out) in self
            .old_frame
            .chunks_exact(4)
            .zip(new_frame.chunks_exact(4))
            .zip(result.chunks_exact_mut(4))
        {
            out[0] = (old[0] as f32 * inv + new[0] as f32 * progress) as u8;
            out[1] = (old[1] as f32 * inv + new[1] as f32 * progress) as u8;
            out[2] = (old[2] as f32 * inv + new[2] as f32 * progress) as u8;
            out[3] = (old[3] as f32 * inv + new[3] as f32 * progress) as u8;
        }

        result
    }

    /// Horizontal wipe transition
    fn blend_wipe_horizontal(
        &self,
        new_frame: &[u8],
        progress: f32,
        right_to_left: bool,
    ) -> Vec<u8> {
        let mut result = vec![0u8; self.old_frame.len()];
        let stride = self.width as usize * 4;
        let boundary_px = if right_to_left {
            (self.width as f32 * (1.0 - progress)) as usize
        } else {
            (self.width as f32 * progress) as usize
        }
        .min(self.width as usize);

        for y in 0..self.height as usize {
            let row_start = y * stride;
            let split = boundary_px * 4;

            if right_to_left {
                // old pixels [0..split), new pixels [split..stride)
                result[row_start..row_start + split]
                    .copy_from_slice(&self.old_frame[row_start..row_start + split]);
                result[row_start + split..row_start + stride]
                    .copy_from_slice(&new_frame[row_start + split..row_start + stride]);
            } else {
                // new pixels [0..split), old pixels [split..stride)
                result[row_start..row_start + split]
                    .copy_from_slice(&new_frame[row_start..row_start + split]);
                result[row_start + split..row_start + stride]
                    .copy_from_slice(&self.old_frame[row_start + split..row_start + stride]);
            }
        }

        result
    }

    /// Vertical wipe transition
    fn blend_wipe_vertical(&self, new_frame: &[u8], progress: f32, bottom_to_top: bool) -> Vec<u8> {
        let mut result = vec![0u8; self.old_frame.len()];
        let stride = self.width as usize * 4;
        let boundary_row = if bottom_to_top {
            (self.height as f32 * (1.0 - progress)) as usize
        } else {
            (self.height as f32 * progress) as usize
        }
        .min(self.height as usize);

        let split_byte = boundary_row * stride;

        if bottom_to_top {
            // old rows [0..split), new rows [split..end)
            result[..split_byte].copy_from_slice(&self.old_frame[..split_byte]);
            result[split_byte..].copy_from_slice(&new_frame[split_byte..]);
        } else {
            // new rows [0..split), old rows [split..end)
            result[..split_byte].copy_from_slice(&new_frame[..split_byte]);
            result[split_byte..].copy_from_slice(&self.old_frame[split_byte..]);
        }

        result
    }

    /// Diagonal wipe transition at a custom angle
    fn blend_wipe_angle(&self, new_frame: &[u8], progress: f32, angle: f32) -> Vec<u8> {
        let mut result = vec![0u8; self.old_frame.len()];
        let stride = self.width as usize * 4;

        // Convert angle to radians
        let angle_rad = angle.to_radians();
        let cos_a = angle_rad.cos();
        let sin_a = angle_rad.sin();

        // Calculate the diagonal distance for normalization
        let max_dist = self.width as f32 * cos_a.abs() + self.height as f32 * sin_a.abs();
        let boundary = max_dist * progress;

        for y in 0..self.height as usize {
            let row_start = y * stride;
            for x in 0..self.width as usize {
                let pixel_start = row_start + x * 4;

                // Calculate distance along the angle direction
                let dist = x as f32 * cos_a + y as f32 * sin_a;

                // Copy from new or old frame
                let src = if dist < boundary {
                    &new_frame[pixel_start..pixel_start + 4]
                } else {
                    &self.old_frame[pixel_start..pixel_start + 4]
                };
                result[pixel_start..pixel_start + 4].copy_from_slice(src);
            }
        }

        result
    }

    /// Center expand transition (expand from center outward)
    fn blend_center(&self, new_frame: &[u8], progress: f32) -> Vec<u8> {
        let mut result = vec![0u8; self.old_frame.len()];
        let stride = self.width as usize * 4;

        // Calculate center point
        let center_x = self.width as f32 / 2.0;
        let center_y = self.height as f32 / 2.0;

        // Maximum distance from center to corner
        let max_radius = (center_x * center_x + center_y * center_y).sqrt();
        let current_radius_sq = (max_radius * progress) * (max_radius * progress);

        for y in 0..self.height as usize {
            let row_start = y * stride;
            let dy = y as f32 - center_y;
            let dy_sq = dy * dy;

            for x in 0..self.width as usize {
                let pixel_start = row_start + x * 4;

                // Calculate squared distance from center (avoid sqrt)
                let dx = x as f32 - center_x;
                let dist_sq = dx * dx + dy_sq;

                // Copy from new frame if within current radius, else old frame
                let src = if dist_sq < current_radius_sq {
                    &new_frame[pixel_start..pixel_start + 4]
                } else {
                    &self.old_frame[pixel_start..pixel_start + 4]
                };
                result[pixel_start..pixel_start + 4].copy_from_slice(src);
            }
        }

        result
    }

    /// Outer shrink transition (shrink from edges inward)
    fn blend_outer(&self, new_frame: &[u8], progress: f32) -> Vec<u8> {
        let mut result = vec![0u8; self.old_frame.len()];
        let stride = self.width as usize * 4;

        // Calculate center point
        let center_x = self.width as f32 / 2.0;
        let center_y = self.height as f32 / 2.0;

        // Maximum distance from center to corner
        let max_radius = (center_x * center_x + center_y * center_y).sqrt();
        let current_radius_sq = (max_radius * (1.0 - progress)) * (max_radius * (1.0 - progress));

        for y in 0..self.height as usize {
            let row_start = y * stride;
            let dy = y as f32 - center_y;
            let dy_sq = dy * dy;

            for x in 0..self.width as usize {
                let pixel_start = row_start + x * 4;

                // Calculate squared distance from center (avoid sqrt)
                let dx = x as f32 - center_x;
                let dist_sq = dx * dx + dy_sq;

                // Copy from new frame if outside current radius, else old frame
                let src = if dist_sq > current_radius_sq {
                    &new_frame[pixel_start..pixel_start + 4]
                } else {
                    &self.old_frame[pixel_start..pixel_start + 4]
                };
                result[pixel_start..pixel_start + 4].copy_from_slice(src);
            }
        }

        result
    }

    /// Set the easing function
    #[allow(dead_code)] // Builder method for public API
    pub fn with_easing(mut self, easing: EasingFunction) -> Self {
        self.easing = easing;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easing_functions() {
        let linear = EasingFunction::Linear;
        assert_eq!(linear.apply(0.0), 0.0);
        assert_eq!(linear.apply(0.5), 0.5);
        assert_eq!(linear.apply(1.0), 1.0);

        let ease_in = EasingFunction::EaseIn;
        assert_eq!(ease_in.apply(0.0), 0.0);
        assert!(ease_in.apply(0.5) < 0.5); // Should be slower in the beginning
        assert_eq!(ease_in.apply(1.0), 1.0);
    }

    #[test]
    fn test_transition_progress() {
        let old_frame = vec![0u8; 100];
        let transition = Transition::new(
            TransitionType::Fade,
            Duration::from_millis(100),
            old_frame,
            10,
            10,
            #[cfg(feature = "gpu")]
            None,
        );

        assert!(transition.progress() >= 0.0);
        assert!(transition.progress() <= 1.0);
    }
}
