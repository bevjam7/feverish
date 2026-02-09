//! Map lifecycle plugin and Trenchbroom integration

use bevy::{prelude::*, scene::SceneInstanceReady};

use crate::{
    GameState,
    assets::GameAssets,
    gameplay::{Player, PlayerRoot},
};

pub(crate) struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LevelToPrepare>()
            .add_systems(OnEnter(GameState::Prepare), (reset, spawn_map).chain());
    }
}

fn reset(mut cmd: Commands, players: Query<Entity, Or<(With<Player>, With<PlayerRoot>)>>) {
    for player in players {
        cmd.entity(player).despawn();
    }
}

fn spawn_map(
    mut cmd: Commands,
    assets: Option<Res<GameAssets>>,
    asset_server: Res<AssetServer>,
    level_to_prepare: Res<LevelToPrepare>,
    existing_level: Option<Single<Entity, With<Level>>>,
) {
    if let Some(existing_level) = existing_level {
        cmd.entity(existing_level.entity()).despawn();
    }
    let level = level_to_prepare
        .level
        .clone()
        .or_else(|| assets.as_ref().map(|loaded| loaded.level_exterior.clone()))
        .unwrap_or_else(|| {
            warn!("GameAssets missing during map spawn; loading fallback scene path directly.");
            asset_server.load("maps/exterior.map#Scene")
        });
    cmd.spawn((SceneRoot(level), Level))
        .observe(move_and_transition_after_spawned);
}

fn move_and_transition_after_spawned(_on: On<SceneInstanceReady>, mut cmd: Commands) {
    // Transition to the main game state
    cmd.set_state(GameState::Main);
}

#[derive(Resource, Default)]
pub(crate) struct LevelToPrepare {
    pub(crate) level: Option<Handle<Scene>>,
    /// If set, move the spawned player to the portal target immediately after
    /// loading
    pub(crate) portal_target: Option<String>,
}

#[derive(Component)]
pub(crate) struct Level;
