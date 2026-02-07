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
    /// Returns the blended frame data
    pub fn blend_frames(&self, new_frame: &[u8]) -> Vec<u8> {
        let progress = self.progress();

        // Try GPU blending first if available
        #[cfg(feature = "gpu")]
        {
            if let Some(ref gpu) = self.gpu_renderer {
                // Map transition type to GPU transition type
                let gpu_transition_type = match self.transition_type {
                    TransitionType::None => return new_frame.to_vec(),
                    TransitionType::Fade => 0,
                    TransitionType::WipeLeft => 1,
                    TransitionType::WipeRight => 2,
                    TransitionType::WipeTop => 3,
                    TransitionType::WipeBottom => 4,
                    TransitionType::Center => 5,
                    TransitionType::Outer => 6,
                    TransitionType::WipeAngle(_) | TransitionType::Random => {
                        // Fall back to CPU for unsupported types
                        return self.blend_frames_cpu(new_frame, progress);
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
                    Ok(blended) => return blended,
                    Err(e) => {
                        log::warn!("GPU transition blending failed: {}, falling back to CPU", e);
                    }
                }
            }
        }

        // CPU fallback
        self.blend_frames_cpu(new_frame, progress)
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
        let mut result = Vec::with_capacity(self.old_frame.len());

        // ARGB8888 format: 4 bytes per pixel
        for i in (0..self.old_frame.len()).step_by(4) {
            let old_b = self.old_frame[i] as f32;
            let old_g = self.old_frame[i + 1] as f32;
            let old_r = self.old_frame[i + 2] as f32;
            let old_a = self.old_frame[i + 3] as f32;

            let new_b = new_frame[i] as f32;
            let new_g = new_frame[i + 1] as f32;
            let new_r = new_frame[i + 2] as f32;
            let new_a = new_frame[i + 3] as f32;

            // Linear interpolation
            result.push((old_b + (new_b - old_b) * progress) as u8);
            result.push((old_g + (new_g - old_g) * progress) as u8);
            result.push((old_r + (new_r - old_r) * progress) as u8);
            result.push((old_a + (new_a - old_a) * progress) as u8);
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
        let mut result = self.old_frame.clone();
        let stride = self.width as usize * 4; // 4 bytes per pixel

        // Calculate the transition boundary (in pixels)
        let boundary = if right_to_left {
            self.width as f32 * (1.0 - progress)
        } else {
            self.width as f32 * progress
        };

        for y in 0..self.height as usize {
            let row_start = y * stride;
            for x in 0..self.width as usize {
                let pixel_start = row_start + x * 4;

                // Determine if this pixel should show new or old frame
                let show_new = if right_to_left {
                    x as f32 >= boundary
                } else {
                    (x as f32) < boundary
                };

                if show_new {
                    result[pixel_start..pixel_start + 4]
                        .copy_from_slice(&new_frame[pixel_start..pixel_start + 4]);
                }
            }
        }

        result
    }

    /// Vertical wipe transition
    fn blend_wipe_vertical(&self, new_frame: &[u8], progress: f32, bottom_to_top: bool) -> Vec<u8> {
        let mut result = self.old_frame.clone();
        let stride = self.width as usize * 4;

        // Calculate the transition boundary (in rows)
        let boundary = if bottom_to_top {
            self.height as f32 * (1.0 - progress)
        } else {
            self.height as f32 * progress
        };

        for y in 0..self.height as usize {
            // Determine if this row should show new or old frame
            let show_new = if bottom_to_top {
                y as f32 >= boundary
            } else {
                (y as f32) < boundary
            };

            if show_new {
                let row_start = y * stride;
                result[row_start..row_start + stride]
                    .copy_from_slice(&new_frame[row_start..row_start + stride]);
            }
        }

        result
    }

    /// Diagonal wipe transition at a custom angle
    fn blend_wipe_angle(&self, new_frame: &[u8], progress: f32, angle: f32) -> Vec<u8> {
        let mut result = self.old_frame.clone();
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

                // Determine if this pixel should show new or old frame
                if dist < boundary {
                    result[pixel_start..pixel_start + 4]
                        .copy_from_slice(&new_frame[pixel_start..pixel_start + 4]);
                }
            }
        }

        result
    }

    /// Center expand transition (expand from center outward)
    fn blend_center(&self, new_frame: &[u8], progress: f32) -> Vec<u8> {
        let mut result = self.old_frame.clone();
        let stride = self.width as usize * 4;

        // Calculate center point
        let center_x = self.width as f32 / 2.0;
        let center_y = self.height as f32 / 2.0;

        // Maximum distance from center to corner
        let max_radius = (center_x * center_x + center_y * center_y).sqrt();
        let current_radius = max_radius * progress;

        for y in 0..self.height as usize {
            let row_start = y * stride;
            for x in 0..self.width as usize {
                let pixel_start = row_start + x * 4;

                // Calculate distance from center
                let dx = x as f32 - center_x;
                let dy = y as f32 - center_y;
                let dist = (dx * dx + dy * dy).sqrt();

                // Show new frame if within current radius
                if dist < current_radius {
                    result[pixel_start..pixel_start + 4]
                        .copy_from_slice(&new_frame[pixel_start..pixel_start + 4]);
                }
            }
        }

        result
    }

    /// Outer shrink transition (shrink from edges inward)
    fn blend_outer(&self, new_frame: &[u8], progress: f32) -> Vec<u8> {
        let mut result = self.old_frame.clone();
        let stride = self.width as usize * 4;

        // Calculate center point
        let center_x = self.width as f32 / 2.0;
        let center_y = self.height as f32 / 2.0;

        // Maximum distance from center to corner
        let max_radius = (center_x * center_x + center_y * center_y).sqrt();
        let current_radius = max_radius * (1.0 - progress);

        for y in 0..self.height as usize {
            let row_start = y * stride;
            for x in 0..self.width as usize {
                let pixel_start = row_start + x * 4;

                // Calculate distance from center
                let dx = x as f32 - center_x;
                let dy = y as f32 - center_y;
                let dist = (dx * dx + dy * dy).sqrt();

                // Show new frame if outside current radius
                if dist > current_radius {
                    result[pixel_start..pixel_start + 4]
                        .copy_from_slice(&new_frame[pixel_start..pixel_start + 4]);
                }
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
