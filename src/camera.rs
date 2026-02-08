use bevy::{
    camera::CameraOutputMode,
    core_pipeline::tonemapping::Tonemapping,
    post_process::auto_exposure::AutoExposure,
    prelude::*,
    render::{render_resource::BlendState, view::Hdr},
};

pub(crate) struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_ui_camera)
            .add_observer(make_hdr_compatible);
    }
}

/// The order of camera drawing, where the last in the list is the last drawn
/// (and on top).
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
            order: CameraOrder::Ui.into(),
            ..default()
        },
    ));
}

pub(crate) fn player_camera_bundle() -> impl Bundle {
    (
        Name::new("3D Camera"),
        Camera {
            order: CameraOrder::World.into(),
            ..default()
        },
        Camera3d::default(),
        (
            Msaa::Off,
            bevy::anti_alias::taa::TemporalAntiAliasing::default(),
            bevy::light::ShadowFilteringMethod::Temporal,
            // bevy::core_pipeline::prepass::DeferredPrepass,
        ),
        AutoExposure::default(),
        Tonemapping::TonyMcMapface,
        Hdr,
    )
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
