// Tunnel shader - infinite tunnel effect
// GPU-accelerated psychedelic vortex

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
    // Center coordinates
    let aspect = uniforms.width / uniforms.height;
    let uv = (in.uv - 0.5) * vec2<f32>(aspect, 1.0);
    
    // Polar coordinates
    let angle = atan2(uv.y, uv.x);
    let radius = length(uv);
    
    // Tunnel depth
    let depth = 1.0 / (radius + 0.1);
    
    // Animate tunnel with speed parameter
    let anim_time = uniforms.time * uniforms.speed;
    let z = depth * uniforms.scale + anim_time * 2.0;
    
    // Create repeating tunnel pattern
    let tunnel_u = angle / (3.14159265 * 2.0) + anim_time * 0.5;
    let tunnel_v = z;
    
    // Checker pattern with configurable size (count parameter)
    let checker_size = max(2.0, uniforms.count * 0.1);
    let checker_u = floor(tunnel_u * checker_size);
    let checker_v = floor(tunnel_v * checker_size);
    let checker = (checker_u + checker_v) % 2.0;
    
    // Custom colors
    let base_color1 = vec3<f32>(uniforms.color1_r, uniforms.color1_g, uniforms.color1_b);
    let base_color2 = vec3<f32>(uniforms.color2_r, uniforms.color2_g, uniforms.color2_b);
    let accent_color = vec3<f32>(uniforms.color3_r, uniforms.color3_g, uniforms.color3_b);
    
    // Color based on depth
    let color1 = base_color1 * (0.5 + 0.5 * sin(z * 2.0));
    let color2 = base_color2 * (0.5 + 0.5 * cos(z * 1.5));
    
    // Mix colors based on checker pattern
    var color = mix(color1, color2, checker);
    
    // Add glow at center
    let glow = 1.0 / (radius * 5.0 + 1.0);
    color += accent_color * glow * 0.5;
    
    // Add rotating rays (count parameter controls ray count)
    let ray_count = max(4.0, uniforms.count * 0.08);
    let rays = sin(angle * ray_count + anim_time * 3.0) * 0.5 + 0.5;
    color += accent_color * rays * 0.2 * (1.0 - radius);
    
    // Vignette
    let vignette = 1.0 - pow(radius, 2.0);
    color *= vignette * uniforms.intensity;
    
    return vec4<f32>(color, 1.0);
}
