use bevy::{
    mesh::MeshVertexBufferLayoutRef,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    reflect::TypePath,
    render::render_resource::{
        AsBindGroup, Face, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
};

use crate::{AppSystems, psx::PsxCamera};

const SKY_SHADER_PATH: &str = "shaders/psx_sky.wgsl";
const SKY_RADIUS: f32 = 350.0;
const SKY_PIXEL_RESOLUTION: Vec2 = Vec2::new(320.0, 240.0);
const FLAG_ORION_BELT: u32 = 1;
const FLAG_PROC_A: u32 = 2;
const FLAG_SCORPIUS: u32 = 4;
const FLAG_CYGNUS: u32 = 8;
const FLAG_URSA_MAJOR: u32 = 16;

pub(super) struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<PsxSkyMaterial>::default())
            .add_systems(Startup, spawn_sky_dome)
            .add_systems(Update, follow_camera_translation.in_set(AppSystems::Update));
    }
}

#[derive(Component)]
struct SkyDome;

#[derive(Debug, Clone, Copy, ShaderType)]
struct SkyUniformData {
    color_top: Vec4,
    color_bottom: Vec4,
    resolution: Vec2,
    seed: f32,
    star_threshold: f32,
    micro_star_threshold: f32,
    flags: u32,
    nebula_strength: f32,
    dither_strength: f32,
    detail_scale: f32,
    horizon_haze_strength: f32,
}

impl Default for SkyUniformData {
    fn default() -> Self {
        Self {
            color_top: linear_color(0.055, 0.015, 0.120),
            color_bottom: linear_color(0.010, 0.135, 0.205),
            resolution: SKY_PIXEL_RESOLUTION,
            seed: 917.0,
            star_threshold: 0.968,
            micro_star_threshold: 0.997,
            flags: FLAG_ORION_BELT | FLAG_PROC_A | FLAG_SCORPIUS | FLAG_CYGNUS | FLAG_URSA_MAJOR,
            nebula_strength: 0.42,
            dither_strength: 0.05,
            detail_scale: 3.6,
            horizon_haze_strength: 0.56,
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(super) struct PsxSkyMaterial {
    #[uniform(0)]
    sky: SkyUniformData,
}

impl Default for PsxSkyMaterial {
    fn default() -> Self {
        Self {
            sky: SkyUniformData::default(),
        }
    }
}

impl Material for PsxSkyMaterial {
    fn fragment_shader() -> ShaderRef {
        SKY_SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = Some(Face::Front);
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = false;
        }
        Ok(())
    }
}

fn spawn_sky_dome(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<PsxSkyMaterial>>,
) {
    let mesh = Sphere::new(SKY_RADIUS).mesh().ico(5).unwrap();
    let material = materials.add(PsxSkyMaterial::default());
    commands.spawn((
        Name::new("PSX Sky Dome"),
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material),
        Transform::default(),
        SkyDome,
    ));
}

fn follow_camera_translation(
    q_camera: Query<&GlobalTransform, With<PsxCamera>>,
    mut q_sky: Query<&mut Transform, With<SkyDome>>,
) {
    let Ok(camera_transform) = q_camera.single() else {
        return;
    };
    let camera_pos = camera_transform.translation();
    for mut transform in &mut q_sky {
        transform.translation = camera_pos;
    }
}

fn linear_color(r: f32, g: f32, b: f32) -> Vec4 {
    let c = LinearRgba::from(Color::srgb(r, g, b));
    Vec4::new(c.red, c.green, c.blue, c.alpha)
}
