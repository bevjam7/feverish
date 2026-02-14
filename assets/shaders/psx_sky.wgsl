#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings::{globals, view},
}
#import "shaders/psx_fx_core.wgsl"::bayer4

struct SkyUniformData {
    color_top: vec4<f32>,
    color_bottom: vec4<f32>,
    resolution: vec2<f32>,
    seed: f32,
    star_threshold: f32,
    micro_star_threshold: f32,
    flags: u32,
    nebula_strength: f32,
    dither_strength: f32,
    detail_scale: f32,
    horizon_haze_strength: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> sky: SkyUniformData;

const PI: f32 = 3.14159265;
const TAU: f32 = 6.28318531;
const FLAG_ORION_BELT: u32 = 1u;
const FLAG_PROC_A: u32 = 2u;
const FLAG_SCORPIUS: u32 = 4u;
const FLAG_CYGNUS: u32 = 8u;
const FLAG_URSA_MAJOR: u32 = 16u;

fn hash21(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var amp = 0.5;
    var pos = p;
    for (var i = 0; i < 4; i += 1) {
        v += noise(pos) * amp;
        pos = pos * 2.0 + vec2<f32>(43.0, 17.0);
        amp *= 0.5;
    }
    return v;
}

fn rotate_y(v: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(v.x * c - v.z * s, v.y, v.x * s + v.z * c);
}

fn uv_to_dir(uv: vec2<f32>) -> vec3<f32> {
    let phi = uv.x * TAU;
    let theta = uv.y * PI;
    return vec3<f32>(
        sin(theta) * cos(phi),
        cos(theta),
        sin(theta) * sin(phi)
    );
}

fn draw_star(dir: vec3<f32>, center: vec3<f32>, radius: f32) -> f32 {
    let d = distance(dir, center);
    return 1.0 - smoothstep(radius, radius * 1.8, d);
}

fn draw_segment(dir: vec3<f32>, a: vec3<f32>, b: vec3<f32>, thickness: f32) -> f32 {
    let pa = dir - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 0.0001), 0.0, 1.0);
    let d = length(pa - ba * h);
    return 1.0 - smoothstep(thickness, thickness * 1.8, d);
}

fn hash22(p: vec2<f32>) -> vec2<f32> {
    let x = hash21(p + vec2<f32>(19.19, 73.73));
    let y = hash21(p + vec2<f32>(61.13, 11.71));
    return vec2<f32>(x, y);
}

fn wrap_delta_uv(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    var d = a - b;
    d.x = d.x - round(d.x);
    return d;
}

fn stellar_density(sample_uv: vec2<f32>, sample_dir: vec3<f32>) -> f32 {
    let n0 = noise(sample_uv * 8.0 + vec2<f32>(sky.seed * 0.12, -sky.seed * 0.09));
    let n1 = noise(sample_uv * 21.0 + vec2<f32>(sky.seed * 0.28, sky.seed * 0.16));
    let filaments = smoothstep(0.54, 0.84, n0 * 0.74 + n1 * 0.34);
    let bridge = smoothstep(0.56, 0.92, abs(n0 - n1 + 0.08));

    let axis = normalize(vec3<f32>(
        sin(sky.seed * 0.091) * 0.62,
        0.24,
        cos(sky.seed * 0.077) * 0.62
    ));
    let lane = exp(-pow(abs(dot(sample_dir, axis)) * 4.2, 2.0));

    // dead zones / void pockets to break uniform spread
    let void0 = noise(sample_uv * 2.7 + vec2<f32>(sky.seed * 0.07, sky.seed * 0.03));
    let void1 = noise(sample_uv * 5.3 + vec2<f32>(-sky.seed * 0.05, sky.seed * 0.11));
    let dead_zone = smoothstep(0.60, 0.88, void0 * 0.74 + void1 * 0.26);
    let alive = 1.0 - dead_zone;

    // cluster boosts to create denser star neighborhoods and "bridges"
    let cluster0 = noise(sample_uv * 6.6 + vec2<f32>(sky.seed * 0.19, -sky.seed * 0.12));
    let cluster1 = noise(sample_uv * 14.0 + vec2<f32>(-sky.seed * 0.23, sky.seed * 0.09));
    let cluster = smoothstep(0.57, 0.90, cluster0 * 0.68 + cluster1 * 0.32);

    let gain = clamp((1.0 - sky.star_threshold) * 18.0, 0.10, 0.85);
    let density = (0.002 + lane * 0.034 + filaments * 0.042 + bridge * 0.016 + cluster * 0.058) * alive;
    return clamp(density * gain, 0.0, 0.12);
}

fn star_grain_field(uv: vec2<f32>, pix_res: vec2<f32>) -> vec3<f32> {
    let grid = max(pix_res * vec2<f32>(1.12, 1.12), vec2<f32>(160.0, 100.0));
    let cell = floor(uv * grid);
    var stars = vec3<f32>(0.0);

    for (var iy = -1; iy <= 1; iy += 1) {
        for (var ix = -1; ix <= 1; ix += 1) {
            let c = cell + vec2<f32>(f32(ix), f32(iy));
            let cell_uv = (c + vec2<f32>(0.5, 0.5)) / grid;
            let density = stellar_density(cell_uv, uv_to_dir(cell_uv));
            let spawn = hash21(c + vec2<f32>(sky.seed * 6.3, sky.seed * 2.9));
            if (spawn < density) {
                let jitter = hash22(c + vec2<f32>(71.3 + sky.seed, 13.1 - sky.seed));
                let star_uv = (c + jitter) / grid;
                let d = wrap_delta_uv(uv, star_uv) * pix_res;
                let r2 = dot(d, d);
                let core = exp(-52.0 * r2);
                if (core > 0.015) {
                    let twinkle_seed = hash21(c + vec2<f32>(5.0, 91.0));
                    let speed = 1.1 + hash21(c + vec2<f32>(33.0, 19.0)) * 3.4;
                    let phase = twinkle_seed * TAU;
                    let twinkle = clamp(
                        0.62
                            + 0.20 * sin(globals.time * speed + phase)
                            + 0.07 * sin(globals.time * speed * 0.43 + phase * 1.73),
                        0.26,
                        1.0
                    );

                    let temp = hash21(c + vec2<f32>(89.0, 47.0));
                    let tint = mix(
                        vec3<f32>(1.0, 0.96, 0.90),
                        vec3<f32>(0.70, 0.85, 1.0),
                        temp
                    );

                    var brightness = mix(0.16, 0.42, hash21(c + vec2<f32>(2.0, 17.0)));
                    let rare_bright = hash21(c + vec2<f32>(107.0, 31.0));
                    if (rare_bright > sky.micro_star_threshold) {
                        brightness += 0.20;
                    }
                    stars += tint * core * twinkle * brightness * 0.75;
                }
            }
        }
    }

    return stars;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let res = max(sky.resolution, vec2<f32>(1.0, 1.0));
    let detail_res = max(res * max(sky.detail_scale, 1.0), vec2<f32>(1.0, 1.0));

    let world_dir = normalize(in.world_position.xyz - view.world_position);
    let phi = atan2(world_dir.z, world_dir.x);
    let theta = acos(clamp(world_dir.y, -1.0, 1.0));
    let uv = vec2<f32>(fract(phi / TAU + 1.0), clamp(theta / PI, 0.0, 1.0));
    let snapped_uv = (floor(uv * res) + 0.5) / res;
    let detail_uv = (floor(uv * detail_res) + 0.5) / detail_res;

    let dir = uv_to_dir(snapped_uv);
    let detail_dir = uv_to_dir(detail_uv);

    let horizon_mix = smoothstep(-0.35, 0.9, dir.y);
    var color = mix(sky.color_bottom.rgb, sky.color_top.rgb, horizon_mix);

    let nebula_uv = vec2<f32>(
        snapped_uv.x * 8.0 + sky.seed * 0.11,
        snapped_uv.y * 4.0 - sky.seed * 0.07
    );
    let nebula = fbm(nebula_uv);
    let nebula_mask = smoothstep(0.45, 0.85, nebula) * sky.nebula_strength;
    let nebula_color = mix(
        vec3<f32>(0.30, 0.10, 0.42),
        vec3<f32>(0.06, 0.20, 0.30),
        nebula
    );
    color = mix(color, nebula_color, nebula_mask);

    let horizon_band = 1.0 - smoothstep(0.01, 0.30, abs(dir.y));
    let haze_noise = noise(vec2<f32>(snapped_uv.x * 140.0 + sky.seed, snapped_uv.y * 26.0));
    let haze_amount = horizon_band * sky.horizon_haze_strength * (0.84 + 0.16 * haze_noise);
    let haze_color = mix(sky.color_bottom.rgb, vec3<f32>(0.17, 0.22, 0.30), 0.62);
    color = mix(color, haze_color, haze_amount);

    let lower_band = 1.0 - smoothstep(-0.35, 0.08, dir.y);
    color = mix(color, vec3<f32>(0.13, 0.18, 0.24), lower_band * 0.24 * sky.horizon_haze_strength);

    let moon_dir = normalize(vec3<f32>(-0.78, 0.31, -0.24));
    let moon_radius = 0.030;

    var star_exclusion = 0.0;
    let moon_star_block =
        1.0 - smoothstep(moon_radius * 1.15, moon_radius * 2.05, distance(detail_dir, moon_dir));
    star_exclusion = max(star_exclusion, moon_star_block);

    if ((sky.flags & FLAG_ORION_BELT) != 0u) {
        let em1 = normalize(vec3<f32>(0.20, 0.41, -0.80));
        let em2 = normalize(vec3<f32>(0.25, 0.43, -0.80));
        let em3 = normalize(vec3<f32>(0.30, 0.45, -0.80));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, em1, 0.010));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, em2, 0.010));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, em3, 0.010));
        star_exclusion = max(star_exclusion, draw_segment(detail_dir, em1, em3, 0.0040) * 0.9);
    }
    if ((sky.flags & FLAG_SCORPIUS) != 0u) {
        let sc1 = normalize(vec3<f32>(0.42, 0.15, -0.60));
        let sc2 = normalize(vec3<f32>(0.38, 0.25, -0.65));
        let sc3 = normalize(vec3<f32>(0.35, 0.32, -0.70));
        let sc4 = normalize(vec3<f32>(0.28, 0.38, -0.72));
        let sc5 = normalize(vec3<f32>(0.20, 0.40, -0.75));
        let sc6 = normalize(vec3<f32>(0.12, 0.35, -0.70));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, sc1, 0.009));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, sc2, 0.011));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, sc3, 0.008));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, sc4, 0.010));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, sc5, 0.012));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, sc6, 0.008));
        star_exclusion = max(star_exclusion, draw_segment(detail_dir, sc1, sc2, 0.0035) * 0.85);
        star_exclusion = max(star_exclusion, draw_segment(detail_dir, sc2, sc3, 0.0030) * 0.85);
        star_exclusion = max(star_exclusion, draw_segment(detail_dir, sc3, sc4, 0.0028) * 0.85);
        star_exclusion = max(star_exclusion, draw_segment(detail_dir, sc4, sc5, 0.0032) * 0.85);
        star_exclusion = max(star_exclusion, draw_segment(detail_dir, sc5, sc6, 0.0035) * 0.85);
    }
    if ((sky.flags & FLAG_CYGNUS) != 0u) {
        let cy1 = normalize(vec3<f32>(-0.35, 0.55, -0.50));
        let cy2 = normalize(vec3<f32>(-0.20, 0.60, -0.55));
        let cy3 = normalize(vec3<f32>(0.0, 0.62, -0.58));
        let cy4 = normalize(vec3<f32>(0.20, 0.58, -0.55));
        let cy5 = normalize(vec3<f32>(0.35, 0.52, -0.50));
        let cy6 = normalize(vec3<f32>(0.0, 0.75, -0.40));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, cy1, 0.009));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, cy2, 0.010));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, cy3, 0.012));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, cy4, 0.010));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, cy5, 0.009));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, cy6, 0.011));
        star_exclusion = max(star_exclusion, draw_segment(detail_dir, cy1, cy3, 0.0030) * 0.85);
        star_exclusion = max(star_exclusion, draw_segment(detail_dir, cy3, cy5, 0.0030) * 0.85);
        star_exclusion = max(star_exclusion, draw_segment(detail_dir, cy2, cy6, 0.0025) * 0.75);
        star_exclusion = max(star_exclusion, draw_segment(detail_dir, cy4, cy6, 0.0025) * 0.75);
    }
    if ((sky.flags & FLAG_URSA_MAJOR) != 0u) {
        let um1 = normalize(vec3<f32>(0.55, 0.65, -0.20));
        let um2 = normalize(vec3<f32>(0.45, 0.72, -0.25));
        let um3 = normalize(vec3<f32>(0.32, 0.75, -0.30));
        let um4 = normalize(vec3<f32>(0.18, 0.78, -0.35));
        let um5 = normalize(vec3<f32>(0.05, 0.75, -0.38));
        let um6 = normalize(vec3<f32>(-0.10, 0.70, -0.40));
        let um7 = normalize(vec3<f32>(-0.22, 0.62, -0.45));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, um1, 0.010));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, um2, 0.011));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, um3, 0.012));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, um4, 0.011));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, um5, 0.010));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, um6, 0.009));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, um7, 0.008));
        star_exclusion = max(star_exclusion, draw_segment(detail_dir, um1, um2, 0.0030) * 0.85);
        star_exclusion = max(star_exclusion, draw_segment(detail_dir, um2, um3, 0.0028) * 0.85);
        star_exclusion = max(star_exclusion, draw_segment(detail_dir, um3, um4, 0.0028) * 0.85);
        star_exclusion = max(star_exclusion, draw_segment(detail_dir, um4, um5, 0.0030) * 0.85);
        star_exclusion = max(star_exclusion, draw_segment(detail_dir, um5, um6, 0.0032) * 0.85);
        star_exclusion = max(star_exclusion, draw_segment(detail_dir, um6, um7, 0.0035) * 0.85);
    }
    if ((sky.flags & FLAG_PROC_A) != 0u) {
        let e_angle = fract(sin(sky.seed) * 43758.5453) * TAU;
        let ep1 = normalize(rotate_y(vec3<f32>(-0.55, 0.58, -0.53), e_angle));
        let ep2 = normalize(rotate_y(vec3<f32>(-0.37, 0.69, -0.61), e_angle));
        let ep3 = normalize(rotate_y(vec3<f32>(-0.28, 0.62, -0.72), e_angle));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, ep1, 0.011));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, ep2, 0.009));
        star_exclusion = max(star_exclusion, draw_star(detail_dir, ep3, 0.008));
        star_exclusion = max(star_exclusion, draw_segment(detail_dir, ep1, ep2, 0.0038) * 0.85);
        star_exclusion = max(star_exclusion, draw_segment(detail_dir, ep2, ep3, 0.0034) * 0.80);
    }
    let star_keep = 1.0 - clamp(star_exclusion, 0.0, 1.0);
    let star_visibility = smoothstep(-0.12, 0.44, dir.y) * star_keep * star_keep;

    // broad cloud glow driven by clustered regions (but respecting dead zones)
    let glow_noise_a = noise(snapped_uv * 4.2 + vec2<f32>(sky.seed * 0.10, -sky.seed * 0.07));
    let glow_noise_b = noise(snapped_uv * 10.2 + vec2<f32>(-sky.seed * 0.16, sky.seed * 0.09));
    let glow_cluster = smoothstep(0.55, 0.88, glow_noise_a * 0.66 + glow_noise_b * 0.34);
    let glow_void = smoothstep(0.60, 0.86, noise(snapped_uv * 2.3 + vec2<f32>(sky.seed * 0.05, sky.seed * 0.04)));
    let glow_field = glow_cluster * (1.0 - glow_void);
    let glow_tint = mix(
        vec3<f32>(0.08, 0.12, 0.22),
        vec3<f32>(0.21, 0.17, 0.31),
        noise(snapped_uv * 6.0 + vec2<f32>(sky.seed * 0.08, -sky.seed * 0.11))
    );
    color += glow_tint * glow_field * star_visibility * 0.11;

    color += star_grain_field(detail_uv, detail_res) * star_visibility;

    let dust = smoothstep(
        0.58,
        0.94,
        fbm(vec2<f32>(detail_uv.x * 24.0 + sky.seed, detail_uv.y * 12.0 - sky.seed * 0.4))
    );
    color += vec3<f32>(0.05, 0.07, 0.12) * dust * 0.02;

    var constellation_core = 0.0;
    var constellation_halo = 0.0;
    if ((sky.flags & FLAG_ORION_BELT) != 0u) {
        let m1 = normalize(vec3<f32>(0.20, 0.41, -0.80));
        let m2 = normalize(vec3<f32>(0.25, 0.43, -0.80));
        let m3 = normalize(vec3<f32>(0.30, 0.45, -0.80));
        let m1_t = 0.86 + 0.14 * sin(globals.time * 1.08 + sky.seed * 0.31 + 0.6);
        let m2_t = 0.86 + 0.14 * sin(globals.time * 1.16 + sky.seed * 0.27 + 1.9);
        let m3_t = 0.86 + 0.14 * sin(globals.time * 1.03 + sky.seed * 0.35 + 3.1);
        constellation_core += draw_star(detail_dir, m1, 0.0044) * m1_t;
        constellation_core += draw_star(detail_dir, m2, 0.0044) * m2_t;
        constellation_core += draw_star(detail_dir, m3, 0.0044) * m3_t;
        constellation_halo += draw_star(detail_dir, m1, 0.010) * 0.36 * m1_t;
        constellation_halo += draw_star(detail_dir, m2, 0.010) * 0.36 * m2_t;
        constellation_halo += draw_star(detail_dir, m3, 0.010) * 0.36 * m3_t;
        constellation_core += draw_segment(detail_dir, m1, m3, 0.0023) * 0.28;
    }
    if ((sky.flags & FLAG_PROC_A) != 0u) {
        let angle = fract(sin(sky.seed) * 43758.5453) * TAU;
        let p1 = normalize(rotate_y(vec3<f32>(-0.55, 0.58, -0.53), angle));
        let p2 = normalize(rotate_y(vec3<f32>(-0.37, 0.69, -0.61), angle));
        let p3 = normalize(rotate_y(vec3<f32>(-0.28, 0.62, -0.72), angle));
        let p1_t = 0.84 + 0.16 * sin(globals.time * 0.93 + sky.seed * 0.23 + 2.2);
        let p2_t = 0.84 + 0.16 * sin(globals.time * 1.21 + sky.seed * 0.19 + 4.1);
        let p3_t = 0.84 + 0.16 * sin(globals.time * 1.07 + sky.seed * 0.17 + 5.0);
        constellation_core += draw_star(detail_dir, p1, 0.0052) * p1_t;
        constellation_core += draw_star(detail_dir, p2, 0.0039) * p2_t;
        constellation_core += draw_star(detail_dir, p3, 0.0035) * p3_t;
        constellation_halo += draw_star(detail_dir, p1, 0.011) * 0.24 * p1_t;
        constellation_halo += draw_star(detail_dir, p2, 0.008) * 0.18 * p2_t;
        constellation_halo += draw_star(detail_dir, p3, 0.007) * 0.16 * p3_t;
        constellation_core += draw_segment(detail_dir, p1, p2, 0.0020) * 0.24;
        constellation_core += draw_segment(detail_dir, p2, p3, 0.0018) * 0.20;
    }
    if ((sky.flags & FLAG_SCORPIUS) != 0u) {
        let sc1 = normalize(vec3<f32>(0.42, 0.15, -0.60));
        let sc2 = normalize(vec3<f32>(0.38, 0.25, -0.65));
        let sc3 = normalize(vec3<f32>(0.35, 0.32, -0.70));
        let sc4 = normalize(vec3<f32>(0.28, 0.38, -0.72));
        let sc5 = normalize(vec3<f32>(0.20, 0.40, -0.75));
        let sc6 = normalize(vec3<f32>(0.12, 0.35, -0.70));
        let sc_t = 0.86 + 0.14 * sin(globals.time * 1.05 + sky.seed * 0.29 + 1.5);
        constellation_core += draw_star(detail_dir, sc1, 0.0042) * sc_t;
        constellation_core += draw_star(detail_dir, sc2, 0.0048) * sc_t;
        constellation_core += draw_star(detail_dir, sc3, 0.0038) * sc_t;
        constellation_core += draw_star(detail_dir, sc4, 0.0045) * sc_t;
        constellation_core += draw_star(detail_dir, sc5, 0.0052) * sc_t;
        constellation_core += draw_star(detail_dir, sc6, 0.0038) * sc_t;
        constellation_halo += draw_star(detail_dir, sc1, 0.0095) * 0.32 * sc_t;
        constellation_halo += draw_star(detail_dir, sc2, 0.0105) * 0.34 * sc_t;
        constellation_halo += draw_star(detail_dir, sc3, 0.0085) * 0.30 * sc_t;
        constellation_halo += draw_star(detail_dir, sc4, 0.0098) * 0.32 * sc_t;
        constellation_halo += draw_star(detail_dir, sc5, 0.0110) * 0.35 * sc_t;
        constellation_halo += draw_star(detail_dir, sc6, 0.0085) * 0.30 * sc_t;
        constellation_core += draw_segment(detail_dir, sc1, sc2, 0.0022) * 0.26;
        constellation_core += draw_segment(detail_dir, sc2, sc3, 0.0020) * 0.24;
        constellation_core += draw_segment(detail_dir, sc3, sc4, 0.0018) * 0.22;
        constellation_core += draw_segment(detail_dir, sc4, sc5, 0.0020) * 0.24;
        constellation_core += draw_segment(detail_dir, sc5, sc6, 0.0022) * 0.26;
    }
    if ((sky.flags & FLAG_CYGNUS) != 0u) {
        let cy1 = normalize(vec3<f32>(-0.35, 0.55, -0.50));
        let cy2 = normalize(vec3<f32>(-0.20, 0.60, -0.55));
        let cy3 = normalize(vec3<f32>(0.0, 0.62, -0.58));
        let cy4 = normalize(vec3<f32>(0.20, 0.58, -0.55));
        let cy5 = normalize(vec3<f32>(0.35, 0.52, -0.50));
        let cy6 = normalize(vec3<f32>(0.0, 0.75, -0.40));
        let cy_t = 0.84 + 0.16 * sin(globals.time * 0.98 + sky.seed * 0.21 + 2.8);
        constellation_core += draw_star(detail_dir, cy1, 0.0042) * cy_t;
        constellation_core += draw_star(detail_dir, cy2, 0.0045) * cy_t;
        constellation_core += draw_star(detail_dir, cy3, 0.0052) * cy_t;
        constellation_core += draw_star(detail_dir, cy4, 0.0045) * cy_t;
        constellation_core += draw_star(detail_dir, cy5, 0.0042) * cy_t;
        constellation_core += draw_star(detail_dir, cy6, 0.0048) * cy_t;
        constellation_halo += draw_star(detail_dir, cy1, 0.0095) * 0.28 * cy_t;
        constellation_halo += draw_star(detail_dir, cy2, 0.0102) * 0.30 * cy_t;
        constellation_halo += draw_star(detail_dir, cy3, 0.0115) * 0.32 * cy_t;
        constellation_halo += draw_star(detail_dir, cy4, 0.0102) * 0.30 * cy_t;
        constellation_halo += draw_star(detail_dir, cy5, 0.0095) * 0.28 * cy_t;
        constellation_halo += draw_star(detail_dir, cy6, 0.0108) * 0.30 * cy_t;
        constellation_core += draw_segment(detail_dir, cy1, cy3, 0.0020) * 0.24;
        constellation_core += draw_segment(detail_dir, cy3, cy5, 0.0020) * 0.24;
        constellation_core += draw_segment(detail_dir, cy2, cy6, 0.0018) * 0.20;
        constellation_core += draw_segment(detail_dir, cy4, cy6, 0.0018) * 0.20;
    }
    if ((sky.flags & FLAG_URSA_MAJOR) != 0u) {
        let um1 = normalize(vec3<f32>(0.55, 0.65, -0.20));
        let um2 = normalize(vec3<f32>(0.45, 0.72, -0.25));
        let um3 = normalize(vec3<f32>(0.32, 0.75, -0.30));
        let um4 = normalize(vec3<f32>(0.18, 0.78, -0.35));
        let um5 = normalize(vec3<f32>(0.05, 0.75, -0.38));
        let um6 = normalize(vec3<f32>(-0.10, 0.70, -0.40));
        let um7 = normalize(vec3<f32>(-0.22, 0.62, -0.45));
        let um_t = 0.85 + 0.15 * sin(globals.time * 0.92 + sky.seed * 0.18 + 4.2);
        constellation_core += draw_star(detail_dir, um1, 0.0045) * um_t;
        constellation_core += draw_star(detail_dir, um2, 0.0048) * um_t;
        constellation_core += draw_star(detail_dir, um3, 0.0052) * um_t;
        constellation_core += draw_star(detail_dir, um4, 0.0048) * um_t;
        constellation_core += draw_star(detail_dir, um5, 0.0045) * um_t;
        constellation_core += draw_star(detail_dir, um6, 0.0042) * um_t;
        constellation_core += draw_star(detail_dir, um7, 0.0038) * um_t;
        constellation_halo += draw_star(detail_dir, um1, 0.0102) * 0.30 * um_t;
        constellation_halo += draw_star(detail_dir, um2, 0.0108) * 0.32 * um_t;
        constellation_halo += draw_star(detail_dir, um3, 0.0115) * 0.34 * um_t;
        constellation_halo += draw_star(detail_dir, um4, 0.0108) * 0.32 * um_t;
        constellation_halo += draw_star(detail_dir, um5, 0.0102) * 0.30 * um_t;
        constellation_halo += draw_star(detail_dir, um6, 0.0095) * 0.28 * um_t;
        constellation_halo += draw_star(detail_dir, um7, 0.0088) * 0.26 * um_t;
        constellation_core += draw_segment(detail_dir, um1, um2, 0.0020) * 0.24;
        constellation_core += draw_segment(detail_dir, um2, um3, 0.0018) * 0.22;
        constellation_core += draw_segment(detail_dir, um3, um4, 0.0018) * 0.22;
        constellation_core += draw_segment(detail_dir, um4, um5, 0.0020) * 0.24;
        constellation_core += draw_segment(detail_dir, um5, um6, 0.0022) * 0.26;
        constellation_core += draw_segment(detail_dir, um6, um7, 0.0024) * 0.28;
    }
    let constellation_pulse = 0.88 + 0.12 * sin(globals.time * 0.85 + sky.seed * 0.3);
    color += vec3<f32>(0.72, 0.96, 1.0) * constellation_core * 1.22 * constellation_pulse;
    color += vec3<f32>(0.32, 0.55, 0.78) * constellation_halo * 0.72;

    let moon_dist = distance(detail_dir, moon_dir);
    let moon = 1.0 - smoothstep(moon_radius, moon_radius * 1.08, moon_dist);
    let moon_halo = 1.0 - smoothstep(moon_radius * 1.3, moon_radius * 2.4, moon_dist);

    let basis_ref = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(moon_dir.y) > 0.93);
    let moon_right = normalize(cross(basis_ref, moon_dir));
    let moon_up = normalize(cross(moon_dir, moon_right));
    let moon_tangent = detail_dir - moon_dir * dot(detail_dir, moon_dir);
    let moon_uv = vec2<f32>(
        dot(moon_tangent, moon_right),
        dot(moon_tangent, moon_up)
    ) / max(moon_radius, 0.0001);

    let crater = fbm(moon_uv * 7.5 + vec2<f32>(sky.seed * 0.29, -sky.seed * 0.17));
    let crater_mask = smoothstep(0.47, 0.86, crater);
    let moon_xy2 = dot(moon_uv, moon_uv);
    let moon_z = sqrt(max(1.0 - moon_xy2, 0.0));
    let moon_normal = normalize(vec3<f32>(moon_uv, moon_z));
    let moon_light = normalize(vec3<f32>(-0.25, 0.20, 0.95));
    let lit = 0.78 + 0.22 * clamp(dot(moon_normal, moon_light), 0.0, 1.0);
    let moon_shimmer = 0.97 + 0.03 * sin(globals.time * 0.26 + sky.seed * 0.22);
    let moon_color = mix(vec3<f32>(0.67, 0.69, 0.74), vec3<f32>(0.95, 0.93, 0.88), crater_mask * 0.55 + 0.22);
    let moon_opacity = clamp(moon * 1.35, 0.0, 1.0);
    let moon_surface = moon_color * lit * moon_shimmer;
    color = mix(color, moon_surface, moon_opacity);
    color += vec3<f32>(0.22, 0.22, 0.30) * moon_halo * 0.16 * moon_shimmer * (1.0 - moon_opacity);

    color += bayer4(in.position.xy) * sky.dither_strength;
    color = clamp(round(color * 31.0) / 31.0, vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(color, 1.0);
}
