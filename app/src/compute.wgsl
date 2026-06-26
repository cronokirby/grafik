// Raymarches a simple SDF scene and shades each hit by its surface normal,
// encoded as RGB. Rays that miss the scene are black.

@group(0) @binding(0) var output: texture_storage_2d<rgba8unorm, write>;

const MAX_STEPS: u32 = 100u;
const MAX_DIST: f32 = 20.0;
const SURF_DIST: f32 = 0.001;

// Signed distance to the scene: two spheres.
fn scene(p: vec3<f32>) -> f32 {
    let s1 = length(p - vec3<f32>(-1.2, 0.0, -4.0)) - 1.5;
    let s2 = length(p - vec3<f32>(1.5, 0.5, -6.0)) - 1.0;
    return min(s1, s2);
}

// Estimate the surface normal at `p` via the gradient of the SDF.
fn calc_normal(p: vec3<f32>) -> vec3<f32> {
    let e = vec2<f32>(0.0005, 0.0);
    let n = vec3<f32>(
        scene(p + e.xyy) - scene(p - e.xyy),
        scene(p + e.yxy) - scene(p - e.yxy),
        scene(p + e.yyx) - scene(p - e.yyx),
    );
    return normalize(n);
}

// March a ray from `ro` along `rd`, returning the distance travelled to the
// first surface hit, or a negative value if the ray missed.
fn raymarch(ro: vec3<f32>, rd: vec3<f32>) -> f32 {
    var t = 0.0;
    for (var i: u32 = 0u; i < MAX_STEPS; i++) {
        let p = ro + rd * t;
        let d = scene(p);
        if d < SURF_DIST {
            return t;
        }
        t += d;
        if t > MAX_DIST {
            break;
        }
    }
    return -1.0;
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = textureDimensions(output);
    if id.x >= dims.x || id.y >= dims.y {
        return;
    }

    // Normalized device coordinates in [-1, 1], with y pointing up and the
    // aspect ratio corrected so spheres stay round.
    let uv = vec2<f32>(f32(id.x), f32(id.y)) / vec2<f32>(dims);
    var ndc = uv * 2.0 - vec2<f32>(1.0, 1.0);
    ndc.y = -ndc.y;
    let aspect = f32(dims.x) / f32(dims.y);
    ndc.x *= aspect;

    // Pinhole camera at the origin looking down -z.
    let ro = vec3<f32>(0.0, 0.0, 0.0);
    let rd = normalize(vec3<f32>(ndc, -1.5));

    let t = raymarch(ro, rd);

    var color = vec3<f32>(0.1, 0.1, 0.1);
    if t >= 0.0 {
        let p = ro + rd * t;
        let normal = calc_normal(p);
        // Encode the normal (components in [-1, 1]) into RGB in [0, 1].
        color = normal * 0.5 + vec3<f32>(0.5, 0.5, 0.5);
    }

    textureStore(output, vec2<i32>(id.xy), vec4<f32>(color, 1.0));
}
