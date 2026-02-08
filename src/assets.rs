use bevy::prelude::*;
use bevy_asset_loader::prelude::*;

use crate::AppState;

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>().add_loading_state(
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
}
