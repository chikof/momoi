<div align="center">
<picture>
    <img alt="momoi image" src="https://cdn3.emoji.gg/emojis/99943-momoi-blue-archive.png" width="200">
</picture>
</div>

---

<div align="center">

[Documentation](./CONFIGURATION.md)
| [Contributing](#-contributing)
| [License](#-license)

</div>

## 📚 Overview

Welcome to Momoi!

First of all, if I were you, I wouldn't consider using this in the first place.
I'd say that this is not usable at all, I mean I use it myself just because I have to test it.

Momoi is an advanced Wayland wallpaper daemon with GPU-accelerated rendering, supporting images, animated GIFs (converted to WebM for efficiency), videos, procedural shaders, and post-processing effects. Built for performance and flexibility, it provides smooth transitions, multi-monitor support, and runtime control.

## ✨ Features

- **Media Support**: PNG, JPEG, WebP, SVG, GIF (auto-converted to WebM), MP4, WebM, MKV, and more
- **GPU Shaders**: 7 customizable procedural shaders (plasma, waves, starfield, matrix, etc.)
- **Post-Processing**: 7 overlay effects (vignette, scanlines, CRT, chromatic aberration, etc.)
- **Smooth Transitions**: 10 GPU-accelerated transition types (fade, wipes, center, outer)
- **Multi-Monitor**: Per-monitor wallpapers with independent control
- **Smart Features**: Playlist mode, time-based scheduling, resource management
- **Runtime Control**: Change wallpapers via CLI without daemon restart

## 🚀 Quick Start

### Requirements

- Wayland compositor with `wlr-layer-shell` support (Sway, Hyprland, River)
- GPU with Vulkan support (for shader features)
- NixOS or Nix package manager

### Building

```bash
# Enter development environment
nix develop

# Build with all features
cargo build --release --features gpu,video
```

### Usage

```bash
# Start the daemon
momoi

# Set a wallpaper
wwctl set /path/to/wallpaper.png

# Set with transition effect
wwctl set image.jpg --transition fade --duration 500

# Use a procedural shader
wwctl shader plasma --speed 2.0 --color1 FF0000

# Apply post-processing overlay
wwctl overlay vignette --strength 0.8

# Multi-monitor support
wwctl set landscape.jpg --output DP-1
wwctl set portrait.jpg --output DP-2

# Query status
wwctl query
```

## Installation

### NixOS Flakes (Easiest)

Add momoi to your flake and use the autoload module:

```nix
{
  inputs.momoi.url = "github:chikof/momoi";

  outputs = { nixpkgs, momoi, ... }@inputs: {
    nixosConfigurations.yourhost = nixpkgs.lib.nixosSystem {
      specialArgs = { inherit inputs; };
      modules = [
        (momoi.nixosModules.autoload { inherit inputs; })
        ./configuration.nix
      ];
    };
  };
}
```

Then enable in `configuration.nix`:

```nix
services.momoi.enable = true;
```

**See [INSTALLATION.md](./INSTALLATION.md) for more installation methods and Home Manager setup.**

### Configuration

#### NixOS (Recommended)

Configure Momoi declaratively using Nix language:

```nix
{
  services.momoi = {
    enable = true;

    settings = {
      general = {
        logLevel = "info";
        defaultTransition = "fade";
        defaultDuration = 500;
      };

      # Playlist mode with auto-rotation
      playlist = {
        enabled = true;
        interval = 300;  # 5 minutes
        shuffle = true;
        sources = [
          "~/Pictures/Wallpapers"
          "~/wallpapers/*.jpg"
        ];
      };

      # Time-based scheduling
      schedule = [
        {
          name = "Morning";
          startTime = "07:00";
          endTime = "12:00";
          wallpaper = "~/wallpapers/morning.jpg";
        }
        {
          name = "Evening";
          startTime = "18:00";
          endTime = "23:00";
          wallpaper = "~/wallpapers/evening.jpg";
        }
      ];

      # Per-monitor configuration
      outputs = [
        {
          name = "DP-1";
          wallpaper = "~/wallpapers/landscape.jpg";
          scale = "fill";
        }
        {
          name = "DP-2";
          wallpaper = "~/wallpapers/portrait.jpg";
          scale = "fit";
        }
      ];

      # Shader presets
      shaderPresets = [
        {
          name = "calm-ocean";
          shader = "waves";
          speed = 0.5;
          color1 = "1a1a2e";
          color2 = "16213e";
        }
        {
          name = "matrix-green";
          shader = "matrix";
          speed = 1.5;
          color1 = "00FF00";
          count = 200;
        }
      ];

      # Performance settings
      advanced = {
        performanceMode = "balanced";
        autoBatteryMode = true;
        maxFps = 60;
      };
    };
  };
}
```

**Flake setup:**

```nix
{
  inputs.momoi.url = "github:chikof/momoi";

  outputs = { self, nixpkgs, momoi, ... }: {
    nixosConfigurations.yourhost = nixpkgs.lib.nixosSystem {
      modules = [
        momoi.nixosModules.default
        ./configuration.nix
      ];
    };
  };
}
```

#### Manual Configuration (TOML)

Create a config file for advanced features:

```bash
mkdir -p ~/.config/momoi
cp config.toml.example ~/.config/momoi/config.toml
```

Example configuration:

```toml
[general]
log_level = "info"
default_transition = "fade"

[playlist]
enabled = true
interval = 300
shuffle = true
sources = ["~/Pictures/Wallpapers"]

[[shader_preset]]
name = "calm"
shader = "plasma"
speed = 0.5
color1 = "1a1a2e"
```

See [NIXOS_CONFIGURATION.md](./NIXOS_CONFIGURATION.md) for complete NixOS guide, or [CONFIGURATION.md](./CONFIGURATION.md) for TOML reference.

## 📖 Documentation

- [NIXOS_CONFIGURATION.md](./NIXOS_CONFIGURATION.md) - NixOS declarative configuration guide
- [CONFIGURATION.md](./CONFIGURATION.md) - TOML configuration reference
- [CONTRIBUTING.md](./CONTRIBUTING.md) - Contribution guidelines

## ⚙️ Contributing

Contributions to Momoi are welcome! If you find any bugs or want to suggest new features, feel free to open an issue or submit a pull request. Please ensure that your contributions align with the project's coding standards and follow the guidelines outlined in the [CONTRIBUTING.md](./CONTRIBUTING.md) file.

**Quick start:**

```bash
git clone https://github.com/chikof/momoi.git
cd momoi
nix develop
cargo test --all
```

## ⚖️ License

Momoi is open-source software licensed under the [MIT License](LICENSE). You are free to use, modify, and distribute the software as per the terms of the license.

---

<div align="center">
<sub>Inspired by <a href="https://codeberg.org/LGFae/awww">awww</a> and <a href="https://www.wallpaperengine.io/">Wallpaper Engine</a></sub>
</div>
