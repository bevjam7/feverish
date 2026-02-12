use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use bevy_seedling::prelude::AudioSample;
use serde::{Deserialize, Serialize};

use crate::{AppState, ratspinner::RatScriptAsset};

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_loading_state(
            LoadingState::new(AppState::Load)
                .continue_to_state(AppState::Main)
                .with_dynamic_assets_file::<StandardDynamicAssetCollection>("default.assets.ron")
                .load_collection::<GameAssets>(),
        )
        .add_plugins(RonAssetPlugin::<ItemMeta>::new(&["item.meta"]));
    }
}

#[allow(dead_code)]
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
    #[asset(key = "items", collection(typed))]
    pub items: Vec<Handle<ItemMeta>>,
    #[asset(key = "ratspinner.scripts", collection(typed))]
    pub rat_scripts: Vec<Handle<RatScriptAsset>>,
    #[asset(key = "font.pixel")]
    pub font_pixel: Handle<Font>,
    #[asset(key = "font.body")]
    pub font_body: Handle<Font>,
    #[asset(key = "image.cursor")]
    pub image_cursor: Handle<Image>,
    #[asset(key = "image.cursor-closed")]
    pub image_cursor_closed: Handle<Image>,
}

#[derive(Asset, Reflect, Clone, Serialize, Deserialize)]
pub(crate) struct ItemMeta {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) subtitle: String,
    pub(crate) description: String,
}
