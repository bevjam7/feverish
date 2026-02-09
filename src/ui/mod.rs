// ui plugin ddefinition and reexports

pub(super) mod components;
pub(super) mod confirm_popup;
pub mod discovery_api;
pub(super) mod main_menu;
pub(super) mod pause_menu;
pub(super) mod systems;
pub(super) mod theme;

use bevy::prelude::*;
#[allow(unused_imports)]
pub use components::{
    DiscoveryEntry, DiscoveryKind, MainMenuUi, PauseMenuUi, UiDiscoveryCommand, UiMenuAction,
};
#[allow(unused_imports)]
pub use discovery_api::DiscoveryCommandsExt;
pub use systems::UiDiscoveryDb;
use systems::{
    animate_dither_pixels, animate_main_menu_ticker, apply_discovery_commands,
    cleanup_removed_main_menu, cleanup_removed_pause_menu, cleanup_ui_cursor,
    ensure_gallery_selection_exists, handle_button_interactions, handle_menu_actions,
    handle_pause_shortcut, load_fonts, rebuild_gallery_lists, refresh_button_highlights,
    refresh_confirm_dialogs, refresh_gallery_details, refresh_main_menu_content,
    refresh_main_menu_panels, reset_ticker_on_scale_change, restore_native_cursor_on_exit,
    spawn_main_menu_on_added, spawn_pause_menu_on_added, update_ui_cursor, update_ui_scale,
};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<systems::UiRegistry>()
            .init_resource::<UiDiscoveryDb>()
            .add_message::<UiMenuAction>()
            .add_message::<UiDiscoveryCommand>()
            .add_systems(Startup, load_fonts)
            .add_systems(
                Update,
                (
                    apply_discovery_commands,
                    spawn_main_menu_on_added,
                    spawn_pause_menu_on_added,
                    cleanup_removed_main_menu,
                    cleanup_removed_pause_menu,
                    handle_pause_shortcut,
                    handle_button_interactions,
                    handle_menu_actions,
                    ensure_gallery_selection_exists,
                    refresh_main_menu_content,
                    refresh_main_menu_panels,
                    rebuild_gallery_lists,
                    refresh_gallery_details,
                    refresh_button_highlights,
                ),
            )
            .add_systems(
                Update,
                (
                    refresh_confirm_dialogs,
                    animate_dither_pixels,
                    update_ui_scale,
                    reset_ticker_on_scale_change,
                    animate_main_menu_ticker,
                    update_ui_cursor,
                    cleanup_ui_cursor,
                    restore_native_cursor_on_exit,
                ),
            );
    }
}
