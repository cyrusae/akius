#import bevy_pbr::forward_io::VertexOutput

struct FloorUniforms {
    grid_color: vec4<f32>,
    bg_color: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> uniforms: FloorUniforms;

@fragment
fn fragment(
    in: VertexOutput,
) -> @location(0) vec4<f32> {
    let uv = in.uv;
    
    // Create a grid in UV space
    let divisions_x = 20.0;
    let divisions_y = 30.0;
    
    let fx = fract(uv.x * divisions_x);
    let dist_x = min(fx, 1.0 - fx);
    let fy = fract(uv.y * divisions_y);
    let dist_y = min(fy, 1.0 - fy);
    let d = min(dist_x, dist_y);
    
    // Sharp grid line core (thin, sharp line)
    let sharp_line = smoothstep(0.012, 0.004, d);
    
    // Volumetric Bloom / Phosphor bleed (focused exponential falloff)
    let glow = exp(-d * 22.0) * 0.55;
    
    // Combined grid intensity
    let is_line = clamp(sharp_line + glow, 0.0, 1.0);
    
    // Bottom gradient glow (brightest at uv.y = 1.0, fading rapidly towards uv.y = 0.0)
    let bottom_gradient = pow(uv.y, 6.0) * 0.35;
    
    // Floor Fog / Ambient Center Radiation (very subtle)
    let center_dist = length(uv - vec2<f32>(0.5, 0.5));
    let center_fog = exp(-center_dist * center_dist * 4.0) * 0.04;
    
    // Vector-Style Background Noise (Subtle motherboard circuit lines)
    // Vertical buses
    let trace1 = smoothstep(0.004, 0.001, abs(uv.x - 0.35)) * step(0.1, uv.y) * step(uv.y, 0.9);
    let trace2 = smoothstep(0.004, 0.001, abs(uv.x - 0.65)) * step(0.15, uv.y) * step(uv.y, 0.85);
    // Horizontal branches
    let branch1 = smoothstep(0.004, 0.001, abs(uv.y - 0.4)) * step(0.35, uv.x) * step(uv.x, 0.55);
    let branch2 = smoothstep(0.004, 0.001, abs(uv.y - 0.6)) * step(0.45, uv.x) * step(uv.x, 0.65);
    
    let circuit = max(max(trace1, trace2), max(branch1, branch2)) * 0.02;
    let circuit_color = uniforms.grid_color.rgb * circuit;
    
    // Combine background components (dark base + circuits + fog + bottom gradient)
    let ambient_glow = uniforms.grid_color.rgb * (center_fog + bottom_gradient);
    let bg = uniforms.bg_color.rgb + circuit_color + ambient_glow;
    
    // Mix background with bright glowing lines (lines get dimmer further away from the player, but remain visible at 20% baseline)
    let line_fade = mix(0.20, 1.0, pow(uv.y, 1.2));
    var color = mix(bg, uniforms.grid_color.rgb * (2.0 * line_fade), is_line);
    
    // Render simple lighting calculation using normal and camera projection
    let normal = normalize(in.world_normal);
    let light_dir = normalize(vec3<f32>(0.3, 1.0, 0.4));
    let ndotl = max(dot(normal, light_dir), 0.0);
    
    // Apply shading to retain 3D volume
    color = color * (0.6 + 0.4 * ndotl);
    
    return vec4<f32>(color, 1.0);
}
