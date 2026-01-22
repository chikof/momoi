// GPU overlay shader - applies post-processing effects on top of any wallpaper
// This shader composites an overlay effect onto an existing rendered texture

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
};

struct Uniforms {
    time: f32,
    width: f32,
    height: f32,
    effect_type: f32,  // 0=vignette, 1=scanlines, 2=film_grain, 3=chromatic, 4=crt, 5=pixelate, 6=tint
    param1: f32,       // Effect-specific parameter 1
    param2: f32,       // Effect-specific parameter 2
    param3: f32,       // Effect-specific parameter 3
    param4: f32,       // Effect-specific parameter 4
    color_r: f32,      // For tint effect
    color_g: f32,
    color_b: f32,
    _padding: f32,
};

@group(0) @binding(0) var texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
@group(0) @binding(2) var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    // Fullscreen triangle
    let x = f32((vertex_index & 1u) << 2u);
    let y = f32((vertex_index & 2u) << 1u);
    
    out.position = vec4<f32>(x - 1.0, 1.0 - y, 0.0, 1.0);
    out.tex_coord = vec2<f32>(x * 0.5, y * 0.5);
    
    return out;
}

// Random function for noise
fn random(seed: vec2<f32>) -> f32 {
    return fract(sin(dot(seed, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

// Vignette effect
fn apply_vignette(color: vec3<f32>, coord: vec2<f32>, strength: f32) -> vec3<f32> {
    let center = vec2<f32>(0.5, 0.5);
    let dist = distance(coord, center);
    let vignette = 1.0 - smoothstep(0.3, 1.0, dist * strength);
    return color * vignette;
}

// Scanlines effect
fn apply_scanlines(color: vec3<f32>, coord: vec2<f32>, intensity: f32, line_width: f32) -> vec3<f32> {
    let scanline = sin(coord.y * uniforms.height / line_width) * 0.5 + 0.5;
    let darken = 1.0 - (scanline * intensity);
    return color * darken;
}

// Film grain effect
fn apply_film_grain(color: vec3<f32>, coord: vec2<f32>, intensity: f32) -> vec3<f32> {
    let noise = random(coord + uniforms.time * 0.1) - 0.5;
    return color + vec3<f32>(noise * intensity);
}

// Chromatic aberration effect
fn apply_chromatic(coord: vec2<f32>, offset: f32) -> vec3<f32> {
    let center = vec2<f32>(0.5, 0.5);
    let dir = normalize(coord - center);
    let dist = distance(coord, center);
    
    let offset_scaled = offset * 0.01 * dist;
    
    let r = textureSample(texture, texture_sampler, coord + dir * offset_scaled).r;
    let g = textureSample(texture, texture_sampler, coord).g;
    let b = textureSample(texture, texture_sampler, coord - dir * offset_scaled).b;
    
    return vec3<f32>(r, g, b);
}

// CRT effect with curvature
fn apply_crt(coord: vec2<f32>, curvature: f32, scanline_intensity: f32) -> vec4<f32> {
    // CRT curvature
    var curved_coord = coord * 2.0 - 1.0;
    let offset = curved_coord.xy * curved_coord.yx * curvature;
    curved_coord = curved_coord + curved_coord * offset;
    curved_coord = curved_coord * 0.5 + 0.5;
    
    // Out of bounds check
    if (curved_coord.x < 0.0 || curved_coord.x > 1.0 || 
        curved_coord.y < 0.0 || curved_coord.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    
    var color = textureSample(texture, texture_sampler, curved_coord).rgb;
    
    // Scanlines
    let scanline = sin(curved_coord.y * uniforms.height * 0.5) * 0.5 + 0.5;
    color = color * (1.0 - scanline * scanline_intensity);
    
    // Vignette
    let dist = distance(coord, vec2<f32>(0.5, 0.5));
    let vignette = 1.0 - smoothstep(0.5, 1.0, dist);
    color = color * vignette;
    
    return vec4<f32>(color, 1.0);
}

// Pixelate effect
fn apply_pixelate(coord: vec2<f32>, pixel_size: f32) -> vec3<f32> {
    let pixel_coord = floor(coord * vec2<f32>(uniforms.width, uniforms.height) / pixel_size) * pixel_size;
    let pixelated_coord = pixel_coord / vec2<f32>(uniforms.width, uniforms.height);
    return textureSample(texture, texture_sampler, pixelated_coord).rgb;
}

// Color tint effect
fn apply_tint(color: vec3<f32>, tint: vec3<f32>, strength: f32) -> vec3<f32> {
    return mix(color, color * tint, strength);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let coord = in.tex_coord;
    var color: vec3<f32>;
    
    // Select effect based on effect_type
    let effect = i32(uniforms.effect_type);
    
    if (effect == 0) {
        // Vignette
        color = textureSample(texture, texture_sampler, coord).rgb;
        color = apply_vignette(color, coord, uniforms.param1);
    } else if (effect == 1) {
        // Scanlines
        color = textureSample(texture, texture_sampler, coord).rgb;
        color = apply_scanlines(color, coord, uniforms.param1, uniforms.param2);
    } else if (effect == 2) {
        // Film grain
        color = textureSample(texture, texture_sampler, coord).rgb;
        color = apply_film_grain(color, coord, uniforms.param1);
    } else if (effect == 3) {
        // Chromatic aberration
        color = apply_chromatic(coord, uniforms.param1);
    } else if (effect == 4) {
        // CRT effect
        return apply_crt(coord, uniforms.param1, uniforms.param2);
    } else if (effect == 5) {
        // Pixelate
        color = apply_pixelate(coord, uniforms.param1);
    } else if (effect == 6) {
        // Color tint
        color = textureSample(texture, texture_sampler, coord).rgb;
        let tint = vec3<f32>(uniforms.color_r, uniforms.color_g, uniforms.color_b);
        color = apply_tint(color, tint, uniforms.param1);
    } else {
        // No effect, pass through
        color = textureSample(texture, texture_sampler, coord).rgb;
    }
    
    return vec4<f32>(color, 1.0);
}
