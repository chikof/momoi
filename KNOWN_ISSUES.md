# Known Issues and Workarounds

## Linker Errors with Rust 1.92.0 (Fixed)

### Issue
When building without the video feature, you may encounter linker errors like:
```
rust-lld: error: undefined hidden symbol: anon.05f2b133e6b3ef9633f2f5cdb3bc86d5.11.llvm...
```

### Root Cause
This is a known bug in Rust 1.92.0 when using the LLD linker with certain optimization settings.

### Solution
The project now includes a `.cargo/config.toml` file that forces the use of the GNU BFD linker instead of LLD:

```toml
[target.x86_64-unknown-linux-gnu]
linker = "cc"
rustflags = ["-C", "link-arg=-fuse-ld=bfd"]
```

This workaround is automatically applied when you build the project.

### Verification
Both build modes should now work:
```bash
# Without video support
cargo build

# With video support
cargo build --features video
```

### Alternative Linkers
If you experience issues with the BFD linker, you can try:

1. **Mold linker** (fastest):
   ```bash
   # Install mold first, then edit .cargo/config.toml:
   rustflags = ["-C", "link-arg=-fuse-ld=mold"]
   ```

2. **Gold linker**:
   ```bash
   # Edit .cargo/config.toml:
   rustflags = ["-C", "link-arg=-fuse-ld=gold"]
   ```

3. **Wait for Rust 1.93+**: This bug should be fixed in future Rust releases.

## Other Known Issues

### Video Playback Performance
- **Issue**: High CPU usage with 4K videos
- **Workaround**: Use lower resolution videos (1080p or 1440p)
- **Future**: GPU acceleration planned in Phase 8

### GIF Loading Time
- **Issue**: Large animated GIFs take time to load (pre-scaling all frames)
- **Workaround**: Use videos instead for long/high-resolution animations
- **Expected**: Loading time proportional to frame count × resolution

### Compositor Compatibility
- **Supported**: Hyprland, Sway, River, and other wlroots-based compositors
- **Not Supported**: GNOME (lacks wlr-layer-shell protocol)
- **Not Tested**: KDE Plasma Wayland (should work but untested)

## Reporting New Issues

When reporting issues, please include:
1. Rust version: `rustc --version`
2. Build command used
3. Full error output
4. System info (OS, compositor, GPU)
5. Output of `RUST_LOG=debug` if relevant
