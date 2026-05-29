#import bevy_pbr::mesh_view_bindings::globals
#import bevy_pbr::forward_io::VertexOutput

struct SphereUniforms {
    color: vec4<f32>,
    base_color: vec4<f32>,
}

@group(2) @binding(0) var<uniform> uniforms: SphereUniforms;

@fragment
fn fragment(
    in: VertexOutput,
) -> @location(0) vec4<f32> {
    let uv = in.uv;
    
    // Draw fine grid lines
    let divisions = 10.0;
    let grid_x = step(0.93, fract(uv.x * divisions));
    let grid_y = step(0.93, fract(uv.y * divisions));
    let is_line = max(grid_x, grid_y);
    
    // Mix dim base color with bright line color
    var final_color = mix(uniforms.base_color.rgb, uniforms.color.rgb, is_line);
    
    // Render simple lighting calculation using normal and camera projection
    let normal = normalize(in.world_normal);
    let light_dir = normalize(vec3<f32>(0.3, 1.0, 0.4));
    let ndotl = max(dot(normal, light_dir), 0.0);
    
    // Apply shading to retain 3D volume
    final_color = final_color * (0.4 + 0.6 * ndotl);

    // Emphasize line glow (emissive lighting mock)
    if (is_line > 0.5) {
        final_color = final_color * 1.4;
    }
    
    return vec4<f32>(final_color, 1.0);
}
