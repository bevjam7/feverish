//! Map lifecycle plugin and Trenchbroom integration

use bevy::{prelude::*, scene::SceneInstanceReady};
use bevy_trenchbroom::{config::DefaultFaceAttributes, prelude::*};

use crate::{GameState, assets::GameAssets};

pub(crate) struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            TrenchBroomPlugins(
                TrenchBroomConfig::new("feverish")
                    // .linear_filtering()
                    .default_face_attributes(DefaultFaceAttributes {
                        scale: Some(Vec2::splat(0.5)), // Suitable for 256x256 textures
                        ..default()
                    })
                    .default_solid_scene_hooks(|| {
                        SceneHooks::new().smooth_by_default_angle()
                        // .convex_collider()
                    }),
            )
            .build(),
        )
        .add_systems(OnEnter(GameState::Prepare), spawn_map);
    }
}

fn spawn_map(mut cmd: Commands, assets: Res<GameAssets>) {
    cmd.spawn(SceneRoot(assets.levels[0].clone()))
        .observe(transition_after_spawned);
}

fn transition_after_spawned(_: On<SceneInstanceReady>, mut cmd: Commands) {
    cmd.set_state(GameState::Main);
}
