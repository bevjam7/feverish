// ui plugin ddefinition and reexports

pub(super) mod components;
pub(super) mod confirm_popup;
pub(super) mod dialogue;
pub mod discovery_api;
pub(super) mod main_menu;
pub(super) mod pause_menu;
pub(super) mod systems;
pub(super) mod theme;

use bevy::prelude::*;
#[allow(unused_imports)]
pub use components::{
    DialogueUiRoot, DiscoveryEntry, DiscoveryKind, MainMenuUi, PauseMenuUi, UiDialogueCommand,
    UiDialogueOption, UiDialoguePreview, UiDialogueRequest, UiDiscoveryCommand, UiMenuAction,
};
#[allow(unused_imports)]
pub use discovery_api::DiscoveryCommandsExt;
pub use systems::UiDiscoveryDb;
use systems::{
    animate_dither_pixels, animate_main_menu_ticker, apply_discovery_commands,
    cleanup_removed_main_menu, cleanup_removed_pause_menu, cleanup_ui_cursor,
    ensure_gallery_selection_exists, handle_button_interactions, handle_menu_actions,
    handle_pause_shortcut, load_fonts, on_ui_scroll, rebuild_gallery_lists,
    refresh_button_highlights, refresh_confirm_dialogs, refresh_gallery_details,
    refresh_main_menu_content, refresh_main_menu_panels, refresh_pause_menu_panels,
    refresh_settings_values, reset_ticker_on_scale_change, restore_native_cursor_on_exit,
    send_scroll_events, spawn_main_menu_on_added, spawn_pause_menu_on_added, update_ui_cursor,
    update_ui_scale,
};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<systems::UiRegistry>()
            .init_resource::<UiDiscoveryDb>()
            .init_resource::<dialogue::UiDialogueRuntime>()
            .init_resource::<dialogue::UiDialogueState>()
            .add_message::<UiMenuAction>()
            .add_message::<UiDialogueCommand>()
            .add_message::<UiDiscoveryCommand>()
            .add_observer(on_ui_scroll)
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
                    refresh_pause_menu_panels,
                    rebuild_gallery_lists,
                    refresh_gallery_details,
                    refresh_settings_values,
                    refresh_button_highlights,
                ),
            )
            .add_systems(Update, send_scroll_events)
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
                    dialogue::apply_dialogue_commands,
                    dialogue::update_typewriter_dialogue,
                    dialogue::sync_picker_preview_from_selection,
                    dialogue::sync_and_frame_dialogue_preview,
                    dialogue::rotate_dialogue_preview,
                    dialogue::advance_dialogue_with_mouse,
                    dialogue::handle_dialogue_shortcuts,
                    dialogue::handle_dialogue_arrow_buttons,
                    dialogue::handle_dialogue_quick_action_buttons,
                    dialogue::animate_option_slot_transition,
                    dialogue::animate_dialogue_glyphs,
                ),
            );
    }
}
