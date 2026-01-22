// Matrix rain shader - falling green vertical streams
// GPU-accelerated version using WGSL

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

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

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
    let x = in.uv.x * uniforms.width;
    let y = in.uv.y * uniforms.height;
    
    // Create vertical streams with configurable density (count parameter)
    let stream_width = max(1.0, 10.0 / uniforms.count); // More streams = narrower streams
    let column_seed = sin(floor(x / stream_width) * 0.1) * 1000.0;
    let fall_speed = (100.0 + column_seed) * uniforms.speed;
    let y_offset = (uniforms.time * fall_speed + column_seed) % (uniforms.height * 2.0);
    
    let dist = abs(y - y_offset);
    let trail_length = 20.0 * uniforms.scale;
    
    var brightness = 0.0;
    if (dist < trail_length) {
        brightness = (1.0 - dist / trail_length) * uniforms.intensity;
    }
    
    // Use custom color (default green for matrix effect)
    let color = vec3<f32>(uniforms.color1_r, uniforms.color1_g, uniforms.color1_b);
    
    return vec4<f32>(color * brightness, 1.0);
}
