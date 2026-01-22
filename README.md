# Momoi

An advanced Wayland wallpaper daemon with GPU-accelerated rendering, procedural shaders, images, animations, and videos.

## Features

- 🖼️ **Static Images**: PNG, JPEG, WebP, BMP, TIFF, SVG
- 🎬 **Animated Content**: GIF support with efficient frame caching and GPU rendering
- 🎥 **Video Wallpapers**: MP4, WebM, MKV, AVI, MOV with GStreamer-based playback (muted by default)
- 🎨 **GPU Shaders**: 7 customizable procedural shaders (plasma, waves, gradient, starfield, matrix, raymarching, tunnel)
- ⚙️ **Shader Parameters**: Full customization via CLI flags or config presets (speed, colors, scale, intensity, count)
- 🎭 **Post-Processing Overlays**: 7 overlay effects (vignette, scanlines, film-grain, chromatic aberration, CRT, pixelate, color tint)
- ✨ **Smooth Transitions**: 10 GPU-accelerated transition types (fade, wipes, diagonal, center, outer, random)
- 📐 **Scaling Modes**: Fill, Fit, Center, Stretch, Tile
- 🖥️ **Multi-Monitor**: Full support for multiple outputs with per-monitor wallpapers
- ⚡ **Runtime Control**: Change wallpapers without restarting the daemon
- 🔄 **Auto-Reconnection**: Automatically reconnects and restores wallpapers if compositor restarts
- 📋 **Playlist Mode**: Automatic wallpaper rotation with shuffle support
- 🕐 **Time-Based Scheduling**: Switch wallpapers based on time of day
- ⚙️ **Configuration File**: TOML-based config for playlists, schedules, shader presets, and per-monitor settings
- 💪 **Resource Management**: Performance modes (performance/balanced/powersave) with battery detection
- 🔧 **Low Resource Usage**: Optimized for performance and memory efficiency with Vulkan backend

## Project Status

✅ **Phase 8 & 9 (GPU Acceleration & Testing) - COMPLETE!** See [ROADMAP.md](./ROADMAP.md) for detailed development plan.

### What's Working

**Media Support:**

- Static images (PNG, JPEG, WebP, BMP, TIFF, GIF static, SVG)
- Animated GIFs with GPU-accelerated rendering
- Video playback (MP4, WebM, MKV, AVI, MOV) with GStreamer

**GPU Features:**

- 7 procedural shaders with full parameter customization
- 7 post-processing overlay effects (vignette, scanlines, film-grain, chromatic, CRT, pixelate, tint)
- GPU-accelerated transitions (10 types)
- Vulkan backend for optimal performance
- 26-29ms frame time @ 2560x1440 (well under 60 FPS budget)

**System Features:**

- Multi-monitor support with per-output control
- CLI tool (`wwctl`) for runtime control
- Configuration file support (TOML format)
- Playlist mode with automatic rotation and shuffle
- Time-based scheduling for dynamic wallpaper changes
- Resource management with performance modes
- Comprehensive testing (37 automated tests)

### Recent Features

- 🎨 **GPU Shader System**: 7 customizable procedural shaders
- 🎭 **Post-Processing Overlays**: 7 overlay effects with full parameter control
- ⚙️ **Shader Parameters**: CLI flags and config presets for customization
- ✅ **Comprehensive Tests**: 37 automated tests covering core functionality
- 📚 **Documentation**: Architecture, troubleshooting, testing, and contribution guides
- 🔋 **Resource Management**: Auto-switching performance modes based on battery status

## Requirements

- NixOS or a system with Nix package manager
- Wayland compositor with `wlr-layer-shell` support (Sway, Hyprland, River, etc.)
- **GPU with Vulkan support** (for shader features)
- **Note**: Does not support GNOME (no wlr-layer-shell implementation)

## Building

### With Nix (Recommended)

Enter the development environment:

```bash
nix develop
```

Build the project:

```bash
cargo build --release
```

Build with video support (required for MP4, WebM, etc.):

```bash
cargo build --release --features video
```

Build with all features (video + GPU acceleration):

```bash
cargo build --release --features all
```

Build the Nix package:

```bash
nix build
```

**Note**: The project includes a workaround for Rust 1.92.0 linker issues via `.cargo/config.toml`. See [KNOWN_ISSUES.md](./KNOWN_ISSUES.md) for details.

### Manual Build

Ensure you have the following dependencies:

- Rust 1.75.0 or later
- wayland-client
- wayland-protocols
- pkg-config
- GStreamer (for video support) with plugins: base, good, bad, ugly, libav

```bash
# Build without video support
cargo build --release

# Build with video support
cargo build --release --features video
```

## Usage

### Start the daemon:

```bash
# Without video support
momoi

# With video support (if built with --features video)
momoi
```

### Control with the client:

```bash
# Set a static image wallpaper
wwctl set /path/to/wallpaper.png

# Set an animated GIF (auto-detected)
wwctl set /path/to/animation.gif

# Set a video wallpaper (requires video feature)
wwctl set /path/to/video.mp4

# Set wallpaper with smooth transition
wwctl set /path/to/wallpaper.png --transition fade --duration 500

# Use different transition effects
wwctl set image.jpg --transition wipe-left --duration 1000
wwctl set image.jpg --transition wipe-right --duration 800
wwctl set image.jpg --transition wipe-top --duration 600
wwctl set image.jpg --transition none  # Instant, no transition

# Set wallpaper with specific scaling mode
wwctl set /path/to/wallpaper.png --scale fit

# Set wallpaper on specific output
wwctl set /path/to/wallpaper.mp4 --output DP-1

# Set different wallpapers on different outputs
wwctl set /path/to/landscape.jpg --output DP-1 --scale fill
wwctl set /path/to/portrait.jpg --output DP-2 --scale fit

# Set solid color background
wwctl color FF5733

# Query daemon status
wwctl query

# List available outputs
wwctl list-outputs

# Stop the daemon
wwctl kill

# Playlist commands
wwctl playlist next      # Skip to next wallpaper
wwctl playlist prev      # Go to previous wallpaper
wwctl playlist shuffle   # Toggle shuffle mode

# GPU Shader wallpapers
wwctl shader plasma                          # Use plasma shader with defaults
wwctl shader starfield --count 500           # Starfield with 500 stars
wwctl shader waves --speed 2.0               # Fast-moving waves
wwctl shader matrix --color1 00FF00          # Green matrix effect
wwctl shader gradient --preset sunset        # Use 'sunset' preset from config

# Shader with custom parameters
wwctl shader plasma --speed 2.0 --color1 FF0000 --color2 0000FF --scale 1.5

# All available shaders:
# - plasma: Smooth flowing plasma patterns
# - waves: Sine wave animations
# - gradient: Rotating color gradients
# - starfield: Animated starfield with parallax
# - matrix: Matrix-style falling code
# - raymarching: 3D scenes via raymarching
# - tunnel: Infinite tunnel effect

# Post-processing overlay effects
wwctl overlay vignette --strength 0.8              # Vignette darkening effect
wwctl overlay scanlines --intensity 0.4 --line-width 2.0  # CRT-style scanlines
wwctl overlay film-grain --intensity 0.2           # Film grain noise
wwctl overlay chromatic --offset 3.0               # Chromatic aberration
wwctl overlay crt --curvature 0.15 --intensity 0.3 # CRT monitor effect
wwctl overlay pixelate --pixel-size 8              # Pixelate effect
wwctl overlay tint --tint-r 1.0 --tint-g 0.8 --tint-b 0.6 --strength 0.4  # Sepia tint
wwctl clear-overlay                                # Remove overlay effect

# All overlay effects work on top of any wallpaper (images, GIFs, videos, shaders)

# Resource management
wwctl resources                              # Show resource usage stats
wwctl set-performance-mode powersave         # Switch to power-save mode
wwctl set-performance-mode balanced          # Balanced performance
wwctl set-performance-mode performance       # Maximum performance
```

### Configuration File

The daemon supports a TOML configuration file for advanced features like playlists and time-based scheduling.

**Quick Start:**

```bash
# Copy example config
mkdir -p ~/.config/momoi
cp config.toml.example ~/.config/momoi/config.toml

# Edit the config
$EDITOR ~/.config/momoi/config.toml

# Restart daemon to load config
wwctl kill
momoi
```

**Example Configuration:**

```toml
[general]
log_level = "info"
default_transition = "fade"
default_duration = 500

[playlist]
enabled = true
interval = 300  # Rotate every 5 minutes
shuffle = true
transition = "random"
sources = [
    "~/Pictures/Wallpapers",
    "~/Wallpapers/*.{jpg,png}",
]

[[schedule]]
name = "Day"
start_time = "07:00"
end_time = "19:00"
wallpaper = "~/Wallpapers/day.jpg"

[[schedule]]
name = "Night"
start_time = "19:00"
end_time = "07:00"
wallpaper = "~/Wallpapers/night.jpg"

# Shader presets for easy reuse
[[shader_preset]]
name = "calm"
shader = "plasma"
description = "Slow, dark plasma effect"
speed = 0.5
color1 = "1a1a2e"
color2 = "16213e"

[[shader_preset]]
name = "hyperspace"
shader = "starfield"
description = "Fast-moving starfield"
speed = 3.0
count = 500
color1 = "FFFFFF"

[[shader_preset]]
name = "matrix-green"
shader = "matrix"
description = "Classic Matrix green"
speed = 1.5
color1 = "00FF00"
count = 200
```

**See [CONFIGURATION.md](./CONFIGURATION.md) for complete documentation.**

### Supported Formats

**Images:**

- PNG, JPEG, WebP, BMP, TIFF, SVG

**Animated:**

- GIF (with frame timing and looping)

**Video** (requires `--features video`):

- MP4, WebM, MKV, AVI, MOV, FLV, WMV, M4V, OGV
- Videos are muted by default
- Automatic looping enabled

### Scaling Modes

Choose how images are displayed on your screen:

- **fill** (default): Scale to fill entire screen, may crop edges
- **fit**: Scale to fit within screen, maintains aspect ratio with letterboxing
- **center**: Display at original size, centered on screen
- **stretch**: Stretch to fill screen, may distort aspect ratio
- **tile**: Repeat image to cover screen

```bash
# Examples
wwctl set image.jpg --scale fill     # Fills screen, may crop
wwctl set image.jpg --scale fit      # Fits within screen, black bars if needed
wwctl set image.jpg --scale center   # Original size, centered
wwctl set image.jpg --scale stretch  # Fills screen, may distort
wwctl set image.jpg --scale tile     # Repeats image like tiles
```

### Transitions

Smooth, cinematic wallpaper transitions rendered at 60 FPS:

**Available Transitions:**

- **fade** - Smooth alpha blend between wallpapers
- **wipe-left** - New wallpaper wipes in from left to right
- **wipe-right** - New wallpaper wipes in from right to left
- **wipe-top** - New wallpaper wipes in from top to bottom
- **wipe-bottom** - New wallpaper wipes in from bottom to top
- **wipe-angle** - Diagonal wipe at custom angle (use `--angle <degrees>`)
- **center** - Expand from center outward (circular reveal)
- **outer** - Shrink from edges inward
- **random** - Randomly select a transition type
- **none** - Instant change (no transition)

**Usage:**

```bash
# Quick fade (500ms)
wwctl set image2.jpg --transition fade --duration 500

# Slow cinematic fade (2 seconds)
wwctl set image3.jpg --transition fade --duration 2000

# Wipe effects
wwctl set image4.jpg --transition wipe-left --duration 800
wwctl set image5.jpg --transition wipe-right --duration 1000
wwctl set image6.jpg --transition wipe-top --duration 600

# Diagonal wipe at 45°
wwctl set image7.jpg --transition wipe-angle --duration 1000 --angle 45

# Center expand
wwctl set image8.jpg --transition center --duration 1200

# Outer shrink
wwctl set image9.jpg --transition outer --duration 1000

# Random (surprise me!)
wwctl set image10.jpg --transition random --duration 800

# Instant change (no transition)
wwctl set image11.jpg --transition none
```

**Testing:**
Run the included test script to see all transitions in action:

```bash
./test-transitions.sh
```

See [TRANSITIONS.md](./TRANSITIONS.md) for complete transition documentation, performance characteristics, and implementation details.

## Installation

### NixOS

Add to your `flake.nix`:

```nix
{
  inputs.momoi.url = "github:chikof/momoi";
}
```

Then in your `configuration.nix`:

```nix
{
  environment.systemPackages = [
    inputs.momoi.packages.${pkgs.system}.default
  ];

  # Or use as a service
  services.momoi = {
    enable = true;
    wallpaperPath = "/path/to/wallpaper.png";
  };
}
```

## Architecture

The project consists of three main components:

- **daemon**: The main wallpaper daemon that manages Wayland surfaces and renders wallpapers
- **client** (`wwctl`): CLI tool for controlling the daemon
- **common**: Shared code and protocol definitions

## Development

See [ROADMAP.md](./ROADMAP.md) for the complete development plan.

### Performance Optimizations

The daemon includes several optimizations for smooth, efficient video playback:

- **Multi-threaded Rendering**: Parallel frame processing for multiple monitors using Rayon
- **Automatic FPS Detection**: Detects video framerate from GStreamer and adapts polling accordingly
- **Adaptive Event Loop**: Dynamic sleep timing based on content type (1-16ms)
- **Frame Change Detection**: Atomic flags prevent redundant rendering
- **Buffer Reuse**: Shared memory buffers are reused between frames
- **Zero-Copy Color Conversion**: Direct memory copy for BGRA format
- **Smart Frame Dropping**: Graceful degradation under CPU load

See [OPTIMIZATIONS.md](./OPTIMIZATIONS.md) for detailed performance metrics and analysis.

**Typical Performance** (2560x1440 + 1080x1920 monitors, H.264 1080p video on both):

- CPU Usage: 5-10% total (vs 25-40% unoptimized)
- Frame Drop Rate: <0.1% per output
- Memory: ~80MB stable (2 outputs)
- Multi-monitor Speedup: 75% faster than sequential

### Development Shell

```bash
nix develop
```

This provides all necessary dependencies and tools:

- Rust toolchain with rust-analyzer
- Wayland development libraries
- Video processing libraries
- Debugging tools

### Running Tests

```bash
# Run all tests (37 automated tests)
cargo test --all

# Run specific test suites
cargo test --package common
cargo test --bin momoi
cargo test --test ipc_integration

# With verbose output
RUST_LOG=debug cargo test -- --nocapture
```

**See [TESTING.md](./TESTING.md) for comprehensive testing guide.**

### Linting

```bash
cargo clippy --all-features
cargo fmt --all
```

## Documentation

- **[README.md](./README.md)** - This file, project overview
- **[CONFIGURATION.md](./CONFIGURATION.md)** - Complete configuration guide
- **[ARCHITECTURE.md](./ARCHITECTURE.md)** - System architecture and design
- **[TESTING.md](./TESTING.md)** - Testing guide and procedures
- **[TROUBLESHOOTING.md](./TROUBLESHOOTING.md)** - Common issues and solutions
- **[CONTRIBUTING.md](./CONTRIBUTING.md)** - Contribution guidelines
- **[ROADMAP.md](./ROADMAP.md)** - Development roadmap and progress
- **[FEATURES.md](./FEATURES.md)** - Detailed feature documentation
- **[OPTIMIZATIONS.md](./OPTIMIZATIONS.md)** - Performance optimizations

## Roadmap Status

- ✅ Phase 1: Foundation & Architecture (Complete)
- ✅ Phase 2: Basic Daemon Implementation (Complete)
- ✅ Phase 3: Client CLI Tool (Complete)
- ✅ Phase 4: Animated GIF Support (Complete)
- ✅ Phase 5: Video Wallpaper Support (Complete)
- ✅ Phase 6: Advanced Transitions (Complete - 10 transition types)
- ✅ Phase 7: Configuration & Playlists (Complete)
- ✅ Phase 8: GPU Acceleration (Complete - 7 shaders, full customization)
- ✅ Phase 9: Testing & Documentation (Complete - 37 tests, comprehensive docs)
- 🔨 Phase 10: Distribution & Community (In Progress)

**See [ROADMAP.md](./ROADMAP.md) for detailed progress and future plans.**

## Inspiration

This project is inspired by:

- [awww](https://codeberg.org/LGFae/awww) - Efficient animated wallpaper daemon for Wayland
- [Wallpaper Engine](https://www.wallpaperengine.io/) - Advanced wallpaper application
- [mpvpaper](https://github.com/GhostNaN/mpvpaper) - Video wallpaper program for wlroots

## License

MIT License - see [LICENSE](./LICENSE) file for details

## Contributing

Contributions are welcome! We appreciate bug reports, feature requests, code contributions, and documentation improvements.

**Please read [CONTRIBUTING.md](./CONTRIBUTING.md) for:**

- Code of conduct
- Development setup
- Coding standards
- Pull request process
- Testing requirements

**Quick start for contributors:**

```bash
# Fork and clone the repository
git clone https://github.com/chikof/momoi.git
cd momoi

# Set up development environment
nix develop

# Make changes, add tests
cargo test --all

# Submit pull request
```

## Acknowledgments

- The [Smithay](https://github.com/Smithay) project for Wayland client toolkit
- The awww project for architectural inspiration
- The Rust community for excellent libraries
