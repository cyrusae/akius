#import bevy_pbr::forward_io::VertexOutput

struct ReticleUniforms {
    color: vec4<f32>,
    time: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> uniforms: ReticleUniforms;

@fragment
fn fragment(
    in: VertexOutput,
) -> @location(0) vec4<f32> {
    let uv = in.uv;
    
    // Coordinates relative to center (0.5, 0.5)
    let local_pos = uv - vec2<f32>(0.5, 0.5);
    let r = length(local_pos);
    let angle = atan2(local_pos.y, local_pos.x);
    
    // Discard outside the circular boundary
    if (r > 0.49) {
        discard;
    }
    
    // 1. Inner Ring: thin, dotted, slow reverse rotation
    let r1 = 0.26;
    let ring1_mask = smoothstep(0.010, 0.0, abs(r - r1));
    let dots = step(0.4, fract((angle - uniforms.time * 0.5) * 8.0 / 3.14159));
    let inner_ring = ring1_mask * dots;
    
    // 2. Middle Ring: thick, segmented, fast forward rotation
    let r2 = 0.36;
    let ring2_mask = smoothstep(0.015, 0.0, abs(r - r2));
    let segments = step(0.2, fract((angle + uniforms.time * 1.5) * 3.0 / 3.14159));
    let middle_ring = ring2_mask * segments;
    
    // 3. Outer Ring: extremely thin ticks, slow forward rotation
    let r3 = 0.45;
    let ring3_mask = smoothstep(0.006, 0.0, abs(r - r3));
    let ticks = step(0.82, fract((angle + uniforms.time * 0.7) * 16.0 / 3.14159));
    let outer_ticks = ring3_mask * ticks;
    
    // 4. Tech Crosshairs: thin indicators along X and Y axes
    let crosshair_x = smoothstep(0.005, 0.0, abs(local_pos.x)) * step(0.24, abs(local_pos.y)) * step(abs(local_pos.y), 0.46);
    let crosshair_y = smoothstep(0.005, 0.0, abs(local_pos.y)) * step(0.24, abs(local_pos.x)) * step(abs(local_pos.x), 0.46);
    let crosshair = max(crosshair_x, crosshair_y);
    
    // Combine all telemetry elements
    let glow = inner_ring * 0.8 + middle_ring * 1.0 + outer_ticks * 0.6 + crosshair * 0.95;
    
    // Apply subtle pulse to color over time
    let pulse = 1.0 + 0.15 * sin(uniforms.time * 8.0);
    let rgb = uniforms.color.rgb * pulse;
    
    // Blend with transparency (faint holographic look)
    let alpha = glow * 0.42 * uniforms.color.a;
    
    return vec4<f32>(rgb, alpha);
}
