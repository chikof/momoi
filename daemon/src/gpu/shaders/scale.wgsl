// GPU Image Scaling Shader
// Performs bilinear interpolation for smooth scaling

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

// Vertex shader - full-screen quad
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

// Fragment shader - bilinear scaling with GPU hardware interpolation
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // GPU's built-in bilinear filtering handles the interpolation
    // This is MUCH faster than manual CPU bilinear interpolation
    return textureSample(input_texture, texture_sampler, input.tex_coords);
}

// Alternative: Manual bilinear interpolation (if needed for custom behavior)
@fragment
fn fs_main_manual(input: VertexOutput) -> @location(0) vec4<f32> {
    let texture_size = vec2<f32>(textureDimensions(input_texture));
    let texel_size = 1.0 / texture_size;
    
    // Calculate texel coordinates
    let uv_pixels = input.tex_coords * texture_size;
    let uv_floor = floor(uv_pixels);
    let uv_fract = fract(uv_pixels);
    
    // Sample 4 nearest pixels
    let uv00 = (uv_floor + vec2<f32>(0.0, 0.0)) * texel_size;
    let uv10 = (uv_floor + vec2<f32>(1.0, 0.0)) * texel_size;
    let uv01 = (uv_floor + vec2<f32>(0.0, 1.0)) * texel_size;
    let uv11 = (uv_floor + vec2<f32>(1.0, 1.0)) * texel_size;
    
    let sample00 = textureSample(input_texture, texture_sampler, uv00);
    let sample10 = textureSample(input_texture, texture_sampler, uv10);
    let sample01 = textureSample(input_texture, texture_sampler, uv01);
    let sample11 = textureSample(input_texture, texture_sampler, uv11);
    
    // Bilinear interpolation
    let top = mix(sample00, sample10, uv_fract.x);
    let bottom = mix(sample01, sample11, uv_fract.x);
    let result = mix(top, bottom, uv_fract.y);
    
    return result;
}
