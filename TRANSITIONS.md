# Wallpaper Transitions

The Momoi supports smooth transitions when changing wallpapers. Transitions are rendered at 60 FPS for smooth, cinematic effects.

## Available Transition Types

### 1. **None** (Default for instant changes)
No transition - wallpaper changes instantly.

```bash
wwctl set image.jpg --transition none
```

### 2. **Fade**
Smooth alpha blend between old and new wallpaper.

```bash
wwctl set image.jpg --transition fade --duration 500
```

**Best for:** General purpose, works well with all image types

### 3. **Wipe Left**
New wallpaper wipes in from left to right.

```bash
wwctl set image.jpg --transition wipe-left --duration 800
```

**Best for:** Dynamic, directional transitions

### 4. **Wipe Right**
New wallpaper wipes in from right to left.

```bash
wwctl set image.jpg --transition wipe-right --duration 800
```

**Best for:** Matching UI flow direction

### 5. **Wipe Top**
New wallpaper wipes in from top to bottom.

```bash
wwctl set image.jpg --transition wipe-top --duration 800
```

**Best for:** Dramatic reveals

### 6. **Wipe Bottom**
New wallpaper wipes in from bottom to top.

```bash
wwctl set image.jpg --transition wipe-bottom --duration 800
```

**Best for:** Uplifting, energetic transitions

### 7. **Wipe Angle**
New wallpaper wipes in at a custom angle (diagonal wipe).

```bash
# 45 degree diagonal (bottom-left to top-right)
wwctl set image.jpg --transition wipe-angle --duration 1000 --angle 45

# 135 degree diagonal (top-left to bottom-right)
wwctl set image.jpg --transition wipe-angle --duration 1000 --angle 135

# Other angles: 0°=right, 90°=down, 180°=left, 270°=up
```

**Best for:** Dynamic, creative transitions
**Note:** Angle parameter defaults to 45° if not specified

### 8. **Center**
New wallpaper expands from the center outward (circular reveal).

```bash
wwctl set image.jpg --transition center --duration 1000
```

**Best for:** Dramatic, attention-grabbing transitions

### 9. **Outer**
New wallpaper shrinks from the edges inward (reverse of center).

```bash
wwctl set image.jpg --transition outer --duration 1000
```

**Best for:** Subtle, elegant transitions

### 10. **Random**
Randomly selects one of the available transition types.

```bash
wwctl set image.jpg --transition random --duration 800
```

**Best for:** Variety in wallpaper playlists/scripts
**Note:** Picks from: fade, wipe-left, wipe-right, wipe-top, wipe-bottom, wipe-angle(45°), center, outer

## CLI Usage

### Basic Syntax

```bash
wwctl set <image-path> [--transition <type>] [--duration <ms>] [--angle <degrees>]
```

### Parameters

- `--transition <type>` - Transition effect (default: fade)
  - Valid types: `none`, `fade`, `wipe-left`, `wipe-right`, `wipe-top`, `wipe-bottom`, `wipe-angle`, `center`, `outer`, `random`
  - Aliases: `left`, `right`, `top`, `bottom`, `angle`, `diagonal`
  
- `--duration <ms>` - Transition duration in milliseconds (default: 300)
  - Range: 0-5000ms recommended
  - 0 = instant (no transition)

- `--angle <degrees>` - Angle for wipe-angle transition (default: 45)
  - Range: 0-360 degrees
  - 0° = wipe from left (→)
  - 90° = wipe from top (↓)
  - 180° = wipe from right (←)
  - 270° = wipe from bottom (↑)
  - Only used with `wipe-angle` transition

### Examples

```bash
# Quick fade (300ms)
wwctl set ~/wallpapers/sunset.jpg --transition fade --duration 300

# Slow cinematic fade (2 seconds)
wwctl set ~/wallpapers/mountains.jpg --transition fade --duration 2000

# Fast wipe
wwctl set ~/wallpapers/forest.jpg --transition wipe-left --duration 400

# Diagonal wipe at 45 degrees
wwctl set ~/wallpapers/ocean.jpg --transition wipe-angle --duration 1000 --angle 45

# Diagonal wipe at 135 degrees
wwctl set ~/wallpapers/desert.jpg --transition wipe-angle --duration 1000 --angle 135

# Center expand (dramatic reveal)
wwctl set ~/wallpapers/city.jpg --transition center --duration 1200

# Outer shrink (elegant)
wwctl set ~/wallpapers/nature.jpg --transition outer --duration 1000

# Random transition (surprise me!)
wwctl set ~/wallpapers/abstract.jpg --transition random --duration 800

# Instant change (no transition)
wwctl set ~/wallpapers/space.jpg --transition none
```

## Easing Functions

All transitions use **ease-in-out** easing by default for smooth, professional-looking animations:
- **Slow start** - Animation begins gradually
- **Fast middle** - Speeds up in the middle
- **Slow end** - Decelerates smoothly to completion

This creates the most pleasing visual effect for most transitions.

### Technical Details

The daemon supports four easing functions internally:
- **Linear** - Constant speed (not currently exposed in CLI)
- **Ease In** - Slow start, fast end (not currently exposed)
- **Ease Out** - Fast start, slow end (not currently exposed)
- **Ease In-Out** - Slow start/end, fast middle *(default)*

## Performance Characteristics

### Fade Transition
- **CPU**: ~2-3ms per frame @ 1080p
- **Memory**: 2x framebuffer size during transition
- **Algorithm**: Per-pixel alpha blending
- **Best for**: Smooth, universal transitions

### Wipe Transitions (Left, Right, Top, Bottom)
- **CPU**: ~0.5-1ms per frame @ 1080p  
- **Memory**: 2x framebuffer size during transition
- **Algorithm**: Row/column copying
- **Best for**: Directional, dramatic effects

### Wipe Angle (Diagonal)
- **CPU**: ~2-4ms per frame @ 1080p
- **Memory**: 2x framebuffer size during transition
- **Algorithm**: Per-pixel distance calculation
- **Best for**: Dynamic, creative effects

### Center / Outer Transitions
- **CPU**: ~2-4ms per frame @ 1080p
- **Memory**: 2x framebuffer size during transition
- **Algorithm**: Radial distance calculation from center
- **Best for**: Dramatic, attention-grabbing effects

### Random Transition
- **Performance**: Depends on selected transition type
- **Overhead**: Negligible (random selection only at start)

### Frame Rate
All transitions render at **60 FPS** (16ms per frame) for smooth animations.

### Multi-Monitor
Transitions work independently on each monitor when using per-output wallpaper commands.

## Implementation Details

### How It Works

1. **Capture Current Frame**: When a transition is requested, the daemon captures the current wallpaper as the "old frame"
2. **Load New Image**: The new wallpaper is loaded and scaled to match output resolution
3. **Blend Frames**: Every 16ms (60 FPS), the transition engine blends the old and new frames based on progress
4. **Easing Applied**: Progress is smoothed using the easing function
5. **Commit Final Frame**: When transition completes, the new wallpaper is committed and old frame is discarded

### Color Format

All blending operations use **ARGB8888** format (32-bit color with alpha channel):
- 8 bits per channel (Red, Green, Blue, Alpha)
- Little-endian byte order (BGRA in memory)
- Allows smooth alpha blending for fade effects

### Event Loop Integration

During transitions, the daemon's event loop polls at 60 FPS to ensure smooth animation. When no transitions are active, polling adapts to the content type:
- **Static images**: Minimal polling (only process Wayland events)
- **GIFs**: Based on GIF frame timing
- **Videos**: Based on detected video FPS / 2

## Limitations & Known Issues

### Current Limitations

1. **GIF Transitions**: Currently disabled - GIF wallpapers switch instantly
2. **Video Transitions**: Currently disabled - video wallpapers switch instantly
3. **Advanced Transitions**: Not yet implemented:
   - Angle wipes (diagonal)
   - Center expand
   - Outer shrink
   - Random selection

### Performance Notes

- **High Resolution (4K+)**: May experience slight frame drops on slower CPUs
  - Fade: ~8-10ms @ 4K (still within 16ms budget)
  - Future GPU acceleration (Phase 8) will eliminate this
  
- **Multiple Monitors**: Each monitor's transition is processed in parallel using multiple CPU cores

## Testing

Run the included test script to verify all transitions:

```bash
cd /path/to/momoi
./test-transitions.sh
```

This will cycle through all transition types with various durations and images.

## Future Enhancements

Planned for future releases:
- **GIF/Video Transitions**: Blend between animated content
- **Advanced Transitions**:
  - `wipe-angle` - Diagonal wipes at custom angles
  - `center` - Expand from center outward
  - `outer` - Shrink from edges inward
  - `random` - Randomly select transition type
- **GPU Acceleration**: Move blending to GPU shaders for 4K/8K support
- **Custom Easing**: Expose easing function selection in CLI
- **Transition Profiles**: Save and reuse favorite transition configurations

## Troubleshooting

### Transition appears choppy
- Check CPU usage with `htop` - system may be under heavy load
- Try shorter duration (faster transitions are less affected by dropped frames)
- Ensure daemon is running with RUST_LOG=debug to see performance stats

### Wallpaper doesn't change after transition
- This is a bug - please check daemon logs and report
- Workaround: Use `--transition none` for instant change

### Transition stops mid-way
- Check daemon logs for errors
- May indicate insufficient memory - try closing other applications
- System may have entered power-saving mode

### Different transition speeds on different monitors
- This is expected - transitions are per-output
- Each monitor completes its transition independently
- To sync, use the same duration for all outputs

## See Also

- `README.md` - General usage and installation
- `OPTIMIZATIONS.md` - Performance tuning and benchmarks
- `ROADMAP.md` - Development roadmap and future features
