// Gradient shader - rotating color gradient
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
    let nx = in.uv.x;
    let ny = in.uv.y;
    
    // Rotating gradient with configurable speed and scale
    let angle = uniforms.time * uniforms.speed * 0.5;
    let gradient = sin((nx * cos(angle) + ny * sin(angle)) * uniforms.scale + uniforms.time * uniforms.speed * 0.3) * 0.5 + 0.5;
    
    // Custom color gradient
    let color1 = vec3<f32>(uniforms.color1_r, uniforms.color1_g, uniforms.color1_b);
    let color2 = vec3<f32>(uniforms.color2_r, uniforms.color2_g, uniforms.color2_b);
    let color3 = vec3<f32>(uniforms.color3_r, uniforms.color3_g, uniforms.color3_b);
    
    // Mix between three colors based on gradient and position
    let t1 = gradient;
    let t2 = (nx + ny) * 0.5;
    
    var color = mix(color1, color2, t1);
    color = mix(color, color3, t2);
    color = color * uniforms.intensity;
    
    return vec4<f32>(color, 1.0);
}
