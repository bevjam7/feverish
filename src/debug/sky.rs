use bevy::prelude::*;

use crate::gameplay::sky::{MarcusType, SkyCommand};

pub struct SkyDebugPlugin;

impl Plugin for SkyDebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_sky_debug_input);
    }
}

fn handle_sky_debug_input(keys: Res<ButtonInput<KeyCode>>, mut commands: Commands) {
    if keys.just_pressed(KeyCode::Digit1) {
        commands.write_message(SkyCommand::ActivateConstellation(MarcusType::Panicking));
        info!("[DEBUG] Activated Panicking Marcus (Scorpius)");
    }
    if keys.just_pressed(KeyCode::Digit2) {
        commands.write_message(SkyCommand::ActivateConstellation(MarcusType::Leaning));
        info!("[DEBUG] Activated Leaning Marcus (Cygnus)");
    }
    if keys.just_pressed(KeyCode::Digit3) {
        commands.write_message(SkyCommand::ActivateConstellation(MarcusType::Lounging));
        info!("[DEBUG] Activated Lounging Marcus (Orion)");
    }
    if keys.just_pressed(KeyCode::Digit4) {
        commands.write_message(SkyCommand::ActivateConstellation(MarcusType::Lonely));
        info!("[DEBUG] Activated Lonely Marcus (Ursa Major)");
    }
    if keys.just_pressed(KeyCode::Digit0) {
        commands.write_message(SkyCommand::ResetToDefault);
        info!("[DEBUG] Reset sky to default");
    }
}
