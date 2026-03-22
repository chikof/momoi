// audio_reactive.wgsl
//
// Example wallpaper shader for momoi.
//
// Uniforms injected every frame by the engine:
//   uniforms.time         – elapsed seconds (f32)
//   uniforms.delta_time   – frame delta in seconds (f32)
//   uniforms.resolution   – surface size in pixels (vec2<f32>)
//   uniforms.mouse        – normalised mouse pos 0..1 (vec2<f32>)
//   uniforms.audio_bands  – 32 frequency magnitudes packed as 8×vec4<f32>
//                           index helpers:
//                             bass  = audio_bands[0].x   (≈20–80 Hz)
//                             mid   = audio_bands[2].z   (≈500–2k Hz)
//                             treble= audio_bands[6].w   (≈8–16k Hz)

struct Uniforms {
    time:        f32,
    delta_time:  f32,
    resolution:  vec2<f32>,
    mouse:       vec2<f32>,
    _pad0:       vec2<f32>,
    audio_bands: array<vec4<f32>, 8>,
}

@group(0) @binding(0)
var<uniform> u: Uniforms;

// Emits a single full-screen triangle; no vertex buffer needed.

struct VertexOut {
    @builtin(position) pos: vec4<f32>,
    @location(0)       uv:  vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOut {
    // Triangle corners in clip space that overshoot the viewport.
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    let p = positions[vi];
    var out: VertexOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv  = p * 0.5 + 0.5;         // [0,1] UV with y-up
    return out;
}

fn hash21(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(127.1, 311.7));
    q += dot(q, q + 19.19);
    return fract(q.x * q.y);
}

/// 2-D value noise, returns 0..1.
fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);   // smoothstep
    return mix(
        mix(hash21(i),               hash21(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash21(i + vec2<f32>(0.0, 1.0)), hash21(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y,
    );
}

/// Fractional Brownian Motion – 4 octaves.
fn fbm(p_in: vec2<f32>) -> f32 {
    var p   = p_in;
    var val = 0.0;
    var amp = 0.5;
    for (var i = 0; i < 4; i++) {
        val += amp * noise2(p);
        p   *= 2.1;
        amp *= 0.5;
    }
    return val;
}

/// HSV → RGB (all channels 0..1).
fn hsv2rgb(h: f32, s: f32, v: f32) -> vec3<f32> {
    let c = v * s;
    let x = c * (1.0 - abs(fract(h * 6.0) * 2.0 - 1.0));
    let m = v - c;
    var rgb: vec3<f32>;
    let hi = i32(h * 6.0) % 6;
    if      hi == 0 { rgb = vec3<f32>(c, x, 0.0); }
    else if hi == 1 { rgb = vec3<f32>(x, c, 0.0); }
    else if hi == 2 { rgb = vec3<f32>(0.0, c, x); }
    else if hi == 3 { rgb = vec3<f32>(0.0, x, c); }
    else if hi == 4 { rgb = vec3<f32>(x, 0.0, c); }
    else            { rgb = vec3<f32>(c, 0.0, x); }
    return rgb + m;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // Normalised coordinates: (0,0) = centre, aspect-corrected.
    let aspect = u.resolution.x / u.resolution.y;
    let uv     = (in.uv - 0.5) * vec2<f32>(aspect, 1.0);

    let bass   = u.audio_bands[0].x;   // 20–80 Hz
    let low    = u.audio_bands[1].y;   // 80–250 Hz
    let mid    = u.audio_bands[2].z;   // 250 Hz–2 kHz
    let treble = u.audio_bands[6].w;   // 8–16 kHz

    // Slow-drifting fBm cloud base.
    let drift  = vec2<f32>(u.time * 0.04, u.time * 0.02);
    let cloud  = fbm(uv * 2.5 + drift);

    // Bass pulses push the cloud outward.
    let pulse_r = length(uv) - bass * 0.35;
    let nebula  = smoothstep(0.6, 0.0, abs(pulse_r - cloud * 0.4));

    // Hue slowly cycles; audio energy shifts saturation and value.
    let hue = fract(u.time * 0.03 + cloud * 0.4 + bass * 0.15);
    let sat = 0.65 + mid * 0.35;
    let val = 0.15 + nebula * (0.6 + treble * 0.4);
    var col = hsv2rgb(hue, sat, val);

    // 32 radial bars arranged in a circle.
    let BANDS = 32u;
    let r     = length(uv);
    let theta = atan2(uv.y, uv.x);   // -π..π

    // Map angle to a band index.
    let band_idx_f = (theta / (2.0 * 3.14159265) + 0.5) * f32(BANDS);
    let band_idx   = u32(band_idx_f) % BANDS;

    // Look up the magnitude for this angular slice.
    let vec_idx  = band_idx / 4u;
    let comp_idx = band_idx % 4u;
    var magnitude = 0.0;
    let bvec = u.audio_bands[vec_idx];
    if      comp_idx == 0u { magnitude = bvec.x; }
    else if comp_idx == 1u { magnitude = bvec.y; }
    else if comp_idx == 2u { magnitude = bvec.z; }
    else                   { magnitude = bvec.w; }

    // Ring geometry.
    let ring_inner = 0.28;
    let ring_outer = ring_inner + 0.08 + magnitude * 0.22;
    let on_ring    = step(ring_inner, r) * step(r, ring_outer);

    // Glow falloff outside the bar.
    let glow_dist  = abs(r - (ring_inner + ring_outer) * 0.5);
    let glow       = exp(-glow_dist * 18.0) * magnitude * 0.6;

    // Bar colour: bright white-hot core, coloured glow halo.
    let bar_hue = fract(hue + magnitude * 0.3 + f32(band_idx) / f32(BANDS) * 0.25);
    let bar_col = mix(
        hsv2rgb(bar_hue, 0.4, 1.0),
        vec3<f32>(1.0),
        on_ring * smoothstep(0.0, 0.1, magnitude),
    );

    col = mix(col, bar_col, clamp(on_ring + glow, 0.0, 1.0));

    let bloom_r   = 0.12 + bass * 0.08;
    let bloom_glow = exp(-max(r - bloom_r, 0.0) * 14.0) * bass;
    col += bloom_glow * hsv2rgb(fract(hue + 0.5), 0.6, 1.0);

    let vignette = 1.0 - smoothstep(0.55, 1.1, r / aspect);
    col *= vignette;

    let grain = (hash21(in.uv * u.resolution + u.time) - 0.5) * 0.025;
    col = clamp(col + grain, vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(col, 1.0);
}
