use anyhow::Result;
use std::time::Instant;

/// Built-in shader types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinShader {
    /// Animated plasma effect
    Plasma,
    /// Sine wave pattern
    Waves,
    /// Matrix-style rain effect
    Matrix,
    /// Gradient animation
    Gradient,
    /// Starfield effect
    Starfield,
    /// 3D raymarching scene
    Raymarching,
    /// Infinite tunnel vortex
    Tunnel,
}

impl BuiltinShader {
    /// Parse shader name from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "plasma" => Some(BuiltinShader::Plasma),
            "waves" => Some(BuiltinShader::Waves),
            "matrix" => Some(BuiltinShader::Matrix),
            "gradient" => Some(BuiltinShader::Gradient),
            "starfield" => Some(BuiltinShader::Starfield),
            "raymarching" | "raymarch" => Some(BuiltinShader::Raymarching),
            "tunnel" | "vortex" => Some(BuiltinShader::Tunnel),
            _ => None,
        }
    }

    pub fn name(&self) -> &str {
        #[allow(dead_code)] // Part of public API for shader name lookup
        match self {
            BuiltinShader::Plasma => "plasma",
            BuiltinShader::Waves => "waves",
            BuiltinShader::Matrix => "matrix",
            BuiltinShader::Gradient => "gradient",
            BuiltinShader::Starfield => "starfield",
            BuiltinShader::Raymarching => "raymarching",
            BuiltinShader::Tunnel => "tunnel",
        }
    }
}

/// Shader context with uniforms
#[derive(Debug, Clone)]
#[allow(dead_code)] // Context fields used for shader uniform data
pub struct ShaderContext {
    /// Current time in seconds since shader start
    pub time: f32,
    /// Screen resolution (width, height)
    pub resolution: (u32, u32),
    /// Mouse position (normalized 0-1), None if not tracked
    pub mouse: Option<(f32, f32)>,
    /// Frame number
    pub frame: u64,
}

impl ShaderContext {
    pub fn new(width: u32, height: u32) -> Self {
        ShaderContext {
            time: 0.0,
            resolution: (width, height),
            mouse: None,
            frame: 0,
        }
    }

    /// Update time based on elapsed duration
    #[allow(dead_code)] // Used for time-based shader animations
    pub fn update(&mut self, elapsed: f32) {
        self.time += elapsed;
        self.frame += 1;
    }
}

/// Software shader renderer
pub struct ShaderManager {
    /// Current shader being rendered
    shader: BuiltinShader,
    /// Shader context (uniforms)
    context: ShaderContext,
    /// Shader parameters for customization
    params: common::ShaderParams,
    /// Start time for animation
    start_time: Instant,
    /// Target frame rate
    target_fps: u32,
    /// Last frame time
    last_frame: Instant,
    /// Optional GPU renderer for accelerated rendering
    #[cfg(feature = "gpu")]
    gpu_renderer: Option<std::sync::Arc<crate::gpu::GpuRenderer>>,
}

impl ShaderManager {
    /// Create a new shader manager
    pub fn new(
        shader: BuiltinShader,
        width: u32,
        height: u32,
        params: Option<common::ShaderParams>,
        #[cfg(feature = "gpu")] gpu_renderer: Option<std::sync::Arc<crate::gpu::GpuRenderer>>,
    ) -> Self {
        ShaderManager {
            shader,
            context: ShaderContext::new(width, height),
            params: params.unwrap_or_default(),
            start_time: Instant::now(),
            target_fps: 30, // Default to 30fps for shaders
            last_frame: Instant::now(),
            #[cfg(feature = "gpu")]
            gpu_renderer,
        }
    }

    /// Set target frame rate
    #[allow(dead_code)] // Part of public API for FPS control
    pub fn set_fps(&mut self, fps: u32) {
        self.target_fps = fps;
    }

    /// Check if it's time to render next frame
    pub fn should_render(&self) -> bool {
        let frame_duration = std::time::Duration::from_millis(1000 / self.target_fps as u64);
        self.last_frame.elapsed() >= frame_duration
    }

    /// Render current frame to ARGB buffer
    pub fn render_frame(&mut self, width: u32, height: u32) -> Result<Vec<u8>> {
        // Update context
        self.context.resolution = (width, height);
        let elapsed = self.start_time.elapsed().as_secs_f32();
        self.context.time = elapsed;
        self.context.frame += 1;
        self.last_frame = Instant::now();

        // Try GPU rendering first if available
        #[cfg(feature = "gpu")]
        {
            if let Some(ref gpu) = self.gpu_renderer {
                // GPU-accelerated shaders
                let shader_name = match self.shader {
                    BuiltinShader::Plasma => Some("plasma"),
                    BuiltinShader::Waves => Some("waves"),
                    BuiltinShader::Gradient => Some("gradient"),
                    BuiltinShader::Starfield => Some("starfield"),
                    BuiltinShader::Matrix => Some("matrix"),
                    BuiltinShader::Raymarching => Some("raymarching"),
                    BuiltinShader::Tunnel => Some("tunnel"),
                };

                if let Some(name) = shader_name {
                    let start = std::time::Instant::now();
                    match gpu.render_shader(name, width, height, elapsed, &self.params) {
                        Ok(data) => {
                            log::info!(
                                "GPU shader '{}' rendered {}x{} in {:.2}ms",
                                name,
                                width,
                                height,
                                start.elapsed().as_secs_f32() * 1000.0
                            );
                            return Ok(data);
                        }
                        Err(e) => {
                            log::warn!("GPU shader rendering failed: {}, falling back to CPU", e);
                        }
                    }
                }
            }
        }

        // CPU fallback (always available)
        let start = std::time::Instant::now();
        let buffer = match self.shader {
            BuiltinShader::Plasma => self.render_plasma(width, height),
            BuiltinShader::Waves => self.render_waves(width, height),
            BuiltinShader::Matrix => self.render_matrix(width, height),
            BuiltinShader::Gradient => self.render_gradient(width, height),
            BuiltinShader::Starfield => self.render_starfield(width, height),
            BuiltinShader::Raymarching => self.render_fallback(width, height, "Raymarching"),
            BuiltinShader::Tunnel => self.render_fallback(width, height, "Tunnel"),
        };
        log::info!(
            "CPU shader '{:?}' rendered {}x{} in {:.2}ms",
            self.shader,
            width,
            height,
            start.elapsed().as_secs_f32() * 1000.0
        );

        Ok(buffer)
    }

    /// Fallback for GPU-only shaders (too complex for CPU)
    fn render_fallback(&self, width: u32, height: u32, name: &str) -> Vec<u8> {
        let mut buffer = vec![0u8; (width * height * 4) as usize];

        // Simple gradient as fallback
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;
                let intensity = ((x + y) as f32 / (width + height) as f32 * 255.0) as u8;

                buffer[idx] = intensity / 2; // B
                buffer[idx + 1] = intensity / 2; // G
                buffer[idx + 2] = intensity / 2; // R
                buffer[idx + 3] = 255; // A
            }
        }

        log::warn!(
            "{} shader requires GPU acceleration, showing simple fallback",
            name
        );
        buffer
    }

    /// Render plasma effect
    fn render_plasma(&self, width: u32, height: u32) -> Vec<u8> {
        let mut buffer = vec![0u8; (width * height * 4) as usize];
        let time = self.context.time;

        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;

                // Normalized coordinates
                let nx = x as f32 / width as f32;
                let ny = y as f32 / height as f32;

                // Plasma effect using sine waves
                let v1 = (nx * 10.0 + time).sin();
                let v2 = (ny * 10.0 - time).sin();
                let v3 = ((nx + ny) * 10.0 + time).sin();
                let v4 = ((nx * nx + ny * ny).sqrt() * 10.0 - time).sin();

                let value = (v1 + v2 + v3 + v4) / 4.0;

                // Map to RGB colors
                let r = ((value.sin() * 0.5 + 0.5) * 255.0) as u8;
                let g = (((value + 2.0).sin() * 0.5 + 0.5) * 255.0) as u8;
                let b = (((value + 4.0).sin() * 0.5 + 0.5) * 255.0) as u8;

                buffer[idx] = b; // B
                buffer[idx + 1] = g; // G
                buffer[idx + 2] = r; // R
                buffer[idx + 3] = 255; // A
            }
        }

        buffer
    }

    /// Render wave effect
    fn render_waves(&self, width: u32, height: u32) -> Vec<u8> {
        let mut buffer = vec![0u8; (width * height * 4) as usize];
        let time = self.context.time;

        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;

                let nx = x as f32 / width as f32;
                let ny = y as f32 / height as f32;

                // Wave patterns
                let wave1 = (nx * 20.0 + time * 2.0).sin();
                let wave2 = (ny * 15.0 - time * 1.5).sin();
                let value = (wave1 + wave2) / 2.0;

                // Blue to cyan gradient based on wave
                let intensity = (value * 0.5 + 0.5) * 255.0;
                buffer[idx] = intensity as u8; // B
                buffer[idx + 1] = (intensity * 0.7) as u8; // G
                buffer[idx + 2] = (intensity * 0.3) as u8; // R
                buffer[idx + 3] = 255;
            }
        }

        buffer
    }

    /// Render matrix rain effect
    fn render_matrix(&self, width: u32, height: u32) -> Vec<u8> {
        let mut buffer = vec![0u8; (width * height * 4) as usize];
        let time = self.context.time;

        // Simple vertical green lines falling
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;

                // Create vertical streams
                let column_seed = (x as f32 * 0.1).sin() * 1000.0;
                let fall_speed = 100.0 + column_seed;
                let y_offset = (time * fall_speed + column_seed) % (height as f32 * 2.0);

                let dist = (y as f32 - y_offset).abs();
                let brightness = if dist < 20.0 {
                    (1.0 - dist / 20.0) * 255.0
                } else {
                    0.0
                };

                buffer[idx] = 0; // B
                buffer[idx + 1] = brightness as u8; // G
                buffer[idx + 2] = 0; // R
                buffer[idx + 3] = 255;
            }
        }

        buffer
    }

    /// Render gradient animation
    fn render_gradient(&self, width: u32, height: u32) -> Vec<u8> {
        let mut buffer = vec![0u8; (width * height * 4) as usize];
        let time = self.context.time;

        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;

                let nx = x as f32 / width as f32;
                let ny = y as f32 / height as f32;

                // Rotating gradient
                let angle = time * 0.5;
                let gradient = (nx * angle.cos() + ny * angle.sin() + time * 0.3).sin() * 0.5 + 0.5;

                let r = (gradient * 255.0) as u8;
                let g = ((1.0 - gradient) * 255.0) as u8;
                let b = ((nx + ny) * 0.5 * 255.0) as u8;

                buffer[idx] = b;
                buffer[idx + 1] = g;
                buffer[idx + 2] = r;
                buffer[idx + 3] = 255;
            }
        }

        buffer
    }

    /// Render starfield effect
    fn render_starfield(&self, width: u32, height: u32) -> Vec<u8> {
        let mut buffer = vec![0u8; (width * height * 4) as usize];
        let time = self.context.time;

        // Generate pseudo-random stars
        let star_count = 200;

        for i in 0..star_count {
            // Pseudo-random position based on index
            let seed = i as f32 * 12.9898;
            let px = (seed.sin() * 43758.5453).fract();
            let py = ((seed + 1.0).sin() * 43758.5453).fract();

            // Animate star position (moving towards viewer)
            let z = ((time * 0.5 + seed) % 2.0) - 1.0;
            let scale = 1.0 / (z + 2.0);

            let sx = ((px - 0.5) * scale + 0.5) * width as f32;
            let sy = ((py - 0.5) * scale + 0.5) * height as f32;

            let x = sx as i32;
            let y = sy as i32;

            if x >= 0 && x < width as i32 && y >= 0 && y < height as i32 {
                let idx = ((y as u32 * width + x as u32) * 4) as usize;
                if idx + 3 < buffer.len() {
                    let brightness = ((1.0 - z) * 255.0) as u8;
                    buffer[idx] = brightness; // B
                    buffer[idx + 1] = brightness; // G
                    buffer[idx + 2] = brightness; // R
                    buffer[idx + 3] = 255;
                }
            }
        }

        buffer
    }

    /// Get current shader type
    #[allow(dead_code)] // Part of public API for shader queries
    pub fn shader(&self) -> BuiltinShader {
        self.shader
    }

    /// Change shader
    #[allow(dead_code)] // Part of public API for shader switching
    pub fn set_shader(&mut self, shader: BuiltinShader) {
        if self.shader != shader {
            self.shader = shader;
            self.start_time = Instant::now();
            self.context.time = 0.0;
            self.context.frame = 0;
            log::info!("Switched to shader: {}", shader.name());
        }
    }
}
