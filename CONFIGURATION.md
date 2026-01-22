# Configuration Guide

This guide covers all configuration options for the Momoi.

## Table of Contents

- [Quick Start](#quick-start)
- [Configuration File Location](#configuration-file-location)
- [Configuration Structure](#configuration-structure)
- [General Settings](#general-settings)
- [Playlist Configuration](#playlist-configuration)
- [Time-Based Scheduling](#time-based-scheduling)
- [Per-Output Configuration](#per-output-configuration)
- [Named Collections](#named-collections)
- [Advanced Settings](#advanced-settings)
- [Example Configurations](#example-configurations)
- [CLI Commands](#cli-commands)

## Quick Start

1. **Copy the example configuration:**
   ```bash
   mkdir -p ~/.config/momoi
   cp config.toml.example ~/.config/momoi/config.toml
   ```

2. **Edit the configuration:**
   ```bash
   $EDITOR ~/.config/momoi/config.toml
   ```

3. **Start the daemon:**
   ```bash
   momoi
   ```

The daemon will automatically load and apply your configuration on startup.

## Configuration File Location

The daemon looks for configuration in the following locations (in order):

1. `~/.config/momoi/config.toml` (default)
2. Path specified with `--config` flag (if provided)

If no configuration file is found, the daemon uses sensible defaults and operates in manual mode (no automatic rotation or scheduling).

## Configuration Structure

The configuration file uses TOML format and is organized into sections:

```toml
[general]       # General daemon settings
[playlist]      # Automatic wallpaper rotation
[[schedule]]    # Time-based wallpaper switching (can have multiple)
[[output]]      # Per-monitor configuration (can have multiple)
[[collection]]  # Named wallpaper collections (can have multiple)
[advanced]      # Advanced performance settings
```

## General Settings

Controls the overall behavior of the daemon.

```toml
[general]
# Log level: trace, debug, info, warn, error
log_level = "info"

# Default transition for wallpaper changes
# Options: none, fade, wipe-left, wipe-right, wipe-top, wipe-bottom,
#          wipe-angle, center, outer, random
default_transition = "fade"

# Default transition duration in milliseconds
default_duration = 500

# Default scaling mode for images
# Options: center, fill, fit, stretch, tile
default_scale = "fill"
```

### Options Explained

- **`log_level`**: Controls verbosity of logging output
  - `trace`: Most verbose, includes all debug information
  - `debug`: Detailed debugging information
  - `info`: Normal informational messages (recommended)
  - `warn`: Only warnings and errors
  - `error`: Only error messages

- **`default_transition`**: Default transition effect for wallpaper changes
  - See [TRANSITIONS.md](TRANSITIONS.md) for detailed descriptions

- **`default_duration`**: How long transitions take (in milliseconds)
  - Shorter = faster transitions
  - Longer = smoother, more gradual transitions
  - Recommended: 300-1000ms

- **`default_scale`**: How images are scaled to fit the screen
  - `center`: Display at original size, centered
  - `fill`: Fill screen, may crop (maintains aspect ratio)
  - `fit`: Fit entire image on screen (may have letterboxing)
  - `stretch`: Stretch to fill screen (may distort)
  - `tile`: Tile image to fill screen

## Playlist Configuration

Enables automatic wallpaper rotation from a list of sources.

```toml
[playlist]
# Enable automatic wallpaper rotation
enabled = true

# Rotation interval in seconds (how long each wallpaper is shown)
interval = 300  # 5 minutes

# Shuffle wallpapers randomly
shuffle = true

# Transition to use when rotating wallpapers
transition = "random"
transition_duration = 1000

# Wallpaper sources (can be files, directories, or glob patterns)
sources = [
    "/home/user/Wallpapers/favorites",
    "/home/user/Pictures/wallpapers/*.jpg",
    "/home/user/Pictures/wallpapers/*.png",
    "~/Downloads/backgrounds",  # Tilde expansion supported
]

# File extensions to include when scanning directories
extensions = ["jpg", "jpeg", "png", "webp", "gif", "mp4", "webm", "mkv"]
```

### Options Explained

- **`enabled`**: Whether to enable playlist rotation
  - `true`: Automatically rotate wallpapers
  - `false`: Manual control only

- **`interval`**: How long to display each wallpaper (in seconds)
  - Examples: 60 = 1 minute, 300 = 5 minutes, 3600 = 1 hour

- **`shuffle`**: Randomize wallpaper order
  - `true`: Random order, regenerates when all wallpapers have been shown
  - `false`: Sequential order based on filename

- **`transition`**: Transition effect for rotations
  - Use `"random"` for variety
  - Or specify a specific transition type

- **`sources`**: List of wallpaper locations
  - Can be individual files: `"/path/to/image.jpg"`
  - Can be directories: `"/path/to/wallpapers"`
  - Can be glob patterns: `"/path/to/wallpapers/*.{jpg,png}"`
  - Supports tilde (`~`) expansion for home directory

- **`extensions`**: File types to include when scanning directories
  - Add or remove extensions as needed
  - Case-insensitive matching

### Playlist CLI Commands

Control the playlist manually:

```bash
# Move to next wallpaper
wwctl playlist next

# Move to previous wallpaper
wwctl playlist prev

# Toggle shuffle mode on/off
wwctl playlist shuffle
```

## Time-Based Scheduling

Automatically switch wallpapers based on the time of day.

```toml
# Morning wallpapers (6:00 AM - 12:00 PM)
[[schedule]]
name = "Morning"
start_time = "06:00"
end_time = "12:00"
wallpaper = "/home/user/Wallpapers/morning/sunrise.jpg"
transition = "fade"
duration = 2000

# Afternoon wallpapers (12:00 PM - 6:00 PM)
[[schedule]]
name = "Afternoon"
start_time = "12:00"
end_time = "18:00"
wallpaper = "/home/user/Wallpapers/afternoon/sunny.jpg"
transition = "center"
duration = 1500

# Evening wallpapers (6:00 PM - 10:00 PM)
[[schedule]]
name = "Evening"
start_time = "18:00"
end_time = "22:00"
wallpaper = "/home/user/Wallpapers/evening/sunset.jpg"
transition = "wipe-left"
duration = 1000

# Night wallpapers (10:00 PM - 6:00 AM)
[[schedule]]
name = "Night"
start_time = "22:00"
end_time = "06:00"
wallpaper = "/home/user/Wallpapers/night/stars.jpg"
transition = "fade"
duration = 3000
```

### Options Explained

- **`name`**: Descriptive name for the schedule entry
  - Used in log messages
  - No functional impact

- **`start_time`** / **`end_time`**: Time range in 24-hour format (HH:MM)
  - Format: "HH:MM" (e.g., "06:00", "18:30")
  - Supports ranges that cross midnight (e.g., "22:00" to "06:00")

- **`wallpaper`**: Path to wallpaper file for this time period
  - Supports tilde (`~`) expansion

- **`transition`** / **`duration`**: Transition effect and duration for this schedule
  - Overrides default transition settings

### How Scheduling Works

1. The daemon checks the schedule every minute
2. When the current time falls within a schedule's time range, that wallpaper is activated
3. When no schedule matches, the previous wallpaper remains active
4. Scheduling works alongside playlists - schedule takes priority when active

### Time Range Examples

```toml
# Morning: 6 AM to 12 PM
start_time = "06:00"
end_time = "12:00"

# Night (crosses midnight): 10 PM to 6 AM
start_time = "22:00"
end_time = "06:00"

# All day except lunch: 12 AM to 11 AM, 1 PM to 12 AM
# Use two separate schedule entries
```

## Per-Output Configuration

Configure different wallpapers for each monitor.

```toml
# Primary monitor (landscape)
[[output]]
name = "DP-2"  # Output name (use 'wwctl list-outputs' to find yours)
wallpaper = "/home/user/Wallpapers/landscape.jpg"
scale = "fill"
transition = "fade"
duration = 500

# Enable playlist for this output
playlist = true
playlist_sources = [
    "/home/user/Wallpapers/primary/*.jpg",
]

# Secondary monitor (portrait)
[[output]]
name = "DP-3"
wallpaper = "/home/user/Wallpapers/portrait.jpg"
scale = "fit"
transition = "wipe-top"
duration = 800

# Use different playlist for secondary monitor
playlist = true
playlist_sources = [
    "/home/user/Wallpapers/secondary/*.jpg",
]
```

### Options Explained

- **`name`**: Output (monitor) identifier
  - Find your output names with: `wwctl list-outputs`
  - Common names: `DP-1`, `DP-2`, `HDMI-A-1`, `eDP-1`

- **`wallpaper`**: Initial wallpaper for this output
  - Optional if using playlist

- **`scale`**: Scaling mode for this output
  - Useful for different aspect ratios (portrait vs landscape)

- **`transition`** / **`duration`**: Default transition for this output

- **`playlist`**: Enable playlist for this specific output
  - `true`: Use playlist from `playlist_sources`
  - `false`: Use global playlist or manual control

- **`playlist_sources`**: Sources for this output's playlist
  - Works the same as global `[playlist]` sources
  - Each output can have its own rotation interval and sources

### Finding Output Names

```bash
# List all connected outputs
wwctl list-outputs

# Example output:
# Output: DP-2
#   Resolution: 2560x1440
#   Scale: 1.0
#
# Output: DP-3
#   Resolution: 1080x1920
#   Scale: 1.0
```

## Named Collections

Create named sets of wallpapers that can be activated together.

```toml
[[collection]]
name = "Nature"
description = "Beautiful nature wallpapers"
wallpapers = [
    "/home/user/Wallpapers/nature/forest.jpg",
    "/home/user/Wallpapers/nature/mountain.jpg",
    "/home/user/Wallpapers/nature/ocean.jpg",
]

[[collection]]
name = "Space"
description = "Space and astronomy"
wallpapers = [
    "/home/user/Wallpapers/space/galaxy.jpg",
    "/home/user/Wallpapers/space/nebula.jpg",
    "/home/user/Wallpapers/space/planet.jpg",
]

[[collection]]
name = "Minimal"
description = "Minimalist designs"
wallpapers = [
    "/home/user/Wallpapers/minimal/gradient.png",
    "/home/user/Wallpapers/minimal/geometric.png",
]
```

### Options Explained

- **`name`**: Unique identifier for the collection
  - Used in commands to activate the collection

- **`description`**: Human-readable description
  - Optional, for documentation purposes

- **`wallpapers`**: List of wallpaper paths in the collection
  - Order is preserved when cycling through the collection

### Using Collections

Collections provide a way to organize and quickly switch between themed sets of wallpapers.

**Note:** Collection activation commands will be added in a future update. Currently, collections serve as documentation and organization within your config file.

## Advanced Settings

Performance and behavior tuning options.

```toml
[advanced]
# Enable video wallpapers (requires video feature)
enable_video = true

# Video playback settings
video_muted = true
video_loop = true

# Frame rate cap (0 = unlimited)
max_fps = 60

# Memory limit for cache in MB (0 = unlimited)
cache_limit_mb = 500

# Preload next wallpaper in playlist
preload_next = true
```

### Options Explained

- **`enable_video`**: Enable video wallpaper support
  - Requires daemon built with `video` feature
  - `true`: MP4, WebM, MKV, etc. are supported
  - `false`: Only images and GIFs

- **`video_muted`**: Mute audio in video wallpapers
  - `true`: No audio playback (recommended for wallpapers)
  - `false`: Play audio from videos

- **`video_loop`**: Loop video wallpapers
  - `true`: Videos restart when they finish
  - `false`: Videos stop at the end

- **`max_fps`**: Maximum frame rate for animations
  - `60`: Standard (recommended)
  - `30`: Lower CPU usage
  - `0`: Unlimited (not recommended, wastes CPU)
  - Affects GIF and video playback

- **`cache_limit_mb`**: Memory limit for cached wallpapers
  - `0`: No limit (may use lots of RAM)
  - `500`: Reasonable limit for most systems
  - Adjust based on available RAM

- **`preload_next`**: Preload next playlist wallpaper
  - `true`: Smoother transitions, uses more RAM
  - `false`: Lower memory usage, slight delay when switching

## Example Configurations

### Minimal Configuration

```toml
# config.toml - Bare minimum
[general]
log_level = "info"

# No playlist or schedule - manual control only
```

### Simple Playlist

```toml
# config.toml - Basic rotation every 5 minutes
[general]
log_level = "info"
default_transition = "fade"

[playlist]
enabled = true
interval = 300
shuffle = true
sources = ["~/Pictures/Wallpapers"]
extensions = ["jpg", "png"]
```

### Time-Based Wallpapers

```toml
# config.toml - Different wallpapers for day/night
[general]
log_level = "info"

[[schedule]]
name = "Day"
start_time = "07:00"
end_time = "19:00"
wallpaper = "~/Wallpapers/day.jpg"
transition = "fade"
duration = 2000

[[schedule]]
name = "Night"
start_time = "19:00"
end_time = "07:00"
wallpaper = "~/Wallpapers/night.jpg"
transition = "fade"
duration = 2000
```

### Multi-Monitor Setup

```toml
# config.toml - Different wallpapers per monitor
[general]
log_level = "info"

[[output]]
name = "DP-1"  # Primary landscape monitor
scale = "fill"
playlist = true
playlist_sources = ["~/Wallpapers/landscape"]

[[output]]
name = "DP-2"  # Secondary portrait monitor
scale = "fit"
playlist = true
playlist_sources = ["~/Wallpapers/portrait"]
```

### Full-Featured Configuration

```toml
# config.toml - All features enabled
[general]
log_level = "info"
default_transition = "random"
default_duration = 800
default_scale = "fill"

[playlist]
enabled = true
interval = 600  # 10 minutes
shuffle = true
transition = "random"
transition_duration = 1200
sources = [
    "~/Wallpapers/favorites",
    "~/Pictures/*.{jpg,png}",
]
extensions = ["jpg", "jpeg", "png", "webp", "gif"]

[[schedule]]
name = "Morning"
start_time = "06:00"
end_time = "12:00"
wallpaper = "~/Wallpapers/morning.jpg"
transition = "center"
duration = 2000

[[schedule]]
name = "Evening"
start_time = "18:00"
end_time = "22:00"
wallpaper = "~/Wallpapers/evening.jpg"
transition = "fade"
duration = 3000

[[output]]
name = "DP-1"
scale = "fill"
transition = "fade"
playlist = true
playlist_sources = ["~/Wallpapers/monitor1"]

[[output]]
name = "DP-2"
scale = "fit"
transition = "wipe-left"
playlist = true
playlist_sources = ["~/Wallpapers/monitor2"]

[advanced]
enable_video = true
video_muted = true
max_fps = 60
cache_limit_mb = 500
preload_next = true
```

## CLI Commands

Control the daemon from the command line.

### Basic Commands

```bash
# Set wallpaper manually
wwctl set /path/to/image.jpg

# Set with transition
wwctl set /path/to/image.jpg --transition fade --duration 1000

# Set for specific output
wwctl set /path/to/image.jpg --output DP-1

# Set solid color
wwctl color "#1e1e1e"

# Query daemon status
wwctl query

# List connected outputs
wwctl list-outputs

# Ping daemon
wwctl ping

# Kill daemon
wwctl kill
```

### Playlist Commands

```bash
# Move to next wallpaper in playlist
wwctl playlist next

# Move to previous wallpaper in playlist
wwctl playlist prev

# Toggle shuffle mode
wwctl playlist shuffle
```

### Advanced Usage

```bash
# Set with custom scale mode
wwctl set image.jpg --scale fit

# Set with angle transition
wwctl set image.jpg --transition wipe-angle --angle 45

# Set for all outputs
wwctl set image.jpg --output all
```

## Troubleshooting

### Configuration Not Loading

1. Check file location:
   ```bash
   ls -la ~/.config/momoi/config.toml
   ```

2. Check syntax errors:
   ```bash
   # The daemon will log syntax errors on startup
   momoi
   ```

3. Check permissions:
   ```bash
   chmod 644 ~/.config/momoi/config.toml
   ```

### Playlist Not Rotating

1. Verify `enabled = true` in `[playlist]` section
2. Check that `sources` point to valid directories
3. Verify files have correct extensions
4. Check daemon logs for errors

### Schedule Not Activating

1. Verify time format is "HH:MM" in 24-hour format
2. Check that wallpaper paths exist
3. Schedule checks every minute - wait up to 60 seconds
4. Check daemon logs for activation messages

### Per-Output Configuration Not Working

1. Verify output names with `wwctl list-outputs`
2. Names are case-sensitive and must match exactly
3. Outputs must be connected when daemon starts

## See Also

- [README.md](README.md) - Main documentation
- [TRANSITIONS.md](TRANSITIONS.md) - Transition effects guide
- [ROADMAP.md](ROADMAP.md) - Development roadmap
- [config.toml.example](config.toml.example) - Full example configuration
