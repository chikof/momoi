// Raymarching shader - 3D sphere with lighting
// GPU-accelerated advanced effect using signed distance fields

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

// Signed distance function for a sphere
fn sdf_sphere(p: vec3<f32>, radius: f32) -> f32 {
    return length(p) - radius;
}

// Signed distance function for the scene
fn map_scene(p: vec3<f32>) -> f32 {
    let anim_time = uniforms.time * uniforms.speed;
    
    // Animated sphere position
    let sphere_pos = vec3<f32>(
        sin(anim_time * 0.5) * 2.0,
        cos(anim_time * 0.7) * 1.5,
        0.0
    );
    
    // Multiple spheres with scale parameter
    let sphere1 = sdf_sphere(p - sphere_pos, 1.5 * uniforms.scale);
    let sphere2 = sdf_sphere(p + sphere_pos * 0.5, 1.0 * uniforms.scale);
    let sphere3 = sdf_sphere(p - vec3<f32>(0.0, sin(anim_time) * 2.0, 2.0), 0.8 * uniforms.scale);
    
    // Ground plane
    let ground = p.y + 3.0;
    
    // Combine all objects (min = union)
    return min(min(min(sphere1, sphere2), sphere3), ground);
}

// Calculate normal using gradient
fn calculate_normal(p: vec3<f32>) -> vec3<f32> {
    let epsilon = 0.001;
    let dx = vec3<f32>(epsilon, 0.0, 0.0);
    let dy = vec3<f32>(0.0, epsilon, 0.0);
    let dz = vec3<f32>(0.0, 0.0, epsilon);
    
    let gradient = vec3<f32>(
        map_scene(p + dx) - map_scene(p - dx),
        map_scene(p + dy) - map_scene(p - dy),
        map_scene(p + dz) - map_scene(p - dz)
    );
    
    return normalize(gradient);
}

// Raymarching algorithm
fn raymarch(ray_origin: vec3<f32>, ray_dir: vec3<f32>) -> f32 {
    var depth = 0.0;
    
    for (var i = 0; i < 100; i++) {
        let pos = ray_origin + ray_dir * depth;
        let dist = map_scene(pos);
        
        // Hit surface
        if (dist < 0.001) {
            return depth;
        }
        
        depth += dist;
        
        // Too far, no hit
        if (depth > 100.0) {
            return -1.0;
        }
    }
    
    return -1.0;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Convert UV to screen coordinates (-1 to 1)
    let aspect = uniforms.width / uniforms.height;
    let uv = (in.uv * 2.0 - 1.0) * vec2<f32>(aspect, 1.0);
    
    let anim_time = uniforms.time * uniforms.speed;
    
    // Camera setup
    let cam_pos = vec3<f32>(
        cos(anim_time * 0.3) * 8.0,
        sin(anim_time * 0.2) * 2.0 + 2.0,
        sin(anim_time * 0.3) * 8.0
    );
    let cam_target = vec3<f32>(0.0, 0.0, 0.0);
    let cam_forward = normalize(cam_target - cam_pos);
    let cam_right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), cam_forward));
    let cam_up = cross(cam_forward, cam_right);
    
    // Ray direction
    let ray_dir = normalize(
        cam_forward +
        cam_right * uv.x +
        cam_up * uv.y
    );
    
    // Raymarch
    let depth = raymarch(cam_pos, ray_dir);
    
    // Background color (use color3)
    var color = vec3<f32>(uniforms.color3_r, uniforms.color3_g, uniforms.color3_b) * 0.15;
    
    if (depth > 0.0) {
        // Hit something!
        let hit_pos = cam_pos + ray_dir * depth;
        let normal = calculate_normal(hit_pos);
        
        // Lighting
        let light_pos = vec3<f32>(5.0, 10.0, 5.0);
        let light_dir = normalize(light_pos - hit_pos);
        let diffuse = max(dot(normal, light_dir), 0.0);
        
        // Specular
        let view_dir = normalize(cam_pos - hit_pos);
        let reflect_dir = reflect(-light_dir, normal);
        let specular = pow(max(dot(view_dir, reflect_dir), 0.0), 32.0);
        
        // Color based on custom colors
        let color1 = vec3<f32>(uniforms.color1_r, uniforms.color1_g, uniforms.color1_b);
        let color2 = vec3<f32>(uniforms.color2_r, uniforms.color2_g, uniforms.color2_b);
        
        let base_color = mix(color1, color2, sin(hit_pos.x * 2.0 + anim_time) * 0.5 + 0.5);
        
        // Ambient occlusion approximation
        let ao = 1.0 - (1.0 / (1.0 + depth * 0.1));
        
        // Final color with intensity
        color = base_color * (0.3 + diffuse * 0.7) * ao * uniforms.intensity + vec3<f32>(specular);
        
        // Fog
        let fog_amount = 1.0 - exp(-depth * 0.05);
        let fog_color = vec3<f32>(uniforms.color3_r, uniforms.color3_g, uniforms.color3_b) * 0.15;
        color = mix(color, fog_color, fog_amount);
    }
    
    return vec4<f32>(color, 1.0);
}
