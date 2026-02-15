// ui plugin ddefinition and reexports

pub(super) mod components;
pub(super) mod confirm_popup;
pub(crate) mod dialogue;
pub mod discovery_api;
pub(super) mod ending;
pub(super) mod fx;
pub(super) mod hint;
pub(super) mod inventory;
pub(super) mod main_menu;
pub(super) mod pause_menu;
pub(super) mod systems;
pub(super) mod theme;

use bevy::prelude::*;
#[allow(unused_imports)]
pub use components::{
    DialogueUiRoot, DiscoveryEntry, DiscoveryInteraction, DiscoveryInteractionAction,
    DiscoveryInteractionActor, DiscoveryInteractionRecord, DiscoveryKind, InventoryUiRoot,
    MainMenuUi, PauseMenuUi, SpawnDroppedItem, UiDialogueCommand, UiDialogueMode, UiDialogueOption,
    UiDialoguePreview, UiDialogueRequest, UiDiscoveryCommand, UiDiscoveryDbSnapshot, UiMenuAction,
};
#[allow(unused_imports)]
pub use discovery_api::DiscoveryCommandsExt;
#[allow(unused_imports)]
pub use ending::{
    EndingUiRoot, UiEndingCatalog, UiEndingCommand, UiEndingCommandsExt, UiEndingPayload,
};
#[allow(unused_imports)]
pub use hint::{UiHintCommand, UiHintCommandsExt, UiHintRequest};
pub use systems::UiDiscoveryDb;
use systems::{
    animate_dither_pixels, animate_main_menu_ticker, apply_discovery_commands,
    cleanup_removed_main_menu, cleanup_removed_pause_menu, cleanup_ui_cursor,
    emulate_button_interaction_for_offscreen_ui, ensure_gallery_selection_exists,
    force_linear_font_atlas_sampling, handle_button_interactions, handle_menu_actions,
    handle_pause_shortcut, on_ui_scroll, populate_ui_fonts_and_cursor, rebuild_gallery_lists,
    refresh_button_highlights, refresh_confirm_dialogs, refresh_gallery_details,
    refresh_main_menu_content, refresh_main_menu_panels, refresh_pause_menu_panels,
    refresh_settings_values, reset_ticker_on_scale_change, restore_native_cursor_on_exit,
    send_scroll_events, spawn_main_menu_on_added, spawn_pause_menu_on_added, update_ui_cursor,
    update_ui_scale,
};

use crate::AppState;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<systems::UiRegistry>()
            .init_resource::<UiDiscoveryDb>()
            .init_resource::<dialogue::UiDialogueRuntime>()
            .init_resource::<dialogue::UiDialogueState>()
            .init_resource::<ending::UiEndingCatalog>()
            .init_resource::<ending::UiEndingRuntime>()
            .init_resource::<hint::UiHintRuntime>()
            .init_resource::<inventory::UiInventoryRuntime>()
            .add_plugins(fx::UiMenuFxPlugin)
            .add_message::<UiMenuAction>()
            .add_message::<UiDialogueCommand>()
            .add_message::<ending::UiEndingCommand>()
            .add_message::<hint::UiHintCommand>()
            .add_message::<inventory::UiInventoryCommand>()
            .add_message::<UiDiscoveryCommand>()
            .add_observer(on_ui_scroll)
            .add_systems(
                OnExit(AppState::Load),
                (
                    populate_ui_fonts_and_cursor,
                    ending::populate_default_endings,
                ),
            )
            .add_systems(
                Update,
                emulate_button_interaction_for_offscreen_ui
                    .before(handle_button_interactions)
                    .before(dialogue::handle_dialogue_arrow_buttons)
                    .before(dialogue::handle_dialogue_quick_action_buttons)
                    .before(dialogue::handle_dialogue_confirm_button),
            )
            .add_systems(
                Update,
                (
                    apply_discovery_commands,
                    ending::apply_ending_commands,
                    hint::apply_hint_commands,
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
                )
                    .run_if(in_state(AppState::Main)),
            )
            .add_systems(Update, send_scroll_events)
            .add_systems(
                Update,
                (
                    refresh_confirm_dialogs,
                    animate_dither_pixels,
                    force_linear_font_atlas_sampling,
                    update_ui_scale,
                    reset_ticker_on_scale_change,
                    animate_main_menu_ticker,
                    update_ui_cursor,
                    cleanup_ui_cursor,
                    restore_native_cursor_on_exit,
                    dialogue::apply_dialogue_commands,
                    hint::animate_hint_glitch,
                    hint::animate_hint_fade,
                    inventory::apply_inventory_commands,
                    (
                        dialogue::update_typewriter_dialogue,
                        dialogue::sync_picker_preview_from_selection,
                        dialogue::sync_and_frame_dialogue_preview,
                        dialogue::rotate_dialogue_preview,
                        dialogue::advance_dialogue_with_mouse,
                        dialogue::handle_dialogue_shortcuts,
                        dialogue::update_dialogue_text_scroll_hint,
                        dialogue::handle_dialogue_arrow_buttons,
                        dialogue::handle_dialogue_quick_action_buttons,
                        dialogue::handle_dialogue_confirm_button,
                        dialogue::animate_option_slot_transition,
                        dialogue::animate_dialogue_glyphs,
                    ),
                    inventory::handle_inventory_tab_shortcut,
                    inventory::handle_inventory_shortcuts,
                    inventory::handle_inventory_item_interactions,
                    inventory::refresh_inventory_on_db_change,
                    inventory::close_inventory_when_blocked,
                    (
                        inventory::sync_inventory_preview_from_selection,
                        inventory::sync_and_frame_inventory_preview,
                        inventory::rotate_inventory_preview,
                        inventory::handle_inventory_preview_zoom,
                    ),
                )
                    .chain()
                    .run_if(in_state(AppState::Main)),
            )
            .add_systems(
                Update,
                (
                    ending::update_ending_reveal,
                    ending::update_ending_hold_to_continue,
                )
                    .run_if(in_state(AppState::Main)),
            );
    }
}
