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
var<uniform> ui_params_b: vec4<f32>; // x: autonomous chance, y: speed, z: reserved, w: mix
@group(#{MATERIAL_BIND_GROUP}) @binding(5)
var<uniform> ui_viewport: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6)
var<uniform> ui_cursor: vec4<f32>; // x/y normalized, z visible

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

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let uv_fold = crt_fold_uv(uv, ui_viewport);
    // Cursor keeps hallucination/fusion effects, but bypasses CRT monitor shaping.
    let cursor_monitor_bypass =
        ui_cursor.z * 0.42 * (1.0 - smoothstep(0.012, 0.042, distance(uv, ui_cursor.xy)));
    let uv_monitor = mix(uv_fold, uv, cursor_monitor_bypass);

    var fold_mask = crt_edge_fold_amount(uv, ui_viewport) * (1.0 - cursor_monitor_bypass);
    let screen_mask_raw = crt_corner_mask(uv, ui_viewport);
    let screen_mask = mix(screen_mask_raw, 1.0, cursor_monitor_bypass);

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
    rgb *= mix(1.0, scan, effect_mix * 0.65 * (1.0 - cursor_monitor_bypass));

    rgb = mix(rgb, rgb * ui_tint.rgb, ui_tint.a * effect_mix);
    let out_rgb = mix(base.rgb, rgb, effect_mix);
    return vec4<f32>(out_rgb, base.a * screen_mask);
}
