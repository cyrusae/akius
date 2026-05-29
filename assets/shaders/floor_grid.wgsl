#import bevy_pbr::forward_io::VertexOutput

struct FloorUniforms {
    grid_color: vec4<f32>,
    bg_color: vec4<f32>,
}

@group(2) @binding(0) var<uniform> uniforms: FloorUniforms;

@fragment
fn fragment(
    in: VertexOutput,
) -> @location(0) vec4<f32> {
    let uv = in.uv;
    
    // Create a grid in UV space
    let divisions_x = 20.0;
    let divisions_y = 30.0;
    
    // Fine grid lines
    let thick = 0.06;
    let grid_x = step(1.0 - thick, fract(uv.x * divisions_x));
    let grid_y = step(1.0 - thick, fract(uv.y * divisions_y));
    let is_line = max(grid_x, grid_y);
    
    // Mix background color with glowing line color
    var color = mix(uniforms.bg_color.rgb, uniforms.grid_color.rgb, is_line);
    
    // Render simple lighting calculation using normal and camera projection
    let normal = normalize(in.world_normal);
    let light_dir = normalize(vec3<f32>(0.3, 1.0, 0.4));
    let ndotl = max(dot(normal, light_dir), 0.0);
    
    // Apply shading to retain 3D volume
    color = color * (0.5 + 0.5 * ndotl);

    // Emphasize line glow
    if (is_line > 0.5) {
        color = color * 1.5;
    }
    
    return vec4<f32>(color, 1.0);
}
