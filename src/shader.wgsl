struct Uniforms {
    resolution: vec2<u32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var u_sampler: sampler;
@group(0) @binding(2) var u_back: texture_2d<f32>;

@group(1) @binding(0) var u_base: texture_2d<f32>;
@group(1) @binding(1) var u_foil: texture_2d<f32>;
@group(1) @binding(2) var u_etch: texture_2d<f32>;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) size: vec2<u32>,
    @location(2) rotation: f32,
    @builtin(vertex_index) index: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) center: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) rotation: f32,
}

const fov: f32 = 60.0;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let max_dimension = f32(max(input.size.x, input.size.y));
    let max_distance = max_dimension * 1.2;
    let camera = vec3(0.0, 0.0, -max_distance);
    let distance_to_camera = length(vec3(input.position, 0.0) - camera);

    let half_fov = fov / 2.0;
    let scale = distance_to_camera * tan(half_fov); 

    let corner = corner_position(input.index);
    let position = scale * vec2<f32>(corner) + (input.position + vec2<f32>(uniforms.resolution) / 2.0) - scale / 2.0;

    out.center = vec2<f32>(input.position);
    out.size = vec2<f32>(input.size);
    out.rotation = input.rotation;
    out.position = vec4<f32>(2.0 * position / vec2<f32>(uniforms.resolution) - 1.0, 0.0, 1.0) ;

    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    const n_samples: i32 = 2;
    const max_distance: f32 = 2.0;

    let max_dimension = f32(max(input.size.x, input.size.y));
    let card_size = input.size / (2.0 * max_dimension);

    let camera = vec3(0.0, 0.0, -max_distance);
    let fov = radians(80.00);
    let aspect = f32(uniforms.resolution.x) / f32(uniforms.resolution.y);

    let light = vec3(3.0, 8.0, -20.0);
    let light_power = 300.0;

    let cos_rot = cos(input.rotation);
    let sin_rot = sin(input.rotation);

    let rotation = mat3x3<f32>(
        vec3(cos_rot, 0.0, sin_rot),
        vec3(0.0, 1.0, 0.0),
        vec3(-sin_rot, 0.0, cos_rot),
    );

    var color: vec4<f32>;

    for (var m = 0; m < n_samples; m++) {
    for (var n = 0; n < n_samples; n++) {
        let o = vec2(f32(m), f32(n)) / f32(n_samples) - 0.5;
        let ray_origin = camera;

        let pixel = vec2<f32>(
            2.0 * (input.position.x + o.x) - f32(uniforms.resolution.x),
            -2.0 * (input.position.y + o.y) + f32(uniforms.resolution.y),
        ) / f32(uniforms.resolution.y);

        let ray_direction = normalize(vec3(pixel.xy, 3.0));

        var t = -max_distance;

        for (var i = 0; i < 64; i++) {
            let p = transpose(rotation) * (ray_origin + ray_direction * t);
            let d = sd_card(p, card_size);

            if d < 0.00001 || t > 2.0 * max_distance {
                break;
            }

            t += d;
        }

        if t <= 2.0 * max_distance {
            let hit = transpose(rotation) * (ray_origin + ray_direction * t);
            let normal = estimate_normal(hit, card_size);
            let normal_abs = abs(normal);

            var sample: vec4<f32>;

            if (normal_abs.z > normal_abs.x && normal_abs.z > normal_abs.y) {
                let local_uv = hit.xy / (2.0 * card_size) + vec2(0.5, 0.5);
                let uv_offset = vec2(0.5, 0.5) - card_size;
                var final_uv = uv_offset + local_uv * card_size * 2.0;
                final_uv.y = 1.0 - final_uv.y;

                if (normal.z < 0.0) {
                    // Front
                    sample = textureSampleLevel(u_base, u_sampler, final_uv, 0.0);
                } else {
                    // Back
                    sample = textureSampleLevel(u_back, u_sampler, vec2(1.0 - final_uv.x, final_uv.y), 0.0);
                }
            } else {
                // Side edge
                sample = vec4(0.5, 0.5, 0.5, abs((rotation * normal).z));
            }

            let hit_rotated = rotation * hit;
            let normal_rotated = rotation * normal;
            let light_dir = normalize(light - hit_rotated);
            let light_strength = light_power / pow(distance(light, hit_rotated), 2.0);

            let ambient = 0.05;
            let diffusion = clamp(dot(normal_rotated, light_dir), 0.0, 1.0) * light_strength;
            let specular = pow(
                clamp(dot(normal_rotated, normalize(light_dir - ray_direction)), 0.0, 1.0),
                60.0,
            ) * light_strength;

            color += vec4(sample.xyz * (ambient + diffusion) + specular, sample.a);
        }
    }
    }

    color /= f32(n_samples * n_samples);

    return color;
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
