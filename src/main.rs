// Support configuring Bevy lints within code.
#![cfg_attr(bevy_lint, feature(register_tool), register_tool(bevy))]
// Disable console on Windows for non-dev builds.
#![cfg_attr(not(feature = "dev"), windows_subsystem = "windows")]

mod assets;
mod camera;
mod gameplay;
mod input;
mod map;
mod npc;
mod props;

use avian3d::prelude::{CollisionLayers, LayerMask};
use bevy::{
    asset::AssetMetaCheck,
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    gltf::{GltfPlugin, convert_coordinates::GltfConvertCoordinates},
    prelude::*,
};
use bevy_trenchbroom::{config::DefaultFaceAttributes, prelude::*};

use crate::gameplay::PhysLayer;

fn main() -> AppExit {
    App::new().add_plugins(AppPlugin).run()
}

pub struct AppPlugin;

impl Plugin for AppPlugin {
    fn build(&self, app: &mut App) {
        // Add Bevy plugins.
        app.add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    // Wasm builds will check for meta files (that don't exist) if this isn't set.
                    // This causes errors and even panics on web build on itch.
                    // See https://github.com/bevyengine/bevy_github_ci_template/issues/48.
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Window {
                        title: "Feverish".to_string(),
                        fit_canvas_to_parent: true,
                        ..default()
                    }
                    .into(),
                    ..default()
                })
                .set(GltfPlugin {
                    convert_coordinates: GltfConvertCoordinates {
                        rotate_scene_entity: true,
                        rotate_meshes: true,
                    },
                    ..default()
                }),
        );

        // Add 3rd party plugins
        app.add_plugins((
            avian3d::PhysicsPlugins::default(),
            (
                TrenchBroomPlugins(
                    TrenchBroomConfig::new("feverish")
                        // .linear_filtering()
                        .default_face_attributes(DefaultFaceAttributes {
                            scale: Some(Vec2::splat(0.5)), // Suitable for 256x256 textures
                            ..default()
                        })
                        .default_solid_scene_hooks(|| {
                            SceneHooks::new()
                                .smooth_by_default_angle()
                                .convex_collider()
                        }),
                )
                .build(),
                TrenchBroomPhysicsPlugin::new(bevy_trenchbroom_avian::AvianPhysicsBackend),
            ),
            avian3d::debug_render::PhysicsDebugPlugin,
            bevy_enhanced_input::EnhancedInputPlugin,
            bevy_ahoy::AhoyPlugins::default(),
        ));

        // Order new `AppSystems` variants by adding them here:
        app.configure_sets(
            Update,
            (
                AppSystems::TickTimers,
                AppSystems::RecordInput,
                AppSystems::Update,
            )
                .chain(),
        );

        // Set up states
        app.init_state::<Paused>()
            .init_state::<AppState>()
            .add_sub_state::<GameState>();
        app.configure_sets(Update, PausableSystems.run_if(in_state(Paused(false))));

        // Set up game plugins
        app.add_plugins((
            assets::AssetsPlugin,
            camera::CameraPlugin,
            map::MapPlugin,
            gameplay::GameplayPlugin,
            input::InputPlugin,
        ));
    }
}

/// High-level groupings of systems for the app in the `Update` schedule.
/// When adding a new variant, make sure to order it in the `configure_sets`
/// call above.
#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash, PartialOrd, Ord)]
enum AppSystems {
    /// Tick timers.
    TickTimers,
    /// Record player input.
    RecordInput,
    /// Do everything else (consider splitting this into further variants).
    Update,
}

/// The overarching app lifecycle state.
#[derive(States, Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
enum AppState {
    #[default]
    Load,
    Game,
}

/// The in-game state
#[derive(SubStates, Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
#[source(AppState = AppState::Game)]
enum GameState {
    #[default]
    Prepare,
    Main,
}

/// A system set for systems that shouldn't run while the game is paused.
#[derive(SystemSet, Copy, Clone, Eq, PartialEq, Hash, Debug)]
struct PausableSystems;

/// Whether or not the game is paused.
#[derive(States, Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
struct Paused(pub bool);

#[derive(Component, Reflect, Default)]
#[component(on_add=Self::on_add_hook)]
#[require(Pickable)]
#[reflect(Component)]
pub struct Usable;

impl Usable {
    fn on_add_hook(mut world: DeferredWorld, hook: HookContext) {
        world
            .commands()
            .entity(hook.entity)
            .insert(CollisionLayers::new(
                [PhysLayer::Default, PhysLayer::Usable],
                LayerMask::NONE,
            ));
    }
}
