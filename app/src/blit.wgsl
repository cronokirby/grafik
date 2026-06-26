// Draws a fullscreen triangle and samples the computed image onto the surface.

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    // Oversized triangle covering the whole clip space.
    var verts = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );

    let p = verts[idx];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    // Map clip-space [-1, 1] to texture-space [0, 1], flipping Y.
    out.uv = vec2<f32>((p.x + 1.0) * 0.5, 1.0 - (p.y + 1.0) * 0.5);
    return out;
}

fn tone_map_reinhard(c: vec3<f32>) -> vec3<f32> {
    let linear = max(c, vec3<f32>(0.0));
    return linear / (linear + vec3<f32>(1.0));
}

// Encode linear color into sRGB for display. The image texture holds HDR
// linear values; the surface is a non-sRGB format, so we apply tone mapping
// and the sRGB curve ourselves.
fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, in.uv);
    return vec4<f32>(linear_to_srgb(tone_map_reinhard(c.rgb)), c.a);
}
