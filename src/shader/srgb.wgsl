fn encodeColor(c: vec4<f32>) -> vec4<f32> {
    return vec4(
        gamma(c.r),
        gamma(c.g),
        gamma(c.b),
        c.a,
    );
}

fn gamma(u: f32) -> f32 {
    return select(
        12.92 * u,
        1.055 * pow(u, 1.0 / 2.4) - 0.055,
        u > 0.0031308
    );
}
