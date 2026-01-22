# NixOS Configuration Guide

This guide shows how to configure Momoi declaratively using Nix language on NixOS or Home Manager.

> **💡 New to Momoi?** Check out [INSTALLATION.md](./INSTALLATION.md) for easy setup instructions!

## Table of Contents

- [Quick Start](#quick-start)
- [Basic Configuration](#basic-configuration)
- [Advanced Examples](#advanced-examples)
- [All Available Options](#all-available-options)

## Quick Start

The easiest way to use Momoi:

```nix
# In flake.nix
{
  inputs.momoi.url = "github:chikof/momoi";

  outputs = { nixpkgs, momoi, ... }@inputs: {
    nixosConfigurations.yourhost = nixpkgs.lib.nixosSystem {
      specialArgs = { inherit inputs; };
      modules = [
        (momoi.nixosModules.autoload { inherit inputs; })  # One line setup!
        ./configuration.nix
      ];
    };
  };
}
```

```nix
# In configuration.nix
{
  services.momoi.enable = true;
  services.momoi.settings.outputs = [
    {
      name = "HDMI-1";
      wallpaper = "~/wallpaper.jpg";
    }
  ];
}
```

For other installation methods, see [INSTALLATION.md](./INSTALLATION.md).

## Basic Configuration

### Simple Static Wallpaper

```nix
{
  services.momoi = {
    enable = true;

    settings = {
      general.defaultTransition = "fade";

      outputs = [
        {
          name = "DP-1";
          wallpaper = "/home/user/wallpaper.jpg";
          scale = "fill";
        }
      ];
    };
  };
}
```

### Playlist Mode

Automatically rotate through wallpapers:

```nix
{
  services.momoi = {
    enable = true;

    settings.playlist = {
      enabled = true;
      interval = 300;  # Change every 5 minutes
      shuffle = true;
      transition = "fade";
      transitionDuration = 1000;
      sources = [
        "~/Pictures/Wallpapers"
        "~/backgrounds/*.{jpg,png}"
      ];
      extensions = [ "jpg" "jpeg" "png" "webp" ];
    };
  };
}
```

### Time-Based Scheduling

Switch wallpapers based on time of day:

```nix
{
  services.momoi = {
    enable = true;

    settings.schedule = [
      {
        name = "Morning";
        startTime = "06:00";
        endTime = "12:00";
        wallpaper = "~/wallpapers/morning-bright.jpg";
        transition = "fade";
        duration = 2000;
      }
      {
        name = "Afternoon";
        startTime = "12:00";
        endTime = "18:00";
        wallpaper = "~/wallpapers/afternoon-warm.jpg";
        transition = "fade";
        duration = 2000;
      }
      {
        name = "Evening";
        startTime = "18:00";
        endTime = "22:00";
        wallpaper = "~/wallpapers/evening-sunset.jpg";
        transition = "fade";
        duration = 2000;
      }
      {
        name = "Night";
        startTime = "22:00";
        endTime = "06:00";
        wallpaper = "~/wallpapers/night-dark.jpg";
        transition = "fade";
        duration = 2000;
      }
    ];
  };
}
```

## Advanced Examples

### Multi-Monitor Setup

Different wallpapers per monitor:

```nix
{
  services.momoi = {
    enable = true;

    settings.outputs = [
      {
        name = "DP-1";  # Main monitor
        wallpaper = "~/wallpapers/landscape-4k.jpg";
        scale = "fill";
        transition = "fade";
        duration = 500;
      }
      {
        name = "DP-2";  # Vertical monitor
        wallpaper = "~/wallpapers/portrait-vertical.jpg";
        scale = "fit";
        transition = "wipe-left";
        duration = 800;
      }
      {
        name = "HDMI-A-1";  # Third monitor with playlist
        playlist = true;
        playlistSources = [ "~/wallpapers/monitor3" ];
        scale = "fill";
      }
    ];
  };
}
```

### Shader Presets

Create reusable shader configurations:

```nix
{
  services.momoi = {
    enable = true;

    settings.shaderPresets = [
      {
        name = "calm-ocean";
        shader = "waves";
        description = "Calm ocean waves";
        speed = 0.5;
        color1 = "1a4d6b";  # Deep blue
        color2 = "2b7a9e";  # Ocean blue
        intensity = 0.7;
      }
      {
        name = "matrix-classic";
        shader = "matrix";
        description = "Classic green Matrix effect";
        speed = 1.5;
        color1 = "00FF00";  # Matrix green
        count = 200;
      }
      {
        name = "starfield-fast";
        shader = "starfield";
        description = "Fast-moving starfield";
        speed = 3.0;
        color1 = "FFFFFF";
        count = 500;
      }
      {
        name = "plasma-pink";
        shader = "plasma";
        description = "Pink and purple plasma";
        speed = 1.0;
        color1 = "FF1493";  # Deep pink
        color2 = "9932CC";  # Purple
        color3 = "FF69B4";  # Hot pink
        scale = 1.5;
      }
      {
        name = "tunnel-vaporwave";
        shader = "tunnel";
        description = "Vaporwave aesthetic tunnel";
        speed = 0.8;
        color1 = "FF71CE";  # Pink
        color2 = "01CDFE";  # Cyan
        color3 = "B967FF";  # Purple
      }
    ];
  };
}
```

Then use presets via CLI:

```bash
wwctl shader plasma --preset calm-ocean
wwctl shader matrix --preset matrix-classic
```

### Complete Setup

Full-featured configuration with all options:

```nix
{
  services.momoi = {
    enable = true;

    settings = {
      # General settings
      general = {
        logLevel = "info";
        defaultTransition = "fade";
        defaultDuration = 500;
        defaultScale = "fill";
      };

      # Playlist configuration
      playlist = {
        enabled = true;
        interval = 600;  # 10 minutes
        shuffle = true;
        transition = "random";
        transitionDuration = 1000;
        sources = [
          "~/Pictures/Wallpapers"
          "~/backgrounds/nature/*.jpg"
          "~/backgrounds/space/*.png"
        ];
        extensions = [ "jpg" "jpeg" "png" "webp" "gif" ];
      };

      # Time-based schedules
      schedule = [
        {
          name = "Work Hours";
          startTime = "09:00";
          endTime = "17:00";
          wallpaper = "~/wallpapers/minimal-focus.jpg";
          transition = "fade";
          duration = 2000;
        }
        {
          name = "Relaxation";
          startTime = "17:00";
          endTime = "23:00";
          wallpaper = "~/wallpapers/evening-calm.jpg";
          transition = "center";
          duration = 1500;
        }
      ];

      # Per-output configuration
      outputs = [
        {
          name = "DP-1";
          wallpaper = "~/wallpapers/main-display.jpg";
          scale = "fill";
          transition = "fade";
          duration = 500;
        }
      ];

      # Wallpaper collections
      collections = [
        {
          name = "nature";
          description = "Nature and landscape photos";
          wallpapers = [
            "~/wallpapers/mountain.jpg"
            "~/wallpapers/ocean.jpg"
            "~/wallpapers/forest.jpg"
          ];
        }
        {
          name = "minimal";
          description = "Minimalist wallpapers";
          wallpapers = [
            "~/wallpapers/minimal-1.png"
            "~/wallpapers/minimal-2.png"
          ];
        }
      ];

      # Shader presets
      shaderPresets = [
        {
          name = "deep-space";
          shader = "starfield";
          description = "Deep space starfield";
          speed = 0.3;
          color1 = "FFFFFF";
          count = 1000;
          intensity = 0.8;
        }
        {
          name = "cyberpunk";
          shader = "matrix";
          description = "Cyberpunk matrix effect";
          speed = 2.0;
          color1 = "FF00FF";
          color2 = "00FFFF";
          count = 300;
        }
      ];

      # Performance and resource management
      advanced = {
        enableVideo = true;
        videoMuted = true;
        videoLoop = true;
        maxFps = 60;
        cacheLimitMb = 500;
        preloadNext = true;
        performanceMode = "balanced";
        autoBatteryMode = true;  # Switch to powersave on battery
        enforceMemoryLimits = true;
        maxMemoryMb = 300;
        cpuThreshold = 80.0;
      };
    };
  };
}
```

## All Available Options

### General Settings

| Option              | Type | Default  | Description                                                      |
| ------------------- | ---- | -------- | ---------------------------------------------------------------- |
| `logLevel`          | enum | `"info"` | Log level: `trace`, `debug`, `info`, `warn`, `error`             |
| `defaultTransition` | enum | `"fade"` | Default transition effect                                        |
| `defaultDuration`   | int  | `500`    | Default transition duration (ms)                                 |
| `defaultScale`      | enum | `"fill"` | Default scaling mode: `center`, `fill`, `fit`, `stretch`, `tile` |

### Playlist Settings

| Option               | Type   | Default  | Description                     |
| -------------------- | ------ | -------- | ------------------------------- |
| `enabled`            | bool   | `true`   | Enable playlist mode            |
| `interval`           | int    | `300`    | Rotation interval (seconds)     |
| `shuffle`            | bool   | `false`  | Shuffle playlist order          |
| `transition`         | string | `"fade"` | Transition effect               |
| `transitionDuration` | int    | `500`    | Transition duration (ms)        |
| `sources`            | list   | `[]`     | Wallpaper sources (paths/globs) |
| `extensions`         | list   | `[...]`  | File extensions to include      |

### Schedule Entry

| Option       | Type   | Description              |
| ------------ | ------ | ------------------------ |
| `name`       | string | Entry name               |
| `startTime`  | string | Start time (HH:MM)       |
| `endTime`    | string | End time (HH:MM)         |
| `wallpaper`  | string | Wallpaper path           |
| `transition` | string | Transition effect        |
| `duration`   | int    | Transition duration (ms) |

### Output Configuration

| Option            | Type    | Default  | Description                |
| ----------------- | ------- | -------- | -------------------------- |
| `name`            | string  | -        | Output name (e.g., "DP-1") |
| `wallpaper`       | string? | `null`   | Wallpaper path             |
| `scale`           | string  | `"fill"` | Scaling mode               |
| `transition`      | string  | `"fade"` | Transition effect          |
| `duration`        | int     | `500`    | Transition duration (ms)   |
| `playlist`        | bool    | `false`  | Enable playlist            |
| `playlistSources` | list    | `[]`     | Playlist sources           |

### Shader Preset

| Option        | Type    | Default | Description                                                                                |
| ------------- | ------- | ------- | ------------------------------------------------------------------------------------------ |
| `name`        | string  | -       | Preset name                                                                                |
| `shader`      | enum    | -       | Shader type: `plasma`, `waves`, `matrix`, `gradient`, `starfield`, `raymarching`, `tunnel` |
| `description` | string  | `""`    | Description                                                                                |
| `speed`       | float?  | `null`  | Animation speed multiplier                                                                 |
| `color1`      | string? | `null`  | Primary color (hex)                                                                        |
| `color2`      | string? | `null`  | Secondary color (hex)                                                                      |
| `color3`      | string? | `null`  | Tertiary color (hex)                                                                       |
| `scale`       | float?  | `null`  | Scale parameter                                                                            |
| `intensity`   | float?  | `null`  | Intensity (0.0-1.0)                                                                        |
| `count`       | int?    | `null`  | Count parameter                                                                            |

### Advanced Settings

| Option                | Type  | Default      | Description                                  |
| --------------------- | ----- | ------------ | -------------------------------------------- |
| `enableVideo`         | bool  | `true`       | Enable video support                         |
| `videoMuted`          | bool  | `true`       | Mute video audio                             |
| `videoLoop`           | bool  | `true`       | Loop videos                                  |
| `maxFps`              | int   | `60`         | Maximum FPS                                  |
| `cacheLimitMb`        | int   | `500`        | Cache size limit (MB)                        |
| `preloadNext`         | bool  | `true`       | Preload next wallpaper                       |
| `performanceMode`     | enum  | `"balanced"` | Mode: `performance`, `balanced`, `powersave` |
| `autoBatteryMode`     | bool  | `true`       | Auto switch to powersave on battery          |
| `enforceMemoryLimits` | bool  | `true`       | Enforce memory limits                        |
| `maxMemoryMb`         | int   | `300`        | Max memory usage (MB)                        |
| `cpuThreshold`        | float | `80.0`       | CPU threshold (%)                            |

## Tips

### Using Environment Variables

Reference home directory with `~`:

```nix
wallpaper = "~/Pictures/wallpaper.jpg";
```

Use `$HOME` explicitly:

```nix
wallpaper = "${config.home.homeDirectory}/Pictures/wallpaper.jpg";
```

### Combining Configurations

You can combine multiple configuration methods:

```nix
{
  services.momoi = {
    enable = true;
    settings = {
      # Base configuration
      general.defaultTransition = "fade";

      # Override with schedule during work hours
      schedule = [
        {
          name = "Work";
          startTime = "09:00";
          endTime = "17:00";
          wallpaper = "~/work-wallpaper.jpg";
        }
      ];

      # Use playlist outside work hours
      playlist = {
        enabled = true;
        sources = [ "~/personal-wallpapers" ];
      };
    };
  };
}
```

### Testing Configuration

After changing configuration:

**For NixOS system module:**

```bash
# Rebuild NixOS
sudo nixos-rebuild switch

# Check daemon status
systemctl --user status momoi

# View logs
journalctl --user -u momoi -f
```

**For Home Manager:**

```bash
# Rebuild Home Manager
home-manager switch

# Check daemon status
systemctl --user status momoi

# View logs
journalctl --user -u momoi -f
```

## Home Manager Support

Momoi can also be configured via Home Manager for per-user configuration.

### Setup

Add to your Home Manager flake:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    home-manager.url = "github:nix-community/home-manager";
    momoi.url = "github:chikof/momoi";
  };

  outputs = { nixpkgs, home-manager, momoi, ... }: {
    homeConfigurations."youruser" = home-manager.lib.homeManagerConfiguration {
      pkgs = nixpkgs.legacyPackages.x86_64-linux;
      extraSpecialArgs = {
        momoiFlake = momoi;  # Pass momoi flake to modules
      };
      modules = [
        momoi.homeManagerModules.default
        ./home.nix
      ];
    };
  };
}
```

### Configuration in home.nix

```nix
{
  services.momoi = {
    enable = true;

    settings = {
      general = {
        logLevel = "info";
        defaultTransition = "fade";
      };

      playlist = {
        enabled = true;
        interval = 300;
        sources = [ "~/Pictures/Wallpapers" ];
      };

      shaderPresets = [
        {
          name = "evening";
          shader = "plasma";
          speed = 0.5;
          color1 = "1a1a2e";
        }
      ];
    };
  };
}
```

### Differences from NixOS Module

**Home Manager module:**

- Config file placed in `~/.config/momoi/config.toml` (user-specific)
- Service runs per-user
- Package installed per-user
- Ideal for multi-user systems or non-NixOS with Nix + Home Manager

**NixOS module:**

- Config file in `/etc/momoi/config.toml` (system-wide)
- Symlinked to each user's home directory
- Package installed system-wide
- Ideal for single-user systems or when all users should have same config

## See Also

- [CONFIGURATION.md](./CONFIGURATION.md) - TOML configuration reference
- [README.md](./README.md) - Main documentation
- [CONTRIBUTING.md](./CONTRIBUTING.md) - Contributing guide
