@group(0) @binding(0) var u_sampler: sampler;
@group(0) @binding(1) var u_back: texture_2d<f32>;
@group(0) @binding(2) var<uniform> u_params: Parameters;

@group(1) @binding(0) var u_base: texture_2d<f32>;
@group(1) @binding(1) var u_foil: texture_2d<f32>;
@group(1) @binding(2) var u_etch: texture_2d<f32>;

struct Parameters {
    n_samples: u32,
    max_iterations: u32,
    _padding: vec2<u32>,
}

struct VertexInput {
    @location(0) viewport: vec4<f32>,
    @location(1) size: vec2<f32>,
    @location(2) rotation: vec4<f32>,
    @builtin(vertex_index) index: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) viewport: vec4<f32>,
    @location(1) @interpolate(flat) size: vec2<f32>,
    @location(2) @interpolate(flat) rotation: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let corner = vec2<f32>(corner_position(input.index));

    out.position = vec4(2.0 * corner - 1.0, 0.0, 1.0);
    out.viewport = input.viewport;
    out.size = input.size;
    out.rotation = input.rotation;

    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let n_samples = u_params.n_samples;
    let max_iterations = u_params.max_iterations;

    const max_distance: f32 = 2.0;

    let position = input.position;
    let viewport = input.viewport;
    let size = input.size;
    let rotation = input.rotation;

    let max_dimension = f32(max(size.x, size.y));
    let card_size = size / (2.0 * max_dimension);

    let camera = vec3(0.0, 0.0, -max_distance);
    let light = vec3(3.0, 10.0, -20.0);
    let light_power = 700.0;

    var color: vec4<f32>;

    for (var m = u32(0); m < n_samples; m++) {
    for (var n = u32(0); n < n_samples; n++) {
        let o = vec2(f32(m), f32(n)) / f32(n_samples) - 0.5;
        let ray_origin = camera;

        let pixel = vec2<f32>(
            2.0 * (position.x - viewport.x + o.x) - viewport.z,
            -2.0 * (position.y - viewport.y + o.y) + viewport.w,
        ) / viewport.w;

        let ray_direction = normalize(vec3(pixel, 3.0));

        var t = -max_distance;

        for (var i = u32(0); i < max_iterations; i++) {
            let p = rotate_i(rotation, ray_origin + ray_direction * t);
            let d = sd_card(p, card_size);

            if d < 0.00001 || t > 2.0 * max_distance {
                break;
            }

            t += d;
        }

        if t <= 2.0 * max_distance {
            let hit_rotated = ray_origin + ray_direction * t;
            let hit = rotate_i(rotation, hit_rotated);
            let normal = estimate_normal(hit, card_size);
            let normal_abs = abs(normal);
            let N = rotate(rotation, normal);
            let V = -ray_direction;
            let L = normalize(light - hit_rotated);
            let light_strength = light_power / pow(distance(light, hit_rotated), 2.0);
            let light_angle = clamp(dot(N, normalize(L + V)), 0.0, 1.0);

            var sample: vec4<f32>;
            var specular_color = vec3(1.0, 1.0, 1.0);
            var foil_color: vec3<f32>;

            if (normal_abs.z > normal_abs.x && normal_abs.z > normal_abs.y) {
                let local_uv = hit.xy / (2.0 * card_size) + vec2(0.5, 0.5);
                let uv_offset = vec2(0.5, 0.5) - card_size;
                var final_uv = uv_offset + local_uv * card_size * 2.0;
                final_uv.y = 1.0 - final_uv.y;

                if (normal.z < 0.0) {
                    // Front
                    sample = textureSampleLevel(u_base, u_sampler, final_uv, 0.0);

                    let lumi = luminance(sample.xyz);
                    let etch = textureSampleLevel(u_etch, u_sampler, final_uv, 0.0).r;
                    let foil = textureSampleLevel(u_foil, u_sampler, final_uv, 0.0).r;
                    let purity = clamp(foil - 3.0 * etch, 0.0, 1.0);

                    if purity > 0.1 && lumi > 0.05 {
                        let strength = pow(light_angle, 48.0) * 6.0;
                        let angle = clamp(dot(N, V), 0.0, 1.0);

                        foil_color = mix(sample.xyz, iridescence(angle), 0.4) * purity * strength;
                        specular_color = vec3(0.0, 0.0, 0.0);

                        // Foil flakes
                        // Inspired by https://www.4rknova.com/blog/2025/08/30/foil-sticker
                        if purity > 0.2 {
                            let uFlakeReduction = 0.1;
                            let uFlakeThreshold = 0.5;
                            let uFlakeSize = 500.0;

                            // Procedural flake mask
                            let flake = hash(floor(local_uv * uFlakeSize));
                            let flakeMask = smoothstep(uFlakeReduction, 1.0, flake);
 
                            // Perturbed flake normal
                            let angleOffset = (hash(vec2(flake, flake + 3.0)) - 0.5) * 0.25;
                            let perturbedNormal = normalize(N + vec3(angleOffset, 0.0, angleOffset));
 
                            // Reflection for sparkle
                            let PR = reflect(-V, perturbedNormal);
 
                            // Dynamic flicker factor (only brightens, never darkens)
                            let flakePhase = hash(floor(local_uv * uFlakeSize) + floor(PR.xy * 15.0));
                            let phaseMod = mix(1.0, 1.8, flakePhase);
        
                            // Core sparkle factor (glimmer preserved)
                            var flakeSpec = pow(clamp(dot(perturbedNormal, V) * 0.5 + 0.5, 0.0, 1.0), 8.0);
                            flakeSpec = max(flakeSpec, 0.15); // always visible
 
                            let flakeIri = iridescence(dot(perturbedNormal, V));
 
                            // Final intensity
                            var flakeIntensity = flakeMask * purity * flakeSpec * phaseMod;
                            flakeIntensity = clamp(flakeIntensity, 0.0, 1.0);

                            foil_color += mix(sample.xyz, flakeIri, 0.6) * flakeIntensity;
                        }

                        foil_color *= light_power * 0.001;
                    }
                } else {
                    // Back
                    sample = textureSampleLevel(u_back, u_sampler, vec2(1.0 - final_uv.x, final_uv.y), 0.0);
                }
            } else {
                // Side edge
                sample = vec4(0.5, 0.5, 0.5, abs(N.z));
            }


            let ambient = 0.1;
            let diffusion = clamp(dot(N, L), 0.0, 1.0) * light_strength;
            let specular = pow(light_angle, 32.0) * light_strength * 0.05;

            color += vec4(sample.xyz * (ambient + diffusion) + specular_color * specular + foil_color, sample.a);
        }
    }
    }

    color /= f32(n_samples * n_samples);

    return encodeColor(color);
}

// Compute the normalized quad coordinates based on the vertex index.
fn corner_position(vertex_index: u32) -> vec2<u32> {
    // #: 0 1 2 3 4 5
    // x: 1 1 0 0 0 1
    // y: 1 0 0 0 1 1
    return vec2<u32>((vec2(1u, 2u) + vertex_index) % vec2(6u) < vec2(3u));
}

fn sd_card(p: vec3<f32>, size: vec2<f32>) -> f32 {
    return extrude(p, sd_rounded_box(p.xy, size, size.x / 20.0), size.x / 220.0);
}

fn sd_rounded_box(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2(0.0))) - r;
}

fn extrude(p: vec3<f32>, sdf: f32, h: f32) -> f32 {
    let w = vec2(sdf, abs(p.z) - h);
  	return min(max(w.x, w.y), 0.0) + length(max(w, vec2(0.0)));
}

fn estimate_normal(p: vec3<f32>, size: vec2<f32>) -> vec3<f32> {
    let eps = 0.00001;

    return normalize(vec3(
        sd_card(p + vec3(eps, 0, 0), size) - sd_card(p - vec3(eps, 0, 0), size),
        sd_card(p + vec3(0, eps, 0), size) - sd_card(p - vec3(0, eps, 0), size),
        sd_card(p + vec3(0, 0, eps), size) - sd_card(p - vec3(0, 0, eps), size)
    ));
}

fn iridescence(angle: f32) -> vec3<f32> {
    let thickness = 100.0 + 600.0 * (1.0 - angle);
    let phase = 6.28318 * thickness * 0.01;
    let rainbow = 0.5 + 0.5 * vec3(sin(phase), sin(phase + 2.094), sin(phase + 4.188));

    return mix(vec3(1.0), rainbow, 1.0);
}

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
}

fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3(0.2126, 0.7152, 0.0722));
}

// Beware! Quaternions below
fn rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let u = q.xyz;
    let s = q.w;
    return v + 2.0 * cross(u, cross(u, v) + s * v);
}

fn rotate_i(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    return rotate(vec4(-q.xyz, q.w), v);
}
