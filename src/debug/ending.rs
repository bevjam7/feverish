use bevy::prelude::*;

use crate::ui::{
    DialogueUiRoot, EndingUiRoot, InventoryUiRoot, MainMenuUi, PauseMenuUi, UiEndingCatalog,
    UiEndingCommandsExt,
};

pub struct EndingDebugPlugin;

impl Plugin for EndingDebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EndingDebugState>()
            .add_systems(Update, handle_ending_debug_input);
    }
}

#[derive(Resource, Default)]
struct EndingDebugState {
    cycle_index: usize,
}

fn handle_ending_debug_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<EndingDebugState>,
    catalog: Res<UiEndingCatalog>,
    mut commands: Commands,
    ending_open: Query<(), With<EndingUiRoot>>,
    main_menu: Query<(), With<MainMenuUi>>,
    pause_menu: Query<(), With<PauseMenuUi>>,
    dialogue_ui: Query<(), With<DialogueUiRoot>>,
    inventory_ui: Query<(), With<InventoryUiRoot>>,
) {
    if !keys.just_pressed(KeyCode::KeyP) {
        return;
    }

    if !main_menu.is_empty()
        || !pause_menu.is_empty()
        || !dialogue_ui.is_empty()
        || !inventory_ui.is_empty()
    {
        return;
    }

    if !ending_open.is_empty() {
        commands.close_ending();
        info!("[DEBUG] ending ui closed");
        return;
    }

    let ids = catalog.debug_ids();
    if ids.is_empty() {
        warn!("[DEBUG] ending catalog is empty; nothing to show");
        return;
    }
    let idx = state.cycle_index % ids.len();
    let selected = ids[idx].clone();
    state.cycle_index = state.cycle_index.wrapping_add(1);

    commands.show_ending(selected.clone());
    info!("[DEBUG] ending ui shown: {selected}");
}
