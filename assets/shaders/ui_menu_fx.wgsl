#import bevy_sprite::{
    mesh2d_vertex_output::VertexOutput,
    mesh2d_view_bindings::globals,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var ui_tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var ui_sampler: sampler;

@group(#{MATERIAL_BIND_GROUP}) @binding(2)
var<uniform> ui_tint: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3)
var<uniform> ui_params_a: vec4<f32>; // x: pixel_size, y: quant_steps, z: dither_strength, w: melt_strength
@group(#{MATERIAL_BIND_GROUP}) @binding(4)
var<uniform> ui_params_b: vec4<f32>; // x: autonomous chance, y: speed, z: monitor_on, w: mix
@group(#{MATERIAL_BIND_GROUP}) @binding(5)
var<uniform> ui_viewport: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6)
var<uniform> ui_cursor: vec4<f32>; // x/y normalized, z visible, w cursor_distortion_on

fn hash12(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453);
}

fn bayer4(pix: vec2<f32>) -> f32 {
    let x = u32(pix.x) & 3u;
    let y = u32(pix.y) & 3u;
    let i = y * 4u + x;
    let m = array<f32, 16>(
        0.0 / 16.0, 8.0 / 16.0, 2.0 / 16.0, 10.0 / 16.0,
        12.0 / 16.0, 4.0 / 16.0, 14.0 / 16.0, 6.0 / 16.0,
        3.0 / 16.0, 11.0 / 16.0, 1.0 / 16.0, 9.0 / 16.0,
        15.0 / 16.0, 7.0 / 16.0, 13.0 / 16.0, 5.0 / 16.0
    );
    return m[i] - 0.5;
}

fn smooth_event_envelope(phase: f32) -> f32 {
    let fade_in = smoothstep(0.08, 0.28, phase);
    let fade_out = 1.0 - smoothstep(0.70, 0.98, phase);
    return clamp(fade_in * fade_out, 0.0, 1.0);
}

fn scanline_mask(y_px: f32, time: f32) -> f32 {
    let wave = sin(y_px * 3.14159265 + time * 0.7) * 0.5 + 0.5;
    return mix(0.86, 1.0, wave);
}

fn crt_corner_mask(uv: vec2<f32>, viewport: vec4<f32>) -> f32 {
    let aspect = max(viewport.x / max(viewport.y, 1.0), 0.001);
    let p = vec2<f32>((uv.x * 2.0 - 1.0) / aspect, uv.y * 2.0 - 1.0);
    // superellipse mask: clean rounded CRT corners without sharp artifacts.
    let shape = pow(abs(p.x), 4.2) + pow(abs(p.y), 4.2);
    return 1.0 - smoothstep(1.0, 1.06, shape);
}

fn crt_edge_fold_amount(uv: vec2<f32>, viewport: vec4<f32>) -> f32 {
    let m = crt_corner_mask(uv, viewport);
    return pow(1.0 - m, 0.65);
}

fn crt_fold_uv(uv: vec2<f32>, viewport: vec4<f32>) -> vec2<f32> {
    let aspect = max(viewport.x / max(viewport.y, 1.0), 0.001);
    var p = uv * 2.0 - vec2<f32>(1.0, 1.0);
    p.x *= aspect;

    // Subtle convex CRT glass warp.
    let r2 = dot(p, p);
    let radial = 1.0 + 0.013 * r2 + 0.004 * r2 * r2;
    p *= radial;
    p.x *= 1.0 + 0.015 * pow(abs(p.y), 2.0);
    p.y *= 1.0 + 0.019 * pow(abs(p.x) / aspect, 2.0);

    let out_uv = vec2<f32>(p.x / aspect, p.y) * 0.5 + vec2<f32>(0.5, 0.5);
    return clamp(out_uv, vec2<f32>(0.0), vec2<f32>(1.0));
}

fn autonomous_blob(uv: vec2<f32>, time: f32, slot: f32, chance: f32) -> f32 {
    let period = 9.0 + slot * 3.7;
    let raw = (time + slot * 6.13) / period;
    let phase = fract(raw);
    let event_id = floor(raw);
    let spawn_seed = hash12(vec2<f32>(event_id + 19.0 * slot, 7.0 + slot));
    var event_on = 0.0;
    if spawn_seed < chance {
        event_on = 1.0;
    }
    let envelope = smooth_event_envelope(phase) * event_on;

    let center = vec2<f32>(
        0.08 + hash12(vec2<f32>(event_id + 17.0 * slot, 11.0 + slot)) * 0.84,
        0.10 + hash12(vec2<f32>(23.0 + slot, event_id + 29.0 * slot)) * 0.80
    );
    let radius = 0.07 + hash12(vec2<f32>(event_id + slot * 13.0, 97.0)) * 0.12;
    let d = distance(uv, center);
    let core = 1.0 - smoothstep(radius * 0.18, radius, d);
    return envelope * core;
}

fn luma709(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn cool_emissive_weight(c: vec3<f32>) -> f32 {
    let l = luma709(c);
    let chroma_vec = c - vec3<f32>(l);
    let chroma = length(chroma_vec);
    let sat = chroma / max(l + 0.0001, 0.0001);

    let blue_ref = vec3<f32>(0.14, 0.30, 0.94);
    let purple_ref = vec3<f32>(0.62, 0.24, 0.88);
    let blue_ref_chroma = normalize(blue_ref - vec3<f32>(luma709(blue_ref)));
    let purple_ref_chroma = normalize(purple_ref - vec3<f32>(luma709(purple_ref)));
    let hue_vec = chroma_vec / max(chroma, 0.0001);
    let hue_match = max(dot(hue_vec, blue_ref_chroma), dot(hue_vec, purple_ref_chroma));

    let hue_w = smoothstep(0.22, 0.84, hue_match);
    let sat_w = smoothstep(0.06, 0.42, sat);
    let light_w = smoothstep(0.01, 0.40, l + chroma * 0.45);
    return clamp(hue_w * sat_w * light_w, 0.0, 1.0);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let monitor_on = select(0.0, 1.0, ui_params_b.z > 0.5);
    let cursor_distortion_on = select(0.0, 1.0, ui_cursor.w > 0.5);

    let uv_fold = crt_fold_uv(uv, ui_viewport);
    // Cursor locally relaxes monitor distortion (hallucinations stay active).
    let cursor_monitor_bypass =
        monitor_on
            * cursor_distortion_on
            * ui_cursor.z
            * 0.24
            * (1.0 - smoothstep(0.010, 0.034, distance(uv, ui_cursor.xy)));
    let monitor_mix = monitor_on * (1.0 - cursor_monitor_bypass);
    let uv_monitor = mix(uv, uv_fold, monitor_mix);

    var fold_mask = crt_edge_fold_amount(uv, ui_viewport) * monitor_mix;
    let screen_mask_raw = crt_corner_mask(uv, ui_viewport);
    let screen_mask = mix(1.0, screen_mask_raw, monitor_mix);

    let base = textureSample(ui_tex, ui_sampler, uv_monitor);
    if base.a <= 0.001 || screen_mask <= 0.001 {
        // preserve world/background untouched when UI has no coverage
        return vec4<f32>(0.0);
    }

    let effect_mix = clamp(ui_params_b.w, 0.0, 1.0);

    let pixel_size = max(ui_params_a.x, 1.0);
    let grid_px = floor(uv_monitor * ui_viewport.xy / pixel_size) * pixel_size + vec2<f32>(0.5 * pixel_size);
    let pix_uv = clamp(grid_px * ui_viewport.zw, vec2<f32>(0.0), vec2<f32>(1.0));

    let t = globals.time * (0.40 + ui_params_b.y * 0.45);

    let autonomous_chance = clamp(ui_params_b.x, 0.0, 1.0);
    let cursor_blob = ui_cursor.z * (1.0 - smoothstep(0.04, 0.19, distance(uv, ui_cursor.xy)));
    let auto_a = autonomous_blob(uv, t, 1.0, autonomous_chance);
    let auto_b = autonomous_blob(uv, t, 2.0, autonomous_chance);
    let auto_c = autonomous_blob(uv, t, 3.0, autonomous_chance);

    let autonomous_strength = max(auto_a, max(auto_b, auto_c));
    let melt_field = clamp(max(cursor_blob * 0.58, autonomous_strength), 0.0, 1.0) * effect_mix;

    // localized liquid fusion that stays readable: mild swirl + directional pull + drip
    let flow = vec2<f32>(
        sin(uv.y * 46.0 + t * 2.8),
        cos(uv.x * 39.0 - t * 3.2)
    ) * melt_field * ui_params_a.w * 0.0035;
    let shear = vec2<f32>((uv.x - 0.5) * melt_field * ui_params_a.w * 0.005, 0.0);
    let drip = vec2<f32>(0.0, 1.0) * pow(melt_field, 1.6) * ui_params_a.w * 0.014;

    let uv_main = clamp(pix_uv + flow + shear, vec2<f32>(0.0), vec2<f32>(1.0));
    let uv_down = clamp(pix_uv + flow * 1.8 + drip, vec2<f32>(0.0), vec2<f32>(1.0));
    let uv_side = clamp(
        pix_uv + flow * 1.2 + vec2<f32>(drip.y * (0.35 + 0.25 * sin(t + uv.y * 9.0)), 0.0),
        vec2<f32>(0.0),
        vec2<f32>(1.0)
    );

    let src = textureSample(ui_tex, ui_sampler, uv_main);
    let down = textureSample(ui_tex, ui_sampler, uv_down);
    let side = textureSample(ui_tex, ui_sampler, uv_side);
    let fuse_jitter = vec2<f32>(
        hash12(grid_px + vec2<f32>(t * 13.1, 31.0)) - 0.5,
        hash12(grid_px + vec2<f32>(71.0, t * 9.7)) - 0.5
    );
    let uv_fuse = clamp(pix_uv + fuse_jitter * melt_field * 0.018, vec2<f32>(0.0), vec2<f32>(1.0));
    let fused = textureSample(ui_tex, ui_sampler, uv_fuse);
    var rgb = src.rgb;
    rgb = mix(rgb, down.rgb, melt_field * 0.62);
    rgb = mix(rgb, side.rgb, melt_field * 0.28);
    rgb = mix(rgb, fused.rgb, melt_field * 0.35);

    // VHS pass (subtle hopefuly): head-switching, phosphor persistence, and vertical hold micro drift
    let line_id = floor(uv_monitor.y * ui_viewport.y * 0.22);
    let line_seed = hash12(vec2<f32>(line_id, floor(t * 8.0)));
    let line_gate = step(0.985, line_seed);
    let vhs_strength = effect_mix * (0.22 + 0.34 * monitor_mix);
    let hold_phase = fract(t * 0.105);
    let hold_env = smooth_event_envelope(hold_phase);
    let hold_drift =
        (sin(t * 0.37) * 0.00065 + sin(t * 1.9) * 0.00028)
        * (0.45 + 0.55 * hold_env)
        * vhs_strength;
    let wobble = (sin(t * 2.6 + uv_monitor.y * 120.0) + sin(t * 4.1 + uv_monitor.y * 47.0)) * 0.5;
    let tape_jitter = vec2<f32>(
        (wobble * 0.0009 + line_gate * (line_seed - 0.5) * 0.008) * vhs_strength,
        0.0
    );
    let hs_event = step(0.90, hash12(vec2<f32>(floor(t * 2.2), 41.0)));
    let hs_center = 0.90 + (hash12(vec2<f32>(floor(t * 2.2), 93.0)) - 0.5) * 0.08;
    let hs_band =
        (1.0 - smoothstep(0.0, 0.030, abs(uv_monitor.y - hs_center)))
        * hs_event
        * monitor_mix
        * effect_mix;
    let hs_shift = (hash12(vec2<f32>(line_id, floor(t * 120.0))) - 0.5) * 0.028 * hs_band;
    let uv_vhs = clamp(uv_main + tape_jitter + vec2<f32>(hs_shift, hold_drift), vec2<f32>(0.0), vec2<f32>(1.0));
    let vhs_ca = (0.00035 + 0.00055 * monitor_mix + 0.0007 * line_gate + 0.0009 * hs_band) * vhs_strength;
    let vhs_r = textureSample(ui_tex, ui_sampler, clamp(uv_vhs + vec2<f32>(vhs_ca, 0.0), vec2<f32>(0.0), vec2<f32>(1.0))).r;
    let vhs_g = textureSample(ui_tex, ui_sampler, uv_vhs).g;
    let vhs_b = textureSample(ui_tex, ui_sampler, clamp(uv_vhs - vec2<f32>(vhs_ca * 1.35, 0.0), vec2<f32>(0.0), vec2<f32>(1.0))).b;
    let vhs_rgb = vec3<f32>(vhs_r, vhs_g, vhs_b);
    let ghost_uv = clamp(uv_vhs + vec2<f32>(0.004 + 0.003 * line_gate, 0.0), vec2<f32>(0.0), vec2<f32>(1.0));
    let ghost = textureSample(ui_tex, ui_sampler, ghost_uv).rgb;
    let trail_a = textureSample(ui_tex, ui_sampler, clamp(uv_vhs - vec2<f32>(0.0026, 0.0), vec2<f32>(0.0), vec2<f32>(1.0))).rgb;
    let trail_b = textureSample(ui_tex, ui_sampler, clamp(uv_vhs - vec2<f32>(0.0052, 0.0), vec2<f32>(0.0), vec2<f32>(1.0))).rgb;
    let beam_luma = dot(vhs_rgb, vec3<f32>(0.299, 0.587, 0.114));
    let persistence = smoothstep(0.32, 0.92, beam_luma) * (0.45 + 0.55 * monitor_mix) * effect_mix;
    let phosphor_trail = trail_a * vec3<f32>(0.06, 0.12, 0.09) + trail_b * vec3<f32>(0.04, 0.08, 0.10);
    rgb = mix(rgb, vhs_rgb, 0.12 * vhs_strength);
    rgb = mix(rgb, ghost, (0.02 + 0.025 * line_gate + 0.04 * hs_band) * vhs_strength);
    rgb += phosphor_trail * 0.55 * persistence;
    let hs_snow = (hash12(vec2<f32>(grid_px.y + floor(t * 160.0), line_id + 17.0)) - 0.5) * 0.10 * hs_band;
    let grain = (hash12(grid_px + vec2<f32>(t * 60.0, t * 23.0)) - 0.5) * (0.015 + 0.02 * line_gate) * vhs_strength + hs_snow;
    rgb += vec3<f32>(grain);

    // only on borders: tiny color separation for CRT edge feel
    let ca = fold_mask * fold_mask * effect_mix * 0.00075;
    let edge_r = textureSample(ui_tex, ui_sampler, clamp(uv_main + vec2<f32>(ca, 0.0), vec2<f32>(0.0), vec2<f32>(1.0))).r;
    let edge_b = textureSample(ui_tex, ui_sampler, clamp(uv_main - vec2<f32>(ca, 0.0), vec2<f32>(0.0), vec2<f32>(1.0))).b;
    rgb = mix(rgb, vec3<f32>(edge_r, rgb.g, edge_b), fold_mask * 0.16);

    let levels = max(ui_params_a.y, 2.0) - 1.0;
    let d = bayer4(grid_px / pixel_size) * ui_params_a.z * effect_mix;
    rgb = round(rgb * levels + d) / levels;
    rgb = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));

    let scan = scanline_mask(grid_px.y, t);
    rgb *= mix(1.0, scan, effect_mix * 0.65 * monitor_mix);

    // CRT glow driven by color math: cool-hue emissive extraction + softer, wider bloom taps :p
    let glow_uv = mix(uv_vhs, uv_monitor, 0.72 * effect_mix);
    let glow_step = ui_viewport.zw * (1.6 + 1.2 * monitor_mix);
    let glow_step_far = glow_step * 2.1;
    let g0 = textureSample(ui_tex, ui_sampler, glow_uv).rgb;
    let gx0 = textureSample(ui_tex, ui_sampler, clamp(glow_uv + vec2<f32>(glow_step.x, 0.0), vec2<f32>(0.0), vec2<f32>(1.0))).rgb;
    let gx1 = textureSample(ui_tex, ui_sampler, clamp(glow_uv - vec2<f32>(glow_step.x, 0.0), vec2<f32>(0.0), vec2<f32>(1.0))).rgb;
    let gy0 = textureSample(ui_tex, ui_sampler, clamp(glow_uv + vec2<f32>(0.0, glow_step.y), vec2<f32>(0.0), vec2<f32>(1.0))).rgb;
    let gy1 = textureSample(ui_tex, ui_sampler, clamp(glow_uv - vec2<f32>(0.0, glow_step.y), vec2<f32>(0.0), vec2<f32>(1.0))).rgb;
    let gd0 = textureSample(ui_tex, ui_sampler, clamp(glow_uv + glow_step, vec2<f32>(0.0), vec2<f32>(1.0))).rgb;
    let gd1 = textureSample(ui_tex, ui_sampler, clamp(glow_uv + vec2<f32>(glow_step.x, -glow_step.y), vec2<f32>(0.0), vec2<f32>(1.0))).rgb;
    let gd2 = textureSample(ui_tex, ui_sampler, clamp(glow_uv + vec2<f32>(-glow_step.x, glow_step.y), vec2<f32>(0.0), vec2<f32>(1.0))).rgb;
    let gd3 = textureSample(ui_tex, ui_sampler, clamp(glow_uv - glow_step, vec2<f32>(0.0), vec2<f32>(1.0))).rgb;
    let gxx0 = textureSample(ui_tex, ui_sampler, clamp(glow_uv + vec2<f32>(glow_step_far.x, 0.0), vec2<f32>(0.0), vec2<f32>(1.0))).rgb;
    let gxx1 = textureSample(ui_tex, ui_sampler, clamp(glow_uv - vec2<f32>(glow_step_far.x, 0.0), vec2<f32>(0.0), vec2<f32>(1.0))).rgb;
    let gyy0 = textureSample(ui_tex, ui_sampler, clamp(glow_uv + vec2<f32>(0.0, glow_step_far.y), vec2<f32>(0.0), vec2<f32>(1.0))).rgb;
    let gyy1 = textureSample(ui_tex, ui_sampler, clamp(glow_uv - vec2<f32>(0.0, glow_step_far.y), vec2<f32>(0.0), vec2<f32>(1.0))).rgb;

    let w0 = cool_emissive_weight(g0) * 0.22;
    let wx0 = cool_emissive_weight(gx0) * 0.14;
    let wx1 = cool_emissive_weight(gx1) * 0.14;
    let wy0 = cool_emissive_weight(gy0) * 0.14;
    let wy1 = cool_emissive_weight(gy1) * 0.14;
    let wd0 = cool_emissive_weight(gd0) * 0.09;
    let wd1 = cool_emissive_weight(gd1) * 0.09;
    let wd2 = cool_emissive_weight(gd2) * 0.09;
    let wd3 = cool_emissive_weight(gd3) * 0.09;
    let wxx0 = cool_emissive_weight(gxx0) * 0.055;
    let wxx1 = cool_emissive_weight(gxx1) * 0.055;
    let wyy0 = cool_emissive_weight(gyy0) * 0.055;
    let wyy1 = cool_emissive_weight(gyy1) * 0.055;

    let glow_w = w0 + wx0 + wx1 + wy0 + wy1 + wd0 + wd1 + wd2 + wd3 + wxx0 + wxx1 + wyy0 + wyy1;
    let glow_sum =
        g0 * w0
        + gx0 * wx0
        + gx1 * wx1
        + gy0 * wy0
        + gy1 * wy1
        + gd0 * wd0
        + gd1 * wd1
        + gd2 * wd2
        + gd3 * wd3
        + gxx0 * wxx0
        + gxx1 * wxx1
        + gyy0 * wyy0
        + gyy1 * wyy1;
    let glow_color = glow_sum / max(glow_w, 0.0001);
    let glow_presence = smoothstep(0.025, 0.33, glow_w);
    let glow_gain = effect_mix * (0.072 + 0.096 * monitor_mix);
    rgb += glow_color * glow_presence * glow_gain;
    rgb = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));

    rgb = mix(rgb, rgb * ui_tint.rgb, ui_tint.a * effect_mix);
    let out_rgb = mix(base.rgb, rgb, effect_mix);
    return vec4<f32>(out_rgb, base.a * screen_mask);
}
