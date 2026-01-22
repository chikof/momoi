// GPU Plasma Shader
// Real-time animated plasma effect using sine waves

struct Uniforms {
    time: f32,
    width: f32,
    height: f32,
    speed: f32,
    color1_r: f32,
    color1_g: f32,
    color1_b: f32,
    scale: f32,
    color2_r: f32,
    color2_g: f32,
    color2_b: f32,
    intensity: f32,
    color3_r: f32,
    color3_g: f32,
    color3_b: f32,
    count: f32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

// Vertex shader - full-screen triangle
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;
    
    // Generate full-screen triangle
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);
    
    output.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    output.tex_coords = vec2<f32>(x, y);
    
    return output;
}

// Fragment shader - plasma effect
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = input.tex_coords;
    let time = uniforms.time * uniforms.speed;
    let freq = 10.0 * uniforms.scale;
    
    // Plasma effect using sine waves
    let v1 = sin(uv.x * freq + time);
    let v2 = sin(uv.y * freq - time);
    let v3 = sin((uv.x + uv.y) * freq + time);
    let v4 = sin(sqrt(uv.x * uv.x + uv.y * uv.y) * freq - time);
    
    let value = (v1 + v2 + v3 + v4) / 4.0;
    
    // Map to RGB colors using custom colors if provided
    let color1 = vec3<f32>(uniforms.color1_r, uniforms.color1_g, uniforms.color1_b);
    let color2 = vec3<f32>(uniforms.color2_r, uniforms.color2_g, uniforms.color2_b);
    let color3 = vec3<f32>(uniforms.color3_r, uniforms.color3_g, uniforms.color3_b);
    
    // Mix colors based on plasma value
    let t = value * 0.5 + 0.5; // Normalize to 0-1
    var color: vec3<f32>;
    if (t < 0.5) {
        color = mix(color1, color2, t * 2.0);
    } else {
        color = mix(color2, color3, (t - 0.5) * 2.0);
    }
    
    color = color * uniforms.intensity;
    
    return vec4<f32>(color, 1.0);
}
