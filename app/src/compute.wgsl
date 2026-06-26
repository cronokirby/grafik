// Raymarches an SDF scene and shades hits with direct PBR lighting.

struct Sphere {
    // Center.xyz and radius.w.
    center_radius: vec4<f32>,
    // Albedo.rgb in [0, 1] and metallic.w in [0, 1].
    albedo_metallic: vec4<f32>,
    // Roughness.x and ambient occlusion.y in [0, 1].
    roughness_ao: vec4<f32>,
}

struct Light {
    // Position.xyz; w is unused.
    position: vec4<f32>,
    // Color.rgb in [0, 1] and scalar intensity.w.
    color_intensity: vec4<f32>,
}

@group(0) @binding(0)
var output: texture_storage_2d<rgba32float, write>;

@group(0) @binding(1)
var<storage, read> spheres: array<Sphere>;

@group(0) @binding(2)
var<storage, read> lights: array<Light>;

const MAX_STEPS: u32 = 100u;
const MAX_DIST: f32 = 20.0;
const SURF_DIST: f32 = 0.001;
const PI: f32 = 3.14159265359;

// Signed distance to the scene.
fn scene(p: vec3<f32>) -> f32 {
    var d = 1e20;
    for (var i = 0u; i < arrayLength(&spheres); i++) {
        let s = spheres[i].center_radius;
        d = min(d, length(p - s.xyz) - s.w);
    }
    return d;
}

fn closest_sphere_index(p: vec3<f32>) -> u32 {
    var closest = 0u;
    var closest_d = 1e20;
    for (var i = 0u; i < arrayLength(&spheres); i++) {
        let s = spheres[i].center_radius;
        let d = length(p - s.xyz) - s.w;
        if d < closest_d {
            closest = i;
            closest_d = d;
        }
    }
    return closest;
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

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let n_dot_h = max(dot(n, h), 0.0);
    let n_dot_h2 = n_dot_h * n_dot_h;
    let denom = n_dot_h2 * (a2 - 1.0) + 1.0;
    return a2 / max(PI * denom * denom, 0.000001);
}

fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot_v / max(n_dot_v * (1.0 - k) + k, 0.000001);
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let n_dot_v = max(dot(n, v), 0.0);
    let n_dot_l = max(dot(n, l), 0.0);
    let ggx2 = geometry_schlick_ggx(n_dot_v, roughness);
    let ggx1 = geometry_schlick_ggx(n_dot_l, roughness);
    return ggx1 * ggx2;
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn shade_pbr(p: vec3<f32>, n: vec3<f32>, v: vec3<f32>, sphere: Sphere) -> vec3<f32> {
    let albedo = clamp(sphere.albedo_metallic.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let metallic = clamp(sphere.albedo_metallic.w, 0.0, 1.0);
    let roughness = clamp(sphere.roughness_ao.x, 0.05, 1.0);
    let ao = clamp(sphere.roughness_ao.y, 0.0, 1.0);
    var f0 = vec3<f32>(0.04);
    f0 = mix(f0, albedo, metallic);

    var lo = vec3<f32>(0.0);
    for (var i = 0u; i < arrayLength(&lights); i++) {
        let light = lights[i];
        let to_light = light.position.xyz - p;
        let distance = length(to_light);
        let l = to_light / max(distance, 0.000001);
        let h = normalize(v + l);
        let attenuation = 1.0 / max(distance * distance, 0.000001);
        let radiance = light.color_intensity.rgb * light.color_intensity.w * attenuation;

        let ndf = distribution_ggx(n, h, roughness);
        let g = geometry_smith(n, v, l, roughness);
        let f = fresnel_schlick(max(dot(h, v), 0.0), f0);
        let numerator = ndf * g * f;
        let denominator = 4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 0.0001;
        let specular = numerator / denominator;

        let k_s = f;
        let k_d = (vec3<f32>(1.0) - k_s) * (1.0 - metallic);
        let n_dot_l = max(dot(n, l), 0.0);
        lo += (k_d * albedo / PI + specular) * radiance * n_dot_l;
    }

    let ambient = vec3<f32>(0.03) * albedo * ao;
    return ambient + lo;
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

    var color = vec3<f32>(0.0);
    if t >= 0.0 {
        let p = ro + rd * t;
        let n = calc_normal(p);
        let v = normalize(ro - p);
        let sphere = spheres[closest_sphere_index(p)];
        color = shade_pbr(p, n, v, sphere);
    }

    textureStore(output, vec2<i32>(id.xy), vec4<f32>(color, 1.0));
}
