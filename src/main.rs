#![feature(str_as_str)]
// Support configuring Bevy lints within code.
#![cfg_attr(bevy_lint, feature(register_tool), register_tool(bevy))]
// Disable console on Windows for non-dev builds.
#![cfg_attr(not(feature = "dev"), windows_subsystem = "windows")]

mod assets;
mod audio;
mod camera;
pub mod debug;
mod gameplay;
mod input;
mod map;
mod psx;
mod ratspinner;
mod settings;
pub(crate) mod ui;
mod voice;

use avian3d::prelude::{CollisionLayers, LayerMask};
use bevy::{
    asset::AssetMetaCheck,
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    gltf::{GltfPlugin, convert_coordinates::GltfConvertCoordinates},
    image::ImagePlugin,
    prelude::*,
};
use bevy_trenchbroom::{config::DefaultFaceAttributes, prelude::*};

use crate::gameplay::PhysLayer;

fn main() -> AppExit {
    if should_skip_graphics_boot() {
        eprintln!("non-interactive environment detected; skipping graphical startup.");
        return AppExit::Success;
    }

    App::new().add_plugins(AppPlugin).run()
}

fn should_skip_graphics_boot() -> bool {
    #[cfg(all(feature = "native", target_os = "linux"))]
    {
        use std::io::IsTerminal;
        std::env::var_os("FEVERISH_FORCE_GRAPHICS").is_none()
            && !std::io::stdout().is_terminal()
            && !std::io::stderr().is_terminal()
    }

    #[cfg(not(all(feature = "native", target_os = "linux")))]
    {
        false
    }
}

pub struct AppPlugin;

#[cfg(feature = "native")]
fn native_seedling_plugin() -> bevy_seedling::SeedlingPlugin<bevy_seedling::prelude::CpalBackend> {
    let mut plugin = bevy_seedling::SeedlingPlugin::default();
    // give alsa more breathing room so underruns chill out
    plugin.stream_config.output.desired_block_frames = Some(4096);
    plugin.stream_config.output.desired_sample_rate = None;
    plugin.stream_config.output.fallback = false;
    plugin
}

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
                        #[cfg(feature = "web")]
                        prevent_default_event_handling: true,
                        ..default()
                    }
                    .into(),
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
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
            #[cfg(feature = "native")]
            native_seedling_plugin(),
            #[cfg(feature = "web")]
            bevy_seedling::SeedlingPlugin::new_web_audio(),
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
            // avian3d::debug_render::PhysicsDebugPlugin,
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
            .add_sub_state::<GameState>()
            .add_sub_state::<Phase>();
        app.configure_sets(Update, PausableSystems.run_if(in_state(Paused(false))));

        // Set up game plugins
        app.add_plugins((
            settings::SettingsPlugin,
            psx::PsxPlugin,
            assets::AssetsPlugin,
            camera::CameraPlugin,
            map::MapPlugin,
            // debug, needs to be after SkyPlugin initializes messages
            #[cfg(feature = "dev")]
            (debug::sky::SkyDebugPlugin, debug::ending::EndingDebugPlugin),
            gameplay::GameplayPlugin,
            input::InputPlugin,
            audio::AudioPlugin,
            // our ui :3
            ui::UiPlugin,
            // voice plugin duh
            voice::VoicePlugin,
            // we might kill ratspinner
            ratspinner::RatSpinnerPlugin,
        ))
        .add_systems(OnEnter(AppState::Main), spawn_default_main_menu);
    }
}

fn spawn_default_main_menu(
    mut commands: Commands,
    drivers: Query<(Entity, &Name, Has<ui::MainMenuUi>)>,
) {
    for (entity, name, has_main_menu) in &drivers {
        if name.as_str() != "Main Menu Driver" {
            continue;
        }
        if has_main_menu {
            return;
        }
        commands.entity(entity).insert(ui::MainMenuUi);
        return;
    }
    commands.spawn((Name::new("Main Menu Driver"), ui::MainMenuUi));
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
    Main,
}

/// The in-game state
#[derive(SubStates, Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
#[source(AppState = AppState::Main)]
enum GameState {
    #[default]
    Main,
    Prepare,
}

/// The in-game state
#[derive(SubStates, Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
#[source(GameState = GameState::Main)]
pub(crate) enum Phase {
    /// Free explore before entering the game area
    #[default]
    Explore,
    /// Main game phase
    Main,
    /// Win state
    #[allow(dead_code)]
    Win,
    /// Loss state
    #[allow(dead_code)]
    Lose,
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
