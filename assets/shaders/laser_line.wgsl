#import bevy_pbr::forward_io::VertexOutput

struct LaserUniforms {
    color: vec4<f32>,
    time: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> uniforms: LaserUniforms;

@fragment
fn fragment(
    in: VertexOutput,
) -> @location(0) vec4<f32> {
    let uv = in.uv;
    
    // Normalized distance from center (0.0 at center, 1.0 at edges)
    let dist = abs(uv.x - 0.5) * 2.0;
    
    // Softer falloff (broader, no sharp core)
    let falloff = exp(-dist * dist * 1.8);
    
    // Pure source color multiplied by falloff and a brightness factor
    var rgb = uniforms.color.rgb * falloff * 0.8;
    
    // Directional chevron pulses moving forward
    let wave = sin(uv.y * 40.0 - uniforms.time * 15.0);
    let pulse = max(wave, 0.0) * 0.25;
    
    // Apply pulse glow boost
    rgb = rgb * (1.0 + pulse);
    
    // Fade out alpha near the edges of the beam (make it translucent)
    let alpha = falloff * 0.20 * uniforms.color.a;
    
    return vec4<f32>(rgb, alpha);
}
