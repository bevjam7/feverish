#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
}
#import "shaders/psx_fx_core.wgsl"::psx_post_fx

struct PsxExtUniform {
    resolution: vec2<f32>,
    quantize_steps: u32,
    flags: u32,
    dither_strength: f32,
    dither_scale: f32,
    dither_mode: u32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100)
var<uniform> psx: PsxExtUniform;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color =
        alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }

    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    out.color = psx_post_fx(
        out.color,
        in.position.xy,
        psx.resolution,
        psx.quantize_steps,
        psx.flags,
        psx.dither_strength,
        psx.dither_scale,
        psx.dither_mode,
    );
    return out;
}
