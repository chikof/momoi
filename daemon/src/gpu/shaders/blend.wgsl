// Blend shader - transitions between two textures
// GPU-accelerated version using WGSL

struct Uniforms {
    progress: f32,        // 0.0 = old texture, 1.0 = new texture
    transition_type: u32, // 0 = fade, 1 = wipe_left, 2 = wipe_right, 3 = wipe_top, 4 = wipe_bottom
    width: f32,
    height: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var old_texture: texture_2d<f32>;

@group(0) @binding(2)
var new_texture: texture_2d<f32>;

@group(0) @binding(3)
var texture_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;
    
    // Full-screen triangle
    let x = f32((vertex_index << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(vertex_index & 2u) * 2.0 - 1.0;
    
    output.position = vec4<f32>(x, -y, 0.0, 1.0);
    output.uv = vec2<f32>((x + 1.0) * 0.5, (y + 1.0) * 0.5);
    
    return output;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let old_color = textureSample(old_texture, texture_sampler, in.uv);
    let new_color = textureSample(new_texture, texture_sampler, in.uv);
    
    var blend_factor = 0.0;
    
    // Determine blend factor based on transition type
    if (uniforms.transition_type == 0u) {
        // Fade: simple linear blend
        blend_factor = uniforms.progress;
    } else if (uniforms.transition_type == 1u) {
        // Wipe left: progress sweeps from left to right
        blend_factor = select(0.0, 1.0, in.uv.x < uniforms.progress);
    } else if (uniforms.transition_type == 2u) {
        // Wipe right: progress sweeps from right to left
        blend_factor = select(0.0, 1.0, in.uv.x > (1.0 - uniforms.progress));
    } else if (uniforms.transition_type == 3u) {
        // Wipe top: progress sweeps from top to bottom
        blend_factor = select(0.0, 1.0, in.uv.y < uniforms.progress);
    } else if (uniforms.transition_type == 4u) {
        // Wipe bottom: progress sweeps from bottom to top
        blend_factor = select(0.0, 1.0, in.uv.y > (1.0 - uniforms.progress));
    } else if (uniforms.transition_type == 5u) {
        // Center: expand from center outward
        let center = vec2<f32>(0.5, 0.5);
        let dist = length(in.uv - center);
        let max_dist = 0.707; // sqrt(0.5^2 + 0.5^2)
        blend_factor = select(0.0, 1.0, dist < uniforms.progress * max_dist);
    } else if (uniforms.transition_type == 6u) {
        // Outer: shrink from edges inward
        let center = vec2<f32>(0.5, 0.5);
        let dist = length(in.uv - center);
        let max_dist = 0.707;
        blend_factor = select(0.0, 1.0, dist > (1.0 - uniforms.progress) * max_dist);
    }
    
    // Blend the two textures
    return mix(old_color, new_color, blend_factor);
}
