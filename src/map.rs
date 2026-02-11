//! Map lifecycle plugin and Trenchbroom integration

use bevy::{prelude::*, scene::SceneInstanceReady};

use crate::{
    AppSystems, GameState,
    assets::GameAssets,
    gameplay::{Player, PlayerRoot},
    psx::PsxWorldRoot,
};

pub(crate) struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LevelToPrepare>()
            .init_resource::<PendingLevelTransition>()
            .add_systems(
                Update,
                drive_pending_level_transition
                    .in_set(AppSystems::Update)
                    .run_if(in_state(GameState::Main)),
            )
            .add_systems(OnEnter(GameState::Prepare), (reset, spawn_map).chain());
    }
}

fn drive_pending_level_transition(
    mut pending: ResMut<PendingLevelTransition>,
    mut level_to_prepare: ResMut<LevelToPrepare>,
    asset_server: Res<AssetServer>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Some(level) = pending.level.clone() else {
        return;
    };

    if !asset_server.is_loaded_with_dependencies(level.id()) {
        return;
    }

    level_to_prepare.level = Some(level);
    level_to_prepare.portal_target = pending.portal_target.take();
    pending.level = None;
    next_state.set(GameState::Prepare);
}

fn reset(mut cmd: Commands, players: Query<Entity, Or<(With<Player>, With<PlayerRoot>)>>) {
    for player in players {
        cmd.entity(player).despawn();
    }
}

fn spawn_map(
    mut cmd: Commands,
    asset_server: Res<AssetServer>,
    assets: Option<Res<GameAssets>>,
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
    cmd.spawn((SceneRoot(level), Level, PsxWorldRoot))
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

#[derive(Resource, Default)]
pub(crate) struct PendingLevelTransition {
    pub(crate) level: Option<Handle<Scene>>,
    pub(crate) portal_target: Option<String>,
}

#[derive(Component)]
pub(crate) struct Level;
