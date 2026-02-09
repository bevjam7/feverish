use bevy::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
use std::{fs, path::Path};

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        let initial = load_persisted_settings().unwrap_or_else(|error| {
            warn!("{error}");
            GameSettings::default()
        });

        app.insert_resource(initial)
            .add_systems(Update, save_settings_on_change);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingKey {
    MasterVolume,
    MusicVolume,
    UiSfxVolume,
    WorldSfxVolume,
    VoiceVolume,
    DialogueSpeed,
    UiScaleMode,
    UiScale,
    CursorMotion,
}

impl SettingKey {
    pub const ALL: [Self; 9] = [
        Self::MasterVolume,
        Self::MusicVolume,
        Self::UiSfxVolume,
        Self::WorldSfxVolume,
        Self::VoiceVolume,
        Self::DialogueSpeed,
        Self::UiScaleMode,
        Self::UiScale,
        Self::CursorMotion,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::MasterVolume => "master volume",
            Self::MusicVolume => "music volume",
            Self::UiSfxVolume => "ui sfx volume",
            Self::WorldSfxVolume => "world sfx volume",
            Self::VoiceVolume => "voice volume",
            Self::DialogueSpeed => "dialogue speed",
            Self::UiScaleMode => "ui scale mode",
            Self::UiScale => "manual ui scale",
            Self::CursorMotion => "cursor motion",
        }
    }
}

#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GameSettings {
    pub master_volume: f32,
    pub music_volume: f32,
    pub ui_sfx_volume: f32,
    pub world_sfx_volume: f32,
    pub voice_volume: f32,
    pub dialogue_speed: f32,
    pub ui_scale_auto: bool,
    pub manual_ui_scale: f32,
    pub cursor_motion: bool,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            music_volume: 0.85,
            ui_sfx_volume: 0.9,
            world_sfx_volume: 0.95,
            voice_volume: 1.0,
            dialogue_speed: 1.0,
            ui_scale_auto: true,
            manual_ui_scale: 1.0,
            cursor_motion: true,
        }
    }
}

impl GameSettings {
    pub fn adjust(&mut self, key: SettingKey, direction: i32) {
        let step = direction.signum() as f32;
        if step == 0.0 {
            return;
        }

        match key {
            SettingKey::MasterVolume => {
                self.master_volume = (self.master_volume + step * 0.05).clamp(0.0, 1.5);
            }
            SettingKey::MusicVolume => {
                self.music_volume = (self.music_volume + step * 0.05).clamp(0.0, 1.5);
            }
            SettingKey::UiSfxVolume => {
                self.ui_sfx_volume = (self.ui_sfx_volume + step * 0.05).clamp(0.0, 1.5);
            }
            SettingKey::WorldSfxVolume => {
                self.world_sfx_volume = (self.world_sfx_volume + step * 0.05).clamp(0.0, 1.5);
            }
            SettingKey::VoiceVolume => {
                self.voice_volume = (self.voice_volume + step * 0.05).clamp(0.0, 1.5);
            }
            SettingKey::DialogueSpeed => {
                self.dialogue_speed = (self.dialogue_speed + step * 0.1).clamp(0.5, 2.0);
            }
            SettingKey::UiScaleMode => {
                self.ui_scale_auto = !self.ui_scale_auto;
            }
            SettingKey::UiScale => {
                self.manual_ui_scale = (self.manual_ui_scale + step * 0.05).clamp(0.6, 2.0);
            }
            SettingKey::CursorMotion => {
                self.cursor_motion = !self.cursor_motion;
            }
        }
    }

    pub fn value_text(&self, key: SettingKey) -> String {
        match key {
            SettingKey::MasterVolume => percent_text(self.master_volume),
            SettingKey::MusicVolume => percent_text(self.music_volume),
            SettingKey::UiSfxVolume => percent_text(self.ui_sfx_volume),
            SettingKey::WorldSfxVolume => percent_text(self.world_sfx_volume),
            SettingKey::VoiceVolume => percent_text(self.voice_volume),
            SettingKey::DialogueSpeed => percent_text(self.dialogue_speed),
            SettingKey::UiScaleMode =>
                if self.ui_scale_auto {
                    "auto".to_string()
                } else {
                    "manual".to_string()
                },
            SettingKey::UiScale => format!("{:.0}%", self.manual_ui_scale * 100.0),
            SettingKey::CursorMotion =>
                if self.cursor_motion {
                    "on".to_string()
                } else {
                    "off".to_string()
                },
        }
    }
}

fn percent_text(value: f32) -> String {
    format!("{:.0}%", value * 100.0)
}

const SETTINGS_SAVE_PATH: &str = "saves/settings.ron";
#[cfg(target_arch = "wasm32")]
const SETTINGS_STORAGE_KEY: &str = "feverish.settings";

fn load_persisted_settings() -> Result<GameSettings, String> {
    let Some(content) = read_settings_source()? else {
        return Ok(GameSettings::default());
    };

    ron::from_str::<GameSettings>(&content).map_err(|error| {
        format!(
            "failed to parse '{}' as settings RON: {}",
            settings_location_hint(),
            error
        )
    })
}

fn save_settings_on_change(settings: Res<GameSettings>) {
    if !settings.is_changed() {
        return;
    }

    let pretty = ron::ser::PrettyConfig::new();
    let Ok(content) = ron::ser::to_string_pretty(&*settings, pretty) else {
        warn!("failed to serialize settings to RON");
        return;
    };

    if let Err(error) = write_settings_source(&content) {
        warn!("{error}");
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_settings_source() -> Result<Option<String>, String> {
    let path = Path::new(SETTINGS_SAVE_PATH);
    if !path.exists() {
        return Ok(None);
    }

    fs::read_to_string(path)
        .map(Some)
        .map_err(|error| format!("failed to read '{}': {}", path.display(), error))
}

#[cfg(target_arch = "wasm32")]
fn read_settings_source() -> Result<Option<String>, String> {
    let Some(window) = web_sys::window() else {
        return Ok(None);
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return Ok(None);
    };

    storage
        .get_item(SETTINGS_STORAGE_KEY)
        .map_err(|error| format!("failed to read settings from localStorage: {error:?}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn write_settings_source(content: &str) -> Result<(), String> {
    let path = Path::new(SETTINGS_SAVE_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create settings directory '{}': {}",
                parent.display(),
                error
            )
        })?;
    }

    fs::write(path, content).map_err(|error| {
        format!(
            "failed to write settings file '{}': {}",
            path.display(),
            error
        )
    })
}

#[cfg(target_arch = "wasm32")]
fn write_settings_source(content: &str) -> Result<(), String> {
    let Some(window) = web_sys::window() else {
        return Err("failed to access browser window for settings persistence".to_string());
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return Err("failed to access browser localStorage for settings persistence".to_string());
    };

    storage
        .set_item(SETTINGS_STORAGE_KEY, content)
        .map_err(|error| format!("failed to write settings to localStorage: {error:?}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn settings_location_hint() -> &'static str {
    SETTINGS_SAVE_PATH
}

#[cfg(target_arch = "wasm32")]
fn settings_location_hint() -> &'static str {
    SETTINGS_STORAGE_KEY
}
