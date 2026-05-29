#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct CrtSettings {
    time: f32,
    aspect_ratio: f32,
    glitch_intensity: f32,
    _padding: f32,
}

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
@group(0) @binding(2) var<uniform> settings: CrtSettings;

// Barrel distortion / Screen curvature
fn curve(uv: vec2<f32>) -> vec2<f32> {
    var u = uv - 0.5;
    // Curved screen profile parameters
    let bend = vec2<f32>(3.8, 3.8);
    u.x = u.x * (1.0 + (u.y * u.y) / bend.x);
    u.y = u.y * (1.0 + (u.x * u.x) / bend.y);
    return u + 0.5;
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    // 1. Curved coordinates
    var uv = curve(in.uv);

    // Apply horizontal screen tearing/glitch offset
    if (settings.glitch_intensity > 0.0) {
        let tear_offset = 0.015 * settings.glitch_intensity * sin(uv.y * 80.0 + settings.time * 40.0);
        // Only tear some horizontal stripes
        if (sin(uv.y * 15.0 + settings.time * 5.0) > 0.3) {
            uv.x = uv.x + tear_offset;
        }
    }

    // 2. Check if out of bounds (black screen borders)
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    // 3. Chromatic Aberration (RGB shift)
    let dir = uv - 0.5;
    let aberration_amount = 0.003 + settings.glitch_intensity * 0.025;
    let r_shift = vec2<f32>(aberration_amount, aberration_amount);
    let b_shift = vec2<f32>(-aberration_amount, -aberration_amount);
    
    let color_r = textureSample(screen_texture, texture_sampler, uv + dir * r_shift).r;
    let color_g = textureSample(screen_texture, texture_sampler, uv).g;
    let color_b = textureSample(screen_texture, texture_sampler, uv + dir * b_shift).b;
    
    var color = vec3<f32>(color_r, color_g, color_b);

    // 4. Scanlines (modulated by screen height)
    let scanline = sin(uv.y * 3.14159 * 2.0 * 240.0); // 240 phosphor scanlines
    let scanline_intensity = 0.12 + settings.glitch_intensity * 0.25;
    color = color * (1.0 - scanline_intensity * (0.5 * scanline + 0.5));

    // 5. Phosphor Glow & Screen Flicker
    let flicker = 0.012 * sin(settings.time * 50.0) * cos(settings.time * 23.0);
    color = color * (1.0 + flicker);

    // 6. Phosphor Tint
    // Apply a subtle green phosphor screen tint, slightly desaturated/corrupted under glitch
    let phosphor_tint = mix(vec3<f32>(0.92, 1.0, 0.88), vec3<f32>(0.7, 1.0, 0.6), settings.glitch_intensity);
    color = color * phosphor_tint;

    return vec4<f32>(color, 1.0);
}
