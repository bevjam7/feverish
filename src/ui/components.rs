use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::settings::SettingKey;

#[derive(Component, Debug, Clone, Copy)]
pub struct MainMenuUi;

#[derive(Component, Debug, Clone, Copy)]
pub struct PauseMenuUi;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PauseMenuPage {
    Status,
    Settings,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PauseMenuState {
    pub owner: Entity,
    pub page: PauseMenuPage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryKind {
    Item,
    Npc,
}

#[derive(Debug, Clone, Reflect, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscoveryEntry {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub description: String,
    pub image_path: Option<String>,
    pub model_path: Option<String>,
    pub seen: bool,
}

impl DiscoveryEntry {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: String::new(),
            description: String::new(),
            image_path: None,
            model_path: None,
            seen: false,
        }
    }

    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    #[allow(dead_code)]
    pub fn image_path(mut self, path: impl Into<String>) -> Self {
        self.image_path = Some(path.into());
        self
    }

    pub fn model_path(mut self, path: impl Into<String>) -> Self {
        self.model_path = Some(path.into());
        self
    }

    pub fn seen(mut self, seen: bool) -> Self {
        self.seen = seen;
        self
    }
}

impl Default for DiscoveryEntry {
    fn default() -> Self {
        Self::new("", "")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryInteractionAction {
    Collected,
    Inspected,
    Shared,
    StatusChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Reflect, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryInteractionActor {
    Player,
    Speaker(String),
    System,
}

#[derive(Debug, Clone, Reflect, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscoveryInteraction {
    pub kind: DiscoveryKind,
    pub id: String,
    pub action: DiscoveryInteractionAction,
    pub actor: DiscoveryInteractionActor,
    pub script_id: Option<String>,
    pub node_id: Option<String>,
    pub option_id: Option<String>,
    pub note: Option<String>,
}

impl Default for DiscoveryInteraction {
    fn default() -> Self {
        Self {
            kind: DiscoveryKind::Item,
            id: String::new(),
            action: DiscoveryInteractionAction::StatusChanged,
            actor: DiscoveryInteractionActor::System,
            script_id: None,
            node_id: None,
            option_id: None,
            note: None,
        }
    }
}

impl DiscoveryInteraction {
    pub fn new(
        kind: DiscoveryKind,
        id: impl Into<String>,
        action: DiscoveryInteractionAction,
        actor: DiscoveryInteractionActor,
    ) -> Self {
        Self {
            kind,
            id: id.into(),
            action,
            actor,
            script_id: None,
            node_id: None,
            option_id: None,
            note: None,
        }
    }

    pub fn script(mut self, script_id: impl Into<String>) -> Self {
        self.script_id = Some(script_id.into());
        self
    }

    pub fn node(mut self, node_id: impl Into<String>) -> Self {
        self.node_id = Some(node_id.into());
        self
    }

    pub fn option(mut self, option_id: impl Into<String>) -> Self {
        self.option_id = Some(option_id.into());
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

#[derive(Debug, Clone, Reflect, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscoveryInteractionRecord {
    pub sequence: u64,
    pub interaction: DiscoveryInteraction,
}

impl Default for DiscoveryInteractionRecord {
    fn default() -> Self {
        Self {
            sequence: 0,
            interaction: DiscoveryInteraction::default(),
        }
    }
}

#[derive(Debug, Clone, Reflect, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDiscoveryDbSnapshot {
    pub items: Vec<DiscoveryEntry>,
    pub npcs: Vec<DiscoveryEntry>,
    pub interactions: Vec<DiscoveryInteractionRecord>,
    pub revision: u64,
    pub next_interaction_sequence: u64,
}

impl Default for UiDiscoveryDbSnapshot {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            npcs: Vec::new(),
            interactions: Vec::new(),
            revision: 1,
            next_interaction_sequence: 1,
        }
    }
}

#[derive(Message, Debug, Clone)]
pub enum UiDiscoveryCommand {
    Upsert {
        kind: DiscoveryKind,
        entry: DiscoveryEntry,
    },
    Remove {
        kind: DiscoveryKind,
        id: String,
    },
    SetSeen {
        kind: DiscoveryKind,
        id: String,
        seen: bool,
    },
    MoveItem {
        id: String,
        to_index: usize,
    },
    DropItem {
        id: String,
    },
    ClearKind {
        kind: DiscoveryKind,
    },
    RecordInteraction {
        interaction: DiscoveryInteraction,
    },
    ReplaceAll {
        snapshot: UiDiscoveryDbSnapshot,
    },
}

#[derive(Message, Debug, Clone)]
pub struct SpawnDroppedItem {
    pub id: String,
    pub model_path: String,
}

#[derive(Message, Debug, Clone, Copy)]
pub enum UiMenuAction {
    Play(Entity),
    Continue(Entity),
    Resume(Entity),
    BackToMainMenu(Entity),
    QuitGame,
}

#[derive(Debug, Clone)]
pub struct UiDialogueOption {
    pub text: String,
    pub preview: Option<UiDialoguePreview>,
    pub item_id: Option<String>,
    pub seen: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UiDialoguePreview {
    pub title: String,
    pub subtitle: String,
    pub description: String,
    pub image_path: Option<String>,
    pub model_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UiDialogueRequest {
    pub mode: UiDialogueMode,
    pub speaker: String,
    pub text: String,
    pub portrait_path: String,
    pub preview: Option<UiDialoguePreview>,
    pub options: Vec<UiDialogueOption>,
    pub reveal_duration_secs: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiDialogueMode {
    Standard,
    Inventory,
}

#[derive(Message, Debug, Clone)]
pub enum UiDialogueCommand {
    OpenInventory,
    Start(UiDialogueRequest),
    Advance,
    Close,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct MenuOwner(pub Entity);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MenuRoot {
    pub owner: Entity,
    pub kind: MenuKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MenuKind {
    Main,
    Pause,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct MenuButton {
    pub action: ButtonAction,
    pub raised: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConfirmAction {
    BackToMainMenu,
    QuitGame,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct MenuConfirmState {
    pub owner: Entity,
    pub pending: Option<ConfirmAction>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ButtonAction {
    SelectPage(MainMenuPage),
    SelectDiscovery(DiscoveryKind, usize),
    AdjustSetting(SettingKey, i32),
    Play,
    Continue,
    Resume,
    BackToMainMenu,
    QuitGame,
    ConfirmProceed,
    ConfirmCancel,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MainMenuState {
    pub owner: Entity,
    pub page: MainMenuPage,
    pub selected_item: Option<usize>,
    pub selected_npc: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MainMenuPage {
    Home,
    Credits,
    DiscoveredItems,
    PhoneList,
    Settings,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct MainMenuHeading {
    pub owner: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct MainMenuLine {
    pub owner: Entity,
    pub index: usize,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct MainMenuTab;

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct MainMenuTerminalPanel {
    pub owner: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct ConfirmDialogRoot {
    pub owner: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct ConfirmDialogMessage {
    pub owner: Entity,
}

#[derive(Component, Debug, Clone)]
pub(super) struct MainMenuTicker {
    pub tips: Vec<String>,
    pub current: usize,
    pub offset_x: f32,
    pub pause_timer: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct MainMenuGalleryPanel {
    pub owner: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct MainMenuSettingsPanel {
    pub owner: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct PauseMenuStatusPanel {
    pub owner: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct PauseMenuSettingsPanel {
    pub owner: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct GalleryListRoot {
    pub owner: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct GalleryDetailTitle {
    pub owner: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct GalleryDetailSubtitle {
    pub owner: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct GalleryDetailDescription {
    pub owner: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct GalleryDetailStatus {
    pub owner: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct SettingsValueText {
    pub owner: Entity,
    pub key: SettingKey,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct GalleryListCache {
    pub owner: Entity,
    pub kind: DiscoveryKind,
    pub revision: u64,
    pub selected: Option<usize>,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct DisabledButton;

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct DitherPixel {
    pub base: Color,
    pub accent: Color,
    pub phase: f32,
    pub speed: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct UiCursorSprite;

#[derive(Component, Debug, Clone, Copy)]
pub struct DialogueUiRoot;

#[derive(Component, Debug, Clone, Copy)]
pub struct InventoryUiRoot;
