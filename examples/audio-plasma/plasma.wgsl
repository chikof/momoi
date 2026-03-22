// Audio-reactive plasma wallpaper for momoi
// ------------------------------------------------
// Standard uniforms injected by shader-engine:
//   u.time         — seconds since daemon start
//   u.resolution_x — output width  (pixels)
//   u.resolution_y — output height (pixels)
//   u.audio_rms    — overall loudness   [0, 1]
//   u.bass         — low-frequency energy [0, 1]
//   u.treble       — high-frequency energy [0, 1]
//   u.hour         — wall-clock hour (0–23)

struct Uniforms {
    time:         f32,
    resolution_x: f32,
    resolution_y: f32,
    audio_rms:    f32,
    bass:         f32,
    treble:       f32,
    hour:         f32,
    _pad:         f32,
}

@group(0) @binding(0) var<uniform> u: Uniforms;

// User-defined uniforms (passed via wallpaper.toml).
// In a full implementation these would live in a second bind group.
// For this example they are hard-coded with sensible defaults.
const speed:      f32 = 1.0;
const complexity: f32 = 3.0;

// Convert HSV to RGB (all components in [0, 1]).
fn hsv2rgb(h: f32, s: f32, v: f32) -> vec3<f32> {
    let c = v * s;
    let x = c * (1.0 - abs(fract(h * 6.0) * 2.0 - 1.0 - 1.0));
    // Note: correct formula uses `abs(fmod(h*6, 2) - 1)`
    let m = v - c;
    var rgb: vec3<f32>;
    let sector = u32(h * 6.0) % 6u;
    switch sector {
        case 0u: { rgb = vec3<f32>(c, x, 0.0); }
        case 1u: { rgb = vec3<f32>(x, c, 0.0); }
        case 2u: { rgb = vec3<f32>(0.0, c, x); }
        case 3u: { rgb = vec3<f32>(0.0, x, c); }
        case 4u: { rgb = vec3<f32>(x, 0.0, c); }
        default: { rgb = vec3<f32>(c, 0.0, x); }
    }
    return rgb + vec3<f32>(m);
}

@fragment
fn fs_main(@builtin(position) frag_pos: vec4<f32>) -> @location(0) vec4<f32> {
    let res  = vec2<f32>(u.resolution_x, u.resolution_y);
    // Normalised device coords in [-1, 1], corrected for aspect ratio.
    let uv   = (frag_pos.xy / res * 2.0 - 1.0) * vec2<f32>(res.x / res.y, 1.0);

    let t    = u.time * speed * 0.5;

    // Bass boosts the spatial scale of the plasma.
    let scale = complexity + u.bass * 2.5;

    // Multiple sinusoidal distance fields layered to create plasma.
    let d1 = sin(uv.x * scale       + t * 1.3);
    let d2 = sin(uv.y * scale * 0.8 - t * 0.9);
    let d3 = sin((uv.x + uv.y) * scale * 0.6 + t * 1.1);
    let d4 = sin(length(uv - vec2<f32>(sin(t * 0.7), cos(t * 0.5))) * scale);

    let plasma = (d1 + d2 + d3 + d4) * 0.25; // [-1, 1]

    // Map plasma to hue; treble shifts the hue offset.
    let hue = fract(plasma * 0.5 + 0.5 + u.treble * 0.15 + t * 0.02);

    // Night mode: desaturate and darken between 20:00 and 06:00.
    let is_night = u.hour >= 20.0 || u.hour < 6.0;
    let saturation = select(0.85, 0.45, is_night);
    let value      = select(0.90, 0.55, is_night) + u.audio_rms * 0.1;

    let rgb = hsv2rgb(hue, saturation, value);
    return vec4<f32>(rgb, 1.0);
}
