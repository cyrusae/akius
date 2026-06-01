#import bevy_pbr::forward_io::VertexOutput

struct SideDeckUniforms {
    color: vec4<f32>,
    time: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> uniforms: SideDeckUniforms;

@fragment
fn fragment(
    in: VertexOutput,
) -> @location(0) vec4<f32> {
    let uv = in.uv;
    
    // Scale UV for diagnostic decks
    let local_uv = uv * vec2<f32>(15.0, 30.0);
    let cell = floor(local_uv);
    let f = fract(local_uv);
    
    // 1. Telemetry Grid: horizontal and vertical thin lines
    let grid_x = smoothstep(0.06, 0.0, abs(f.x - 0.5));
    let grid_y = smoothstep(0.06, 0.0, abs(f.y - 0.5));
    let grid = max(grid_x, grid_y) * 0.08;
    
    // 2. Scrolling Motherboard Circuit Buses (flowing green energy lines)
    // We draw horizontal buses flowing left/right at different Y coordinates
    let flow_speed1 = 1.2;
    let flow_speed2 = -0.8;
    
    // Horizontal bus line 1 (near top)
    let bus1_y = 0.25;
    let bus1_line = smoothstep(0.04, 0.005, abs(uv.y - bus1_y));
    let bus1_flow = step(0.3, fract(uv.x * 4.0 - uniforms.time * flow_speed1));
    let bus1 = bus1_line * bus1_flow * 0.25;
    
    // Horizontal bus line 2 (near bottom)
    let bus2_y = 0.75;
    let bus2_line = smoothstep(0.04, 0.005, abs(uv.y - bus2_y));
    let bus2_flow = step(0.4, fract(uv.x * 6.0 - uniforms.time * flow_speed2));
    let bus2 = bus2_line * bus2_flow * 0.20;
    
    // Vertical branches
    let branch_x1 = 0.35;
    let branch1 = smoothstep(0.04, 0.005, abs(uv.x - branch_x1)) * step(0.25, uv.y) * step(uv.y, 0.75) * 0.15;
    
    let branch_x2 = 0.65;
    let branch2 = smoothstep(0.04, 0.005, abs(uv.x - branch_x2)) * step(0.25, uv.y) * step(uv.y, 0.75) * 0.15;
    
    let circuits = max(max(bus1, bus2), max(branch1, branch2));
    
    // 3. Scrolling Diagnostic Bit Streams / Columns
    // We simulate vertical data streams (like memory dumps/scrolling bytes)
    // Column 1 at uv.x = 0.15
    let col1_x = 0.15;
    let col1_mask = smoothstep(0.05, 0.0, abs(uv.x - col1_x));
    let col1_data = step(0.5, sin(floor(uv.y * 24.0 - uniforms.time * 6.0) + col1_x * 99.0)) * 0.18;
    let col1 = col1_mask * col1_data;
    
    // Column 2 at uv.x = 0.85
    let col2_x = 0.85;
    let col2_mask = smoothstep(0.05, 0.0, abs(uv.x - col2_x));
    let col2_data = step(0.6, cos(floor(uv.y * 32.0 - uniforms.time * 8.0) + col2_x * 77.0)) * 0.15;
    let col2 = col2_mask * col2_data;
    
    // 4. Radar Sweeps (diagonal glowing telemetry pulses)
    let sweep = smoothstep(0.08, 0.0, abs(fract(uv.x + uv.y - uniforms.time * 0.15) - 0.5)) * 0.06;
    
    // Combine everything
    let raw_intensity = grid + circuits + col1 + col2 + sweep;
    
    // Mute at the outer edges (vignette/fade towards the margins)
    let edge_fade = smoothstep(0.0, 0.1, uv.y) * smoothstep(1.0, 0.9, uv.y) * smoothstep(0.0, 0.08, uv.x) * smoothstep(1.0, 0.92, uv.x);
    let intensity = raw_intensity * edge_fade;
    
    // Ambient green base
    let bg = vec3<f32>(0.0, 0.015, 0.0);
    
    // Glowing circuits
    let color = mix(bg, uniforms.color.rgb * 3.5, intensity);
    
    return vec4<f32>(color, 1.0);
}
