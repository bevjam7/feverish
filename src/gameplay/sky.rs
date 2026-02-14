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
const SKY_COLOR_TRANSITION_SECS: f32 = 1.35;
const FLAG_ORION_BELT: u32 = 1;
const FLAG_PROC_A: u32 = 2;
const FLAG_SCORPIUS: u32 = 4;
const FLAG_CYGNUS: u32 = 8;
const FLAG_URSA_MAJOR: u32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarcusType {
    Panicking,
    Leaning,
    Lounging,
    Lonely,
}

impl MarcusType {
    pub fn flag(&self) -> u32 {
        match self {
            MarcusType::Panicking => FLAG_SCORPIUS,
            MarcusType::Leaning => FLAG_CYGNUS,
            MarcusType::Lounging => FLAG_ORION_BELT,
            MarcusType::Lonely => FLAG_URSA_MAJOR,
        }
    }

    pub fn default_sky_color(&self) -> (Vec4, Vec4) {
        match self {
            MarcusType::Panicking => (
                linear_color(0.08, 0.02, 0.15),
                linear_color(0.18, 0.05, 0.25),
            ),
            MarcusType::Leaning => (
                linear_color(0.02, 0.05, 0.18),
                linear_color(0.05, 0.12, 0.28),
            ),
            MarcusType::Lounging => (
                linear_color(0.12, 0.02, 0.05),
                linear_color(0.25, 0.08, 0.10),
            ),
            MarcusType::Lonely => (
                linear_color(0.02, 0.10, 0.06),
                linear_color(0.05, 0.20, 0.12),
            ),
        }
    }
}

#[derive(Message)]
pub enum SkyCommand {
    ActivateConstellation(MarcusType),
    DeactivateConstellation(MarcusType),
    SetSkyColor { top: Color, bottom: Color },
    ResetToDefault,
}

pub(super) struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<PsxSkyMaterial>::default())
            .add_message::<SkyCommand>()
            .add_systems(Startup, spawn_sky_dome)
            .add_systems(
                Update,
                (follow_camera_translation, handle_sky_commands).in_set(AppSystems::Update),
            );
    }
}

#[derive(Component)]
struct SkyDome;

#[derive(Debug, Clone, Copy, ShaderType)]
struct SkyUniformData {
    color_top: Vec4,
    color_bottom: Vec4,
    prev_color_top: Vec4,
    prev_color_bottom: Vec4,
    resolution: Vec2,
    seed: f32,
    star_threshold: f32,
    micro_star_threshold: f32,
    flags: u32,
    nebula_strength: f32,
    dither_strength: f32,
    detail_scale: f32,
    horizon_haze_strength: f32,
    color_transition_start: f32,
    color_transition_duration: f32,
}

impl Default for SkyUniformData {
    fn default() -> Self {
        Self {
            color_top: linear_color(0.055, 0.015, 0.120),
            color_bottom: linear_color(0.010, 0.135, 0.205),
            prev_color_top: linear_color(0.055, 0.015, 0.120),
            prev_color_bottom: linear_color(0.010, 0.135, 0.205),
            resolution: SKY_PIXEL_RESOLUTION,
            seed: 917.0,
            star_threshold: 0.968,
            micro_star_threshold: 0.997,
            flags: FLAG_PROC_A,
            nebula_strength: 0.42,
            dither_strength: 0.05,
            detail_scale: 3.6,
            horizon_haze_strength: 0.56,
            color_transition_start: 0.0,
            color_transition_duration: SKY_COLOR_TRANSITION_SECS,
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
    commands.insert_resource(SkyMaterialHandle(material.clone()));
    commands.spawn((
        Name::new("PSX Sky Dome"),
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material),
        Transform::default(),
        SkyDome,
    ));
}

#[derive(Resource)]
struct SkyMaterialHandle(Handle<PsxSkyMaterial>);

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

fn handle_sky_commands(
    mut commands: MessageReader<SkyCommand>,
    mut materials: ResMut<Assets<PsxSkyMaterial>>,
    handle: Res<SkyMaterialHandle>,
    time: Res<Time>,
) {
    let material = match materials.get_mut(&handle.0) {
        Some(m) => m,
        None => return,
    };
    let now = time.elapsed_secs();

    for cmd in commands.read() {
        match cmd {
            SkyCommand::ActivateConstellation(marcus) => {
                material.sky.flags |= marcus.flag();
                let (top, bottom) = marcus.default_sky_color();
                set_sky_target(&mut material.sky, top, bottom, now);
            }
            SkyCommand::DeactivateConstellation(marcus) => {
                material.sky.flags &= !marcus.flag();
            }
            SkyCommand::SetSkyColor { top, bottom } => {
                let top = LinearRgba::from(*top);
                let bottom = LinearRgba::from(*bottom);
                set_sky_target(
                    &mut material.sky,
                    Vec4::new(top.red, top.green, top.blue, top.alpha),
                    Vec4::new(bottom.red, bottom.green, bottom.blue, bottom.alpha),
                    now,
                );
            }
            SkyCommand::ResetToDefault => {
                let (current_top, current_bottom) = current_sky_colors(&material.sky, now);
                material.sky = SkyUniformData::default();
                material.sky.prev_color_top = current_top;
                material.sky.prev_color_bottom = current_bottom;
                material.sky.color_transition_start = now;
                material.sky.color_transition_duration = SKY_COLOR_TRANSITION_SECS;
            }
        }
    }
}

fn set_sky_target(sky: &mut SkyUniformData, top: Vec4, bottom: Vec4, now: f32) {
    let (current_top, current_bottom) = current_sky_colors(sky, now);
    sky.prev_color_top = current_top;
    sky.prev_color_bottom = current_bottom;
    sky.color_top = top;
    sky.color_bottom = bottom;
    sky.color_transition_start = now;
    sky.color_transition_duration = SKY_COLOR_TRANSITION_SECS;
}

fn current_sky_colors(sky: &SkyUniformData, now: f32) -> (Vec4, Vec4) {
    let duration = sky.color_transition_duration.max(0.0001);
    let t = ((now - sky.color_transition_start) / duration).clamp(0.0, 1.0);
    let eased = t * t * (3.0 - 2.0 * t);
    (
        sky.prev_color_top.lerp(sky.color_top, eased),
        sky.prev_color_bottom.lerp(sky.color_bottom, eased),
    )
}
