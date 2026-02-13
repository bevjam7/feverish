use bevy::{
    camera::CameraOutputMode,
    core_pipeline::tonemapping::Tonemapping,
    pbr::{DistanceFog, FogFalloff},
    prelude::*,
    render::{
        alpha::AlphaMode,
        render_resource::{BlendState, Face},
    },
};
#[cfg(not(target_arch = "wasm32"))]
use bevy::{post_process::auto_exposure::AutoExposure, render::view::Hdr};

fn default_outdoor_fog() -> DistanceFog {
    DistanceFog {
        color: Color::srgba(0.09, 0.13, 0.18, 0.90),
        directional_light_color: Color::NONE,
        directional_light_exponent: 32.0,
        falloff: FogFalloff::Linear {
            start: 16.0,
            end: 110.0,
        },
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn player_camera_bundle() -> impl Bundle {
    (
        Name::new("3D Camera"),
        Camera {
            order: CameraOrder::World.into(),
            ..default()
        },
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 1.309, // ~75 degrees
            near: 0.2,
            far: 600.0,
            aspect_ratio: 16.0 / 9.0,
            ..default()
        }),
        (
            Msaa::Off,
            // bevy::anti_alias::taa::TemporalAntiAliasing::default(),
            // bevy::light::ShadowFilteringMethod::Temporal,
            bevy::core_pipeline::prepass::DeferredPrepass,
        ),
        default_outdoor_fog(),
        AutoExposure::default(),
        Tonemapping::TonyMcMapface,
        Hdr,
    )
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn player_camera_bundle() -> impl Bundle {
    (
        Name::new("3D Camera"),
        Camera {
            order: CameraOrder::World.into(),
            ..default()
        },
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 1.309, // ~75 degrees
            near: 0.2,
            far: 600.0,
            aspect_ratio: 16.0 / 9.0,
            ..default()
        }),
        Msaa::Off,
        default_outdoor_fog(),
        Tonemapping::None,
    )
}

pub(crate) struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_ui_camera)
            .add_systems(Update, force_opaque_material_depth)
            .add_observer(make_hdr_compatible);
    }
}

/// the order of camera drawing, where the last in the list is the last drawn
/// (and on top)
enum CameraOrder {
    World,
    Ui,
}

impl From<CameraOrder> for isize {
    fn from(order: CameraOrder) -> Self {
        order as isize
    }
}

fn spawn_ui_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("UI Camera"),
        Camera2d,
        IsDefaultUiCamera,
        Camera {
            // keep UI above the PSX presentation camera, this was driving me crazy lol
            order: isize::from(CameraOrder::Ui) + 1,
            ..default()
        },
    ));
}

fn force_opaque_material_depth(mut materials: ResMut<Assets<StandardMaterial>>) {
    for (_, material) in materials.iter_mut() {
        if !matches!(material.alpha_mode, AlphaMode::Blend) {
            continue;
        }
        if material.base_color.alpha() <= 0.99 {
            continue;
        }
        material.alpha_mode = AlphaMode::Opaque;
        material.double_sided = false;
        material.cull_mode = Some(Face::Back);
    }
}

fn make_hdr_compatible(
    add: On<Add, Camera>,
    mut cameras: Query<&mut Camera>,
    mut commands: Commands,
) {
    let entity = add.entity;
    let mut camera = cameras.get_mut(entity).unwrap();
    if camera.order == isize::from(CameraOrder::World) {
        // Use the world model camera to determine tonemapping.
        return;
    }
    // Needed because of https://github.com/bevyengine/bevy/issues/18902
    commands.entity(entity).insert(Tonemapping::None);
    // Needed because of https://github.com/bevyengine/bevy/issues/18901
    // and https://github.com/bevyengine/bevy/issues/18903
    camera.clear_color = ClearColorConfig::Custom(Color::NONE);
    camera.output_mode = CameraOutputMode::Write {
        blend_state: Some(BlendState::ALPHA_BLENDING),
        clear_color: ClearColorConfig::None,
    };
}
