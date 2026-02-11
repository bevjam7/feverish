#import "shaders/psx_fx_core.wgsl"::{fx_enabled, FX_SNAP}

fn psx_snap_clip(clip: vec4<f32>, resolution: vec2<f32>, flags: u32) -> vec4<f32> {
    if !fx_enabled(flags, FX_SNAP) {
        return clip;
    }

    let w = clip.w;
    var ndc = clip.xy / w;
    ndc = floor(ndc * resolution) / resolution;
    return vec4<f32>(ndc * w, clip.z, w);
}
