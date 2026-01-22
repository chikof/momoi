use anyhow::Result;
use clap::{Parser, Subcommand};
use common::{Command, Response};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

#[derive(Parser)]
#[command(name = "wwctl")]
#[command(about = "Wayland Wallpaper Daemon Control", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Set wallpaper from image or video file
    Set {
        /// Path to the wallpaper file
        path: String,

        /// Target output (monitor), or 'all' for all outputs
        #[arg(short, long)]
        output: Option<String>,

        /// Transition effect
        #[arg(short, long, default_value = "fade")]
        transition: String,

        /// Transition duration in milliseconds
        #[arg(short, long, default_value = "300")]
        duration: u32,

        /// Angle for wipe-angle transition (degrees, 0=right, 90=down, 180=left, 270=up)
        #[arg(short, long, default_value = "45")]
        angle: f32,

        /// Image scaling mode (center, fill, fit, stretch, tile)
        #[arg(short, long, default_value = "fill")]
        scale: String,
    },

    /// Set solid color background
    Color {
        /// Color in hex format (e.g., #FF5733 or FF5733)
        color: String,

        /// Target output (monitor), or 'all' for all outputs
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Set animated shader wallpaper
    Shader {
        /// Shader name (plasma, waves, matrix, gradient, starfield, raymarching, tunnel)
        shader: String,

        /// Target output (monitor), or 'all' for all outputs
        #[arg(short, long)]
        output: Option<String>,

        /// Transition effect
        #[arg(short, long)]
        transition: Option<String>,

        /// Transition duration in milliseconds
        #[arg(short, long, default_value = "500")]
        duration: u32,

        /// Use a named preset from config (overrides other params)
        #[arg(short, long)]
        preset: Option<String>,

        /// Animation speed multiplier (e.g., 0.5 for half speed, 2.0 for double speed)
        #[arg(long)]
        speed: Option<f32>,

        /// Primary color (hex format: FF0000 or #FF0000)
        #[arg(long)]
        color1: Option<String>,

        /// Secondary color (hex format: 0000FF or #0000FF)
        #[arg(long)]
        color2: Option<String>,

        /// Tertiary color (hex format: 00FF00 or #00FF00)
        #[arg(long)]
        color3: Option<String>,

        /// Scale/size parameter (shader-specific meaning)
        #[arg(long)]
        scale: Option<f32>,

        /// Intensity parameter (0.0-1.0)
        #[arg(long)]
        intensity: Option<f32>,

        /// Count parameter (e.g., number of objects)
        #[arg(long)]
        count: Option<u32>,
    },

    /// Apply shader overlay effect on top of current wallpaper
    Overlay {
        /// Overlay name (vignette, scanlines, film-grain, chromatic, crt, pixelate, tint)
        overlay: String,

        /// Target output (monitor), or 'all' for all outputs
        #[arg(short, long)]
        output: Option<String>,

        /// Effect intensity (0.0-1.0) - for scanlines, film-grain, crt
        #[arg(short, long)]
        intensity: Option<f32>,

        /// Effect strength (0.0-1.0) - for vignette, tint
        #[arg(short, long)]
        strength: Option<f32>,

        /// Line width (pixels) - for scanlines effect
        #[arg(long)]
        line_width: Option<f32>,

        /// Offset (pixels) - for chromatic aberration
        #[arg(long)]
        offset: Option<f32>,

        /// Curvature (0.0-1.0) - for CRT effect
        #[arg(long)]
        curvature: Option<f32>,

        /// Pixel size - for pixelate effect
        #[arg(long)]
        pixel_size: Option<u32>,

        /// Red component (0.0-1.0) - for tint effect
        #[arg(long)]
        tint_r: Option<f32>,

        /// Green component (0.0-1.0) - for tint effect
        #[arg(long)]
        tint_g: Option<f32>,

        /// Blue component (0.0-1.0) - for tint effect
        #[arg(long)]
        tint_b: Option<f32>,
    },

    /// Clear shader overlay
    ClearOverlay {
        /// Target output (monitor), or 'all' for all outputs
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Query daemon status and outputs
    Query,

    /// Kill the running daemon
    Kill,

    /// List available outputs (monitors)
    ListOutputs,

    /// Ping the daemon to check if it's running
    Ping,

    /// Playlist commands
    Playlist {
        #[command(subcommand)]
        action: PlaylistCommands,
    },

    /// Show resource usage and performance mode
    Resources,
}

#[derive(Subcommand)]
enum PlaylistCommands {
    /// Move to next wallpaper in playlist
    Next,

    /// Move to previous wallpaper in playlist
    Prev,

    /// Toggle shuffle mode
    Shuffle,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let command = match cli.command {
        Commands::Set {
            path,
            output,
            transition,
            duration,
            angle,
            scale,
        } => {
            let transition_type = parse_transition(&transition, duration, angle);
            let scale_mode = parse_scale_mode(&scale);
            Command::SetWallpaper {
                path,
                output,
                transition: Some(transition_type),
                scale: Some(scale_mode),
            }
        }
        Commands::Color { color, output } => Command::SetColor { color, output },
        Commands::Shader {
            shader,
            output,
            transition,
            duration,
            preset,
            speed,
            color1,
            color2,
            color3,
            scale,
            intensity,
            count,
        } => {
            let transition_type = transition.map(|t| parse_transition(&t, duration, 45.0));

            // If preset is specified, send preset name in params
            // The daemon will look it up from config
            let params = if let Some(preset_name) = preset {
                // Create params with preset name in a special field
                // We'll use color1 as a special marker: "preset:name"
                Some(common::ShaderParams {
                    speed: None,
                    color1: Some(format!("preset:{}", preset_name)),
                    color2: None,
                    color3: None,
                    scale: None,
                    intensity: None,
                    count: None,
                })
            } else if speed.is_some()
                || color1.is_some()
                || color2.is_some()
                || color3.is_some()
                || scale.is_some()
                || intensity.is_some()
                || count.is_some()
            {
                Some(common::ShaderParams {
                    speed,
                    color1,
                    color2,
                    color3,
                    scale,
                    intensity,
                    count,
                })
            } else {
                None
            };

            Command::SetShader {
                shader,
                output,
                transition: transition_type,
                params,
            }
        }
        Commands::Overlay {
            overlay,
            output,
            intensity,
            strength,
            line_width,
            offset,
            curvature,
            pixel_size,
            tint_r,
            tint_g,
            tint_b,
        } => {
            // Only create params if at least one parameter is specified
            let params = if intensity.is_some()
                || strength.is_some()
                || line_width.is_some()
                || offset.is_some()
                || curvature.is_some()
                || pixel_size.is_some()
                || tint_r.is_some()
                || tint_g.is_some()
                || tint_b.is_some()
            {
                Some(common::OverlayParams {
                    intensity,
                    strength,
                    line_width,
                    offset,
                    curvature,
                    pixel_size,
                    r: tint_r,
                    g: tint_g,
                    b: tint_b,
                })
            } else {
                None
            };

            Command::SetOverlay {
                overlay,
                params,
                output,
            }
        }
        Commands::ClearOverlay { output } => Command::ClearOverlay { output },
        Commands::Query => Command::Query,
        Commands::Kill => Command::Kill,
        Commands::ListOutputs => Command::ListOutputs,
        Commands::Ping => Command::Ping,
        Commands::Playlist { action } => match action {
            PlaylistCommands::Next => Command::PlaylistNext,
            PlaylistCommands::Prev => Command::PlaylistPrev,
            PlaylistCommands::Shuffle => Command::PlaylistToggleShuffle,
        },
        Commands::Resources => Command::GetResources,
    };

    match send_command(command).await {
        Ok(response) => {
            handle_response(response);
            Ok(())
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("\nIs the daemon running? Try starting it with: momoi");
            std::process::exit(1);
        }
    }
}

async fn send_command(command: Command) -> Result<Response> {
    let socket_path = common::get_socket_path();

    let stream = UnixStream::connect(&socket_path).await?;
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // Send command
    let command_json = serde_json::to_string(&command)?;
    writer.write_all(command_json.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;

    // Read response
    let mut response_line = String::new();
    reader.read_line(&mut response_line).await?;

    let response: Response = serde_json::from_str(&response_line)?;
    Ok(response)
}

fn handle_response(response: Response) {
    match response {
        Response::Ok => {
            println!("✓ Success");
        }
        Response::Error(e) => {
            eprintln!("✗ Error: {}", e);
            std::process::exit(1);
        }
        Response::Status(status) => {
            println!("Daemon Status:");
            println!("  Version: {}", status.version);
            println!("  Uptime: {}s", status.uptime_secs);
            println!("  Current Wallpapers:");
            for wp in status.current_wallpapers {
                println!("    {} -> {:?}", wp.output, wp.wallpaper);
            }
        }
        Response::Outputs(outputs) => {
            println!("Available Outputs:");
            for output in outputs {
                println!(
                    "  {} - {}x{} (scale: {})",
                    output.name, output.width, output.height, output.scale
                );
            }
        }
        Response::Pong => {
            println!("✓ Daemon is running");
        }
        Response::Resources(res) => {
            println!("Resource Status:");
            println!("  Performance Mode: {}", res.performance_mode);
            println!("  Memory Usage: {} MB", res.memory_mb);
            println!("  CPU Usage: {:.1}%", res.cpu_percent);
            println!("  Power: {}", if res.on_battery { "Battery" } else { "AC" });
            if let Some(pct) = res.battery_percent {
                println!("  Battery: {}%", pct);
            }
        }
    }
}

fn parse_transition(name: &str, duration_ms: u32, angle: f32) -> common::TransitionType {
    match name.to_lowercase().as_str() {
        "none" => common::TransitionType::None,
        "fade" => common::TransitionType::Fade { duration_ms },
        "wipe-left" | "left" => common::TransitionType::WipeLeft { duration_ms },
        "wipe-right" | "right" => common::TransitionType::WipeRight { duration_ms },
        "wipe-top" | "top" => common::TransitionType::WipeTop { duration_ms },
        "wipe-bottom" | "bottom" => common::TransitionType::WipeBottom { duration_ms },
        "wipe-angle" | "angle" | "diagonal" => common::TransitionType::WipeAngle {
            angle_degrees: angle,
            duration_ms,
        },
        "center" => common::TransitionType::Center { duration_ms },
        "outer" => common::TransitionType::Outer { duration_ms },
        "random" => common::TransitionType::Random { duration_ms },
        _ => {
            eprintln!("Warning: Unknown transition '{}', using 'fade'", name);
            common::TransitionType::Fade { duration_ms }
        }
    }
}

fn parse_scale_mode(name: &str) -> common::ScaleMode {
    match name.to_lowercase().as_str() {
        "center" => common::ScaleMode::Center,
        "fill" => common::ScaleMode::Fill,
        "fit" => common::ScaleMode::Fit,
        "stretch" => common::ScaleMode::Stretch,
        "tile" => common::ScaleMode::Tile,
        _ => {
            eprintln!("Warning: Unknown scale mode '{}', using 'fill'", name);
            common::ScaleMode::Fill
        }
    }
}
