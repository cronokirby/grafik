// Dummy compute shader: writes a simple gradient pattern into a storage texture.

@group(0) @binding(0) var output: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = textureDimensions(output);
    if id.x >= dims.x || id.y >= dims.y {
        return;
    }

    let uv = vec2<f32>(f32(id.x), f32(id.y)) / vec2<f32>(dims);

    // A simple, recognizable dummy pattern: a colorful gradient with rings.
    let center = uv - vec2<f32>(0.5, 0.5);
    let r = length(center);
    let rings = 0.5 + 0.5 * sin(r * 100.0);

    let color = vec3<f32>(uv.x, uv.y, rings);
    textureStore(output, vec2<i32>(id.xy), vec4<f32>(color, 1.0));
}
