use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_seedling::prelude::AudioSample;

use crate::{AppState, ratspinner::RatScriptAsset};

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_loading_state(
            LoadingState::new(AppState::Load)
                .continue_to_state(AppState::Game)
                .with_dynamic_assets_file::<StandardDynamicAssetCollection>("default.assets.ron")
                .load_collection::<GameAssets>(),
        );
    }
}

#[derive(AssetCollection, Resource)]
pub struct GameAssets {
    #[asset(key = "levels.hallway")]
    pub level_hallway: Handle<Scene>,
    #[asset(key = "levels.exterior")]
    pub level_exterior: Handle<Scene>,
    #[asset(key = "models", collection(typed))]
    pub models: Vec<Handle<Gltf>>,
    #[asset(key = "audio", collection(typed))]
    pub audio: Vec<Handle<AudioSample>>,
    #[asset(key = "ratspinner.scripts", collection(typed))]
    pub rat_scripts: Vec<Handle<RatScriptAsset>>,
}
