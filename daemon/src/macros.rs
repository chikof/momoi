//! Custom macros for reducing code repetition in momoi

/// Log an error and continue execution (non-fatal error handling)
///
/// # Example
/// ```
/// log_and_continue!(app_data.update_video_frames(&qh), "update video frames");
/// ```
#[macro_export]
macro_rules! log_and_continue {
    ($expr:expr, $context:expr) => {
        if let Err(e) = $expr {
            log::error!("Failed to {}: {}", $context, e);
        }
    };
}

/// Apply overlay to frame data, logging a warning on failure
///
/// # Example
/// ```
/// apply_overlay_or_warn!(
///     Self::apply_overlay_to_frame,
///     output_data,
///     &mut frame_data,
///     width,
///     height,
///     "video frame"
/// );
/// ```
#[macro_export]
macro_rules! apply_overlay_or_warn {
    ($apply_fn:expr, $output_data:expr, $frame_data:expr, $width:expr, $height:expr, $context:expr) => {
        if let Err(e) = $apply_fn($output_data, $frame_data, $width, $height) {
            log::warn!("Failed to apply overlay to {}: {}", $context, e);
        }
    };
}

/// Log parameters on the first frame only
///
/// # Example
/// ```
/// log_params_once!(self.frame, "Vignette", "strength" => strength);
/// log_params_once!(self.frame, "ColorTint", "r" => r, "g" => g, "b" => b, "strength" => strength);
/// ```
#[macro_export]
macro_rules! log_params_once {
    ($frame:expr, $name:expr, $($key:expr => $value:expr),+) => {
        if $frame == 1 {
            let params = vec![$(format!("{}={}", $key, $value)),+];
            log::info!("{} parameters: {}", $name, params.join(", "));
        }
    };
}

/// Create a wgpu render pipeline with standard configuration
///
/// # Example
/// ```
/// let pipeline = create_shader_pipeline!(
///     device,
///     "Plasma Pipeline",
///     plasma_shader,
///     plasma_pipeline_layout
/// );
/// ```
#[macro_export]
macro_rules! create_shader_pipeline {
    ($device:expr, $label:expr, $shader:expr, $layout:expr) => {
        $device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some($label),
            layout: Some(&$layout),
            vertex: wgpu::VertexState {
                module: &$shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &$shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        })
    };
}

/// Commit a buffer to a Wayland layer surface
///
/// # Example
/// ```
/// commit_buffer!(layer_surface, buffer, width, height);
/// ```
#[macro_export]
macro_rules! commit_buffer {
    ($layer_surface:expr, $buffer:expr, $width:expr, $height:expr) => {
        $layer_surface
            .wl_surface()
            .attach(Some($buffer.buffer()), 0, 0);
        $layer_surface
            .wl_surface()
            .damage_buffer(0, 0, $width as i32, $height as i32);
        $layer_surface.wl_surface().commit();
    };
}

/// Convert ARGB to RGBA format
///
/// # Example
/// ```
/// let rgba = convert_argb_rgba!(argb_data);
/// ```
#[macro_export]
macro_rules! convert_argb_rgba {
    ($argb_data:expr) => {{
        let mut rgba_data = vec![0u8; $argb_data.len()];
        for i in 0..($argb_data.len() / 4) {
            let offset = i * 4;
            rgba_data[offset + 0] = $argb_data[offset + 2]; // R
            rgba_data[offset + 1] = $argb_data[offset + 1]; // G
            rgba_data[offset + 2] = $argb_data[offset + 0]; // B
            rgba_data[offset + 3] = $argb_data[offset + 3]; // A
        }
        rgba_data
    }};
}

/// Validate an enum-like string value
///
/// # Example
/// ```
/// validate_enum!(transition, "none", "fade", "wipe-left", "wipe-right");
/// validate_enum!(scale, "center", "fill", "fit", "stretch", "tile");
/// ```
#[macro_export]
macro_rules! validate_enum {
    ($value:expr, $($variant:expr),+) => {
        match $value {
            $($variant)|+ => Ok(()),
            _ => anyhow::bail!("Invalid value: {} (expected one of: {})", $value, [$($variant),+].join(", ")),
        }
    };
}

/// Update shared wallpaper state for outputs
///
/// # Example
/// ```
/// update_wallpaper_state!(
///     state,
///     output_filter,
///     WallpaperType::Image(path.to_string())
/// );
/// ```
#[macro_export]
macro_rules! update_wallpaper_state {
    ($state:expr, $output_filter:expr, $wallpaper_type:expr) => {
        if let Some(filter) = $output_filter {
            if filter == "all" {
                let output_names: Vec<String> =
                    $state.outputs.iter().map(|o| o.name.clone()).collect();
                for name in output_names {
                    $state.wallpapers.insert(name, $wallpaper_type.clone());
                }
            } else {
                $state
                    .wallpapers
                    .insert(filter.to_string(), $wallpaper_type);
            }
        } else {
            let output_names: Vec<String> = $state.outputs.iter().map(|o| o.name.clone()).collect();
            for name in output_names {
                $state.wallpapers.insert(name, $wallpaper_type.clone());
            }
        }
    };
}

/// Parse transition string into TransitionType enum
///
/// # Example
/// ```
/// let transition = parse_transition!("fade", 500);
/// let transition = parse_transition!("wipe-left", duration_ms);
/// ```
#[macro_export]
macro_rules! parse_transition {
    ($transition_str:expr, $duration:expr) => {
        match $transition_str {
            "none" => common::TransitionType::None,
            "fade" => common::TransitionType::Fade {
                duration_ms: $duration as u32,
            },
            "wipe-left" => common::TransitionType::WipeLeft {
                duration_ms: $duration as u32,
            },
            "wipe-right" => common::TransitionType::WipeRight {
                duration_ms: $duration as u32,
            },
            "wipe-top" => common::TransitionType::WipeTop {
                duration_ms: $duration as u32,
            },
            "wipe-bottom" => common::TransitionType::WipeBottom {
                duration_ms: $duration as u32,
            },
            "wipe-angle" => common::TransitionType::WipeAngle {
                angle_degrees: 45.0,
                duration_ms: $duration as u32,
            },
            "center" => common::TransitionType::Center {
                duration_ms: $duration as u32,
            },
            "outer" => common::TransitionType::Outer {
                duration_ms: $duration as u32,
            },
            "random" => {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                let dur_ms = $duration as u32;
                match rng.gen_range(0..8) {
                    0 => common::TransitionType::Fade {
                        duration_ms: dur_ms,
                    },
                    1 => common::TransitionType::WipeLeft {
                        duration_ms: dur_ms,
                    },
                    2 => common::TransitionType::WipeRight {
                        duration_ms: dur_ms,
                    },
                    3 => common::TransitionType::WipeTop {
                        duration_ms: dur_ms,
                    },
                    4 => common::TransitionType::WipeBottom {
                        duration_ms: dur_ms,
                    },
                    5 => common::TransitionType::WipeAngle {
                        angle_degrees: 45.0,
                        duration_ms: dur_ms,
                    },
                    6 => common::TransitionType::Center {
                        duration_ms: dur_ms,
                    },
                    _ => common::TransitionType::Outer {
                        duration_ms: dur_ms,
                    },
                }
            }
            _ => common::TransitionType::Fade {
                duration_ms: $duration as u32,
            },
        }
    };
}
