// Starfield shader - stars moving towards viewer
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

// Pseudo-random hash function
fn hash(p: f32) -> f32 {
    return fract(sin(p * 12.9898) * 43758.5453);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = vec3<f32>(0.0, 0.0, 0.0);
    
    let star_count = i32(uniforms.count);
    let star_color = vec3<f32>(uniforms.color1_r, uniforms.color1_g, uniforms.color1_b);
    
    // Check each star to see if it's at this pixel
    for (var i = 0; i < star_count; i++) {
        let seed = f32(i) * 12.9898;
        let px = hash(seed);
        let py = hash(seed + 1.0);
        
        // Animate star position (moving towards viewer)
        let z = ((uniforms.time * uniforms.speed * 0.5 + seed) % 2.0) - 1.0;
        let scale_factor = 1.0 / (z + 2.0) * uniforms.scale;
        
        let sx = (px - 0.5) * scale_factor + 0.5;
        let sy = (py - 0.5) * scale_factor + 0.5;
        
        // Calculate distance from this pixel to the star
        let dx = (in.uv.x - sx) * uniforms.width;
        let dy = (in.uv.y - sy) * uniforms.height;
        let dist = sqrt(dx * dx + dy * dy);
        
        // Draw star with soft falloff
        if (dist < 2.0) {
            let brightness = (1.0 - z) * (1.0 - dist / 2.0) * uniforms.intensity;
            color += star_color * brightness;
        }
    }
    
    return vec4<f32>(color, 1.0);
}
