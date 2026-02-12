use bevy::prelude::*;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum DiscoveryKind {
    Item,
    Npc,
}

#[derive(Debug, Clone, Reflect)]
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
    ClearKind {
        kind: DiscoveryKind,
    },
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
    pub speaker: String,
    pub text: String,
    pub portrait_path: String,
    pub preview: Option<UiDialoguePreview>,
    pub options: Vec<UiDialogueOption>,
    pub reveal_duration_secs: f32,
}

#[derive(Message, Debug, Clone)]
pub enum UiDialogueCommand {
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
