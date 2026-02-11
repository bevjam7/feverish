const FX_SNAP: u32 = 1u;
const FX_DITHER: u32 = 2u;
const FX_QUANTIZE: u32 = 4u;
const FX_AFFINE: u32 = 8u;

fn fx_enabled(flags: u32, bit: u32) -> bool {
    return (flags & bit) != 0u;
}

fn saturate3(v: vec3<f32>) -> vec3<f32> {
    return clamp(v, vec3(0.0), vec3(1.0));
}

fn bayer4(pix: vec2<f32>) -> f32 {
    let x = u32(pix.x) & 3u;
    let y = u32(pix.y) & 3u;
    let i = y * 4u + x;
    let m = array<f32, 16>(
        0.0 / 16.0,
        8.0 / 16.0,
        2.0 / 16.0,
        10.0 / 16.0,
        12.0 / 16.0,
        4.0 / 16.0,
        14.0 / 16.0,
        6.0 / 16.0,
        3.0 / 16.0,
        11.0 / 16.0,
        1.0 / 16.0,
        9.0 / 16.0,
        15.0 / 16.0,
        7.0 / 16.0,
        13.0 / 16.0,
        5.0 / 16.0
    );
    return m[i] - 0.5;
}

fn ign(pix: vec2<f32>) -> f32 {
    let f = fract(52.9829189 * fract(0.06711056 * pix.x + 0.00583715 * pix.y));
    return f - 0.5;
}

fn hash_noise(pix: vec2<f32>) -> f32 {
    let p = vec2<u32>(u32(pix.x), u32(pix.y));
    var x = p.x * 1973u + p.y * 9277u + 89173u;
    x = (x << 13u) ^ x;
    let n = x * (x * x * 15731u + 789221u) + 1376312589u;
    return (f32(n & 1023u) / 1023.0) - 0.5;
}

fn dither_value(pix: vec2<f32>, mode: u32) -> f32 {
    if mode == 3u {
        return ign(pix);
    } else if mode == 4u {
        return hash_noise(pix);
    }
    return bayer4(pix);
}

fn quantize_rgb(
    rgb: vec3<f32>,
    pix: vec2<f32>,
    steps_u: u32,
    flags: u32,
    dither_strength: f32,
    dither_mode: u32,
) -> vec3<f32> {
    if !fx_enabled(flags, FX_QUANTIZE) {
        return rgb;
    }

    let levels = f32(max(steps_u, 2u)) - 1.0;

    var d = 0.0;
    if fx_enabled(flags, FX_DITHER) {
        d = dither_value(pix, dither_mode) * dither_strength;
    }

    // use unbiased quantization to avoid darkening from floor() bias
    let rgb_q = round(rgb * levels + d) / max(levels, 0.001);
    return saturate3(rgb_q);
}

fn psx_post_fx(
    color: vec4<f32>,
    frag_coord_xy: vec2<f32>,
    _resolution: vec2<f32>,
    quantize_steps: u32,
    flags: u32,
    dither_strength: f32,
    dither_scale: f32,
    dither_mode: u32,
) -> vec4<f32> {
    let pix = floor(frag_coord_xy) / max(dither_scale, 0.001);
    let rgb = quantize_rgb(
        color.rgb,
        pix,
        quantize_steps,
        flags,
        dither_strength,
        dither_mode,
    );
    return vec4<f32>(rgb, color.a);
}
