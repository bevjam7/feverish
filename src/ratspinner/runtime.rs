use std::{collections::HashMap, fmt, str::FromStr};

use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
    reflect::TypePath,
};

use super::types::{
    RatCommand, RatDialoguePresentation, RatHookTriggered, RatNode, RatNodeBuilder, RatOption,
    RatOptionBuilder, RatScript, RatScriptAsset, RatScriptBuilder, RatScriptRon, RatStart,
};
use crate::{
    assets::GameAssets,
    ui::{
        DiscoveryInteraction, DiscoveryInteractionAction, DiscoveryInteractionActor, DiscoveryKind,
        UiDialogueCommand, UiDialogueMode, UiDialogueOption, UiDialoguePreview, UiDialogueRequest,
        UiDiscoveryCommand, UiDiscoveryDb,
    },
    voice::{Speak, StopVoice, VoicePreset, estimate_speech_duration_secs},
};

#[derive(Default, TypePath)]
pub(super) struct RatScriptAssetLoader;

#[derive(Debug)]
pub enum RatScriptAssetLoaderError {
    Io(std::io::Error),
    Utf8(std::str::Utf8Error),
    Ron(ron::error::SpannedError),
    Invalid(String),
    UnsupportedExtension(String),
}

impl fmt::Display for RatScriptAssetLoaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "failed to read script asset bytes: {error}"),
            Self::Utf8(error) => write!(f, "script asset is not valid utf-8: {error}"),
            Self::Ron(error) => write!(f, "failed to parse script RON: {error}"),
            Self::Invalid(error) => write!(f, "invalid script asset: {error}"),
            Self::UnsupportedExtension(ext) => {
                write!(f, "unsupported script extension '{ext}'")
            }
        }
    }
}

impl std::error::Error for RatScriptAssetLoaderError {}

impl From<std::io::Error> for RatScriptAssetLoaderError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<std::str::Utf8Error> for RatScriptAssetLoaderError {
    fn from(value: std::str::Utf8Error) -> Self {
        Self::Utf8(value)
    }
}

impl From<ron::error::SpannedError> for RatScriptAssetLoaderError {
    fn from(value: ron::error::SpannedError) -> Self {
        Self::Ron(value)
    }
}

impl AssetLoader for RatScriptAssetLoader {
    type Asset = RatScriptAsset;
    type Error = RatScriptAssetLoaderError;
    type Settings = ();

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let path = load_context.path().path();
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let fallback_script_id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("script");

        let script = match extension.as_str() {
            "rat" => {
                let content = std::str::from_utf8(&bytes)?;
                parse_rat_script(content, fallback_script_id)?
            }
            "ron" => {
                let raw = ron::de::from_bytes::<RatScriptRon>(&bytes)?;
                RatScript::try_from(raw).map_err(RatScriptAssetLoaderError::Invalid)?
            }
            _ => return Err(RatScriptAssetLoaderError::UnsupportedExtension(extension)),
        };

        Ok(RatScriptAsset::new(script))
    }

    fn extensions(&self) -> &[&str] {
        &["rat", "ron"]
    }
}

#[derive(Resource, Default)]
pub(super) struct RatLibrary {
    scripts: HashMap<String, RatScript>,
}

#[derive(Resource, Default)]
pub(super) struct RatRuntime {
    active: Option<ActiveDialogue>,
}

#[derive(Resource, Default)]
pub struct RatDialogueState {
    pub active: bool,
}

#[derive(Debug, Clone)]
struct ActiveDialogue {
    script_id: String,
    node_id: String,
    target: Option<Entity>,
    presentation: RatDialoguePresentation,
    overlay: DialogueOverlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum DialogueOverlay {
    #[default]
    None,
    InventoryPicker,
    ItemResponse,
}

pub(super) fn load_scripts_from_assets(
    mut library: ResMut<RatLibrary>,
    game_assets: Res<GameAssets>,
    script_assets: Res<Assets<RatScriptAsset>>,
) {
    library.scripts.clear();

    for handle in &game_assets.rat_scripts {
        if let Some(asset) = script_assets.get(handle) {
            let script = asset.script.clone();
            library.scripts.insert(script.id.clone(), script);
        } else {
            warn!("ratspinner script handle not ready: {:?}", handle);
        }
    }
}

pub(super) fn seed_builtin_script(mut library: ResMut<RatLibrary>) {
    if library.scripts.contains_key("npc.default") {
        return;
    }

    let script = RatScriptBuilder::new("npc.default")
        .entry("greeting")
        .node(
            RatNodeBuilder::new("greeting")
                .speaker("mr. d.")
                .portrait("models/npc_a/npc_a.png")
                .text("hey there! i'm mr d.")
                .hook("npc.default.greeting")
                .option(
                    RatOptionBuilder::new("who are you?")
                        .id("ask_identity")
                        .goto("identity")
                        .hook("npc.default.option.identity"),
                )
                .option(
                    RatOptionBuilder::new("what happened here?")
                        .id("ask_place")
                        .goto("place")
                        .hook("npc.default.option.place"),
                )
                .option(
                    RatOptionBuilder::new("i should go.")
                        .id("leave")
                        .hook("npc.default.option.leave"),
                ),
        )
        .node(
            RatNodeBuilder::new("identity")
                .speaker("mr. d.")
                .portrait("models/npc_a/npc_a.png")
                .text("i am mr d!!!!!")
                .next("greeting"),
        )
        .node(
            RatNodeBuilder::new("place")
                .speaker("mr. d.")
                .portrait("models/npc_a/npc_a.png")
                .text("its doom time")
                .next("greeting"),
        )
        .build();

    library.scripts.insert(script.id.clone(), script);
}

#[derive(Debug)]
struct RatNodeDraft {
    id: String,
    speaker: String,
    text: String,
    portrait_path: String,
    voice: VoicePreset,
    next: Option<String>,
    hooks: Vec<String>,
    options: Vec<RatOption>,
}

impl RatNodeDraft {
    fn new(id: String) -> Self {
        Self {
            id,
            speaker: "unknown".to_string(),
            text: String::new(),
            portrait_path: "models/npc_a/npc_a.png".to_string(),
            voice: VoicePreset::NeutralNpc,
            next: None,
            hooks: Vec::new(),
            options: Vec::new(),
        }
    }

    fn build(self) -> RatNode {
        RatNode {
            id: self.id,
            speaker: self.speaker,
            text: self.text,
            portrait_path: self.portrait_path,
            voice: self.voice,
            next: self.next,
            hooks: self.hooks,
            options: self.options,
        }
    }
}

fn parse_rat_script(
    content: &str,
    fallback_script_id: &str,
) -> Result<RatScript, RatScriptAssetLoaderError> {
    let mut script_id = fallback_script_id.to_string();
    let mut entry = "start".to_string();
    let mut nodes: HashMap<String, RatNode> = HashMap::new();
    let mut first_node_id: Option<String> = None;
    let mut current: Option<RatNodeDraft> = None;

    for (line_index, line) in content.lines().enumerate() {
        let raw = line.trim();
        if raw.is_empty() {
            continue;
        }

        if let Some(meta) = raw.strip_prefix("//") {
            parse_metadata_comment(meta.trim(), &mut script_id, &mut entry);
            continue;
        }

        if raw.starts_with('[') && raw.ends_with(']') {
            flush_current_node(&mut current, &mut nodes)?;
            let node_id = raw[1..raw.len() - 1].trim();
            if node_id.is_empty() {
                return Err(RatScriptAssetLoaderError::Invalid(format!(
                    "line {} has an empty node id",
                    line_index + 1
                )));
            }
            if first_node_id.is_none() {
                first_node_id = Some(node_id.to_string());
            }
            current = Some(RatNodeDraft::new(node_id.to_string()));
            continue;
        }

        let Some(node) = current.as_mut() else {
            continue;
        };

        if let Some(option_raw) = raw.strip_prefix('>') {
            let option = parse_option_line(option_raw.trim(), line_index + 1)?;
            node.options.push(option);
            continue;
        }

        if let Some(next_raw) = raw.strip_prefix("->") {
            let next = next_raw.trim();
            if !next.is_empty() {
                node.next = Some(next.to_string());
            }
            continue;
        }

        if let Some((key, value)) = raw.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "speaker" => node.speaker = value.to_string(),
                "text" => node.text = value.to_string(),
                "portrait" => node.portrait_path = value.to_string(),
                "voice" => {
                    node.voice = VoicePreset::from_str(value).unwrap_or(VoicePreset::NeutralNpc);
                }
                "hook" => extend_hooks(value, &mut node.hooks),
                _ => {}
            }
        }
    }

    flush_current_node(&mut current, &mut nodes)?;

    if nodes.is_empty() {
        return Err(RatScriptAssetLoaderError::Invalid(
            "script does not define any nodes".to_string(),
        ));
    }

    if !nodes.contains_key(&entry) {
        if let Some(first) = first_node_id {
            entry = first;
        } else {
            return Err(RatScriptAssetLoaderError::Invalid(format!(
                "entry node '{}' was not found",
                entry
            )));
        }
    }

    Ok(RatScript {
        id: script_id,
        entry,
        nodes,
    })
}

fn parse_metadata_comment(raw: &str, script_id: &mut String, entry: &mut String) {
    let Some((key, value)) = raw.split_once(':') else {
        return;
    };
    let key = key.trim();
    let value = value.trim();
    if value.is_empty() {
        return;
    }

    match key {
        "script" => *script_id = value.to_string(),
        "entry" => *entry = value.to_string(),
        _ => {}
    }
}

fn parse_option_line(
    raw: &str,
    line_number: usize,
) -> Result<RatOption, RatScriptAssetLoaderError> {
    let (text_part, metadata, inline_annotations) = if let Some((text, rest)) = raw.split_once("->")
    {
        (text.trim(), Some(rest.trim()), "")
    } else {
        let (text, annotations) = split_target_and_annotations(raw.trim());
        (text, None, annotations)
    };

    if text_part.is_empty() {
        return Err(RatScriptAssetLoaderError::Invalid(format!(
            "line {} has an empty option text",
            line_number
        )));
    }

    let mut option = RatOption {
        id: None,
        text: text_part.to_string(),
        next: None,
        hooks: Vec::new(),
    };

    if let Some(meta) = metadata {
        let (target, annotations) = split_target_and_annotations(meta);
        if !target.is_empty() {
            option.next = Some(target.to_string());
        }
        parse_option_annotations(annotations, &mut option);
    }
    parse_option_annotations(inline_annotations, &mut option);

    Ok(option)
}

fn split_target_and_annotations(raw: &str) -> (&str, &str) {
    if let Some(index) = raw.find('[') {
        (raw[..index].trim(), raw[index..].trim())
    } else {
        (raw.trim(), "")
    }
}

fn parse_option_annotations(raw: &str, option: &mut RatOption) {
    let mut remaining = raw.trim();
    while let Some(start) = remaining.find('[') {
        let Some(end_rel) = remaining[start + 1..].find(']') else {
            break;
        };
        let end = start + 1 + end_rel;
        let annotation = remaining[start + 1..end].trim();
        if let Some(value) = annotation.strip_prefix("hook:") {
            extend_hooks(value, &mut option.hooks);
        } else if let Some(value) = annotation.strip_prefix("id:") {
            let value = value.trim();
            if !value.is_empty() {
                option.id = Some(value.to_string());
            }
        }
        remaining = remaining[end + 1..].trim();
    }
}

fn extend_hooks(raw: &str, hooks: &mut Vec<String>) {
    for hook in raw.split(',') {
        let hook = hook.trim();
        if !hook.is_empty() {
            hooks.push(hook.to_string());
        }
    }
}

fn flush_current_node(
    current: &mut Option<RatNodeDraft>,
    nodes: &mut HashMap<String, RatNode>,
) -> Result<(), RatScriptAssetLoaderError> {
    let Some(node) = current.take() else {
        return Ok(());
    };
    if nodes.contains_key(&node.id) {
        return Err(RatScriptAssetLoaderError::Invalid(format!(
            "duplicate node id '{}'",
            node.id
        )));
    }
    let built = node.build();
    nodes.insert(built.id.clone(), built);
    Ok(())
}

pub(super) fn handle_rat_commands(
    mut commands: Commands,
    mut messages: MessageReader<RatCommand>,
    mut hooks: MessageWriter<RatHookTriggered>,
    mut ui_commands: MessageWriter<UiDialogueCommand>,
    mut discovery_commands: MessageWriter<UiDiscoveryCommand>,
    mut runtime: ResMut<RatRuntime>,
    mut state: ResMut<RatDialogueState>,
    mut library: ResMut<RatLibrary>,
    discovery_db: Res<UiDiscoveryDb>,
) {
    for msg in messages.read() {
        match msg {
            RatCommand::Register(script) => {
                library.scripts.insert(script.id.clone(), script.clone());
            }
            RatCommand::Start(start) => {
                start_dialogue(
                    &mut commands,
                    &mut hooks,
                    &mut ui_commands,
                    &mut discovery_commands,
                    &library,
                    &mut runtime,
                    &mut state,
                    &discovery_db,
                    start.clone(),
                );
            }
            RatCommand::Advance => {
                advance_dialogue(
                    &mut commands,
                    &mut hooks,
                    &mut ui_commands,
                    &mut discovery_commands,
                    &library,
                    &mut runtime,
                    &mut state,
                    &discovery_db,
                );
            }
            RatCommand::Choose(index) => {
                choose_option(
                    &mut commands,
                    &mut hooks,
                    &mut ui_commands,
                    &mut discovery_commands,
                    &library,
                    &mut runtime,
                    &mut state,
                    &discovery_db,
                    *index,
                );
            }
            RatCommand::Close => {
                let headless = is_headless(runtime.active.as_ref());
                runtime.active = None;
                state.active = false;
                if !headless {
                    ui_commands.write(UiDialogueCommand::Close);
                }
                commands.write_message(StopVoice);
            }
        }
    }
}

fn start_dialogue(
    commands: &mut Commands,
    hooks: &mut MessageWriter<RatHookTriggered>,
    ui_commands: &mut MessageWriter<UiDialogueCommand>,
    discovery_commands: &mut MessageWriter<UiDiscoveryCommand>,
    library: &RatLibrary,
    runtime: &mut RatRuntime,
    state: &mut RatDialogueState,
    discovery_db: &UiDiscoveryDb,
    start: RatStart,
) {
    let Some(script) = library.scripts.get(&start.script_id) else {
        warn!("ratspinner script '{}' not found", start.script_id);
        return;
    };

    runtime.active = Some(ActiveDialogue {
        script_id: start.script_id,
        node_id: script.entry.clone(),
        target: start.target,
        presentation: start.presentation,
        overlay: DialogueOverlay::None,
    });
    discovery_commands.write(UiDiscoveryCommand::RecordInteraction {
        interaction: DiscoveryInteraction::new(
            DiscoveryKind::Npc,
            script.id.clone(),
            DiscoveryInteractionAction::Inspected,
            DiscoveryInteractionActor::Player,
        )
        .script(script.id.clone())
        .node(script.entry.clone())
        .note("dialogue.start"),
    });
    state.active = true;

    show_current_node(
        commands,
        hooks,
        ui_commands,
        discovery_commands,
        library,
        runtime,
        state,
        discovery_db,
        true,
    );
}

fn advance_dialogue(
    commands: &mut Commands,
    hooks: &mut MessageWriter<RatHookTriggered>,
    ui_commands: &mut MessageWriter<UiDialogueCommand>,
    discovery_commands: &mut MessageWriter<UiDiscoveryCommand>,
    library: &RatLibrary,
    runtime: &mut RatRuntime,
    state: &mut RatDialogueState,
    discovery_db: &UiDiscoveryDb,
) {
    let Some(active) = runtime.active.clone() else {
        return;
    };
    let Some(script) = library.scripts.get(&active.script_id) else {
        close_dialogue(commands, runtime, state, ui_commands);
        return;
    };
    let Some(node) = script.nodes.get(&active.node_id) else {
        close_dialogue(commands, runtime, state, ui_commands);
        return;
    };

    if !node.options.is_empty() {
        choose_option(
            commands,
            hooks,
            ui_commands,
            discovery_commands,
            library,
            runtime,
            state,
            discovery_db,
            0,
        );
        return;
    }

    if let Some(next) = node.next.clone() {
        if let Some(active_mut) = runtime.active.as_mut() {
            active_mut.node_id = next;
            active_mut.overlay = DialogueOverlay::None;
        }
        show_current_node(
            commands,
            hooks,
            ui_commands,
            discovery_commands,
            library,
            runtime,
            state,
            discovery_db,
            true,
        );
    } else {
        close_dialogue(commands, runtime, state, ui_commands);
    }
}

fn choose_option(
    commands: &mut Commands,
    hooks: &mut MessageWriter<RatHookTriggered>,
    ui_commands: &mut MessageWriter<UiDialogueCommand>,
    discovery_commands: &mut MessageWriter<UiDiscoveryCommand>,
    library: &RatLibrary,
    runtime: &mut RatRuntime,
    state: &mut RatDialogueState,
    discovery_db: &UiDiscoveryDb,
    index: usize,
) {
    let Some(active_snapshot) = runtime.active.clone() else {
        return;
    };
    let Some(script) = library.scripts.get(&active_snapshot.script_id) else {
        close_dialogue(commands, runtime, state, ui_commands);
        return;
    };
    let Some(node) = script.nodes.get(&active_snapshot.node_id) else {
        close_dialogue(commands, runtime, state, ui_commands);
        return;
    };

    if active_snapshot.overlay == DialogueOverlay::InventoryPicker {
        let items = discovery_db.entries(DiscoveryKind::Item);
        if index < items.len() {
            let item = &items[index];
            let specific_response_id = format!("response_{}", item.id);
            hooks.write(RatHookTriggered {
                hook: "dialogue.show_item".to_string(),
                script_id: active_snapshot.script_id.clone(),
                node_id: active_snapshot.node_id.clone(),
                option_id: Some(item.id.clone()),
                target: active_snapshot.target,
            });

            if script.nodes.contains_key(&specific_response_id) {
                if let Some(active_mut) = runtime.active.as_mut() {
                    active_mut.node_id = specific_response_id;
                    active_mut.overlay = DialogueOverlay::None;
                }
                show_current_node(
                    commands,
                    hooks,
                    ui_commands,
                    discovery_commands,
                    library,
                    runtime,
                    state,
                    discovery_db,
                    true,
                );
                return;
            }

            if let Some(active_mut) = runtime.active.as_mut() {
                active_mut.overlay = DialogueOverlay::ItemResponse;
            }
            discovery_commands.write(UiDiscoveryCommand::SetSeen {
                kind: DiscoveryKind::Item,
                id: item.id.clone(),
                seen: true,
            });
            let already_shared = discovery_db.was_item_shared_with_speaker(
                &item.id,
                &active_snapshot.script_id,
                &node.speaker,
            );
            discovery_commands.write(UiDiscoveryCommand::RecordInteraction {
                interaction: DiscoveryInteraction::new(
                    DiscoveryKind::Item,
                    item.id.clone(),
                    DiscoveryInteractionAction::Shared,
                    DiscoveryInteractionActor::Speaker(node.speaker.clone()),
                )
                .script(active_snapshot.script_id.clone())
                .node(active_snapshot.node_id.clone())
                .option(item.id.clone())
                .note("dialogue.show_item"),
            });
            let headless = is_headless(Some(&active_snapshot));
            let line = show_item_response(ui_commands, node, item, already_shared, headless);
            if !headless {
                commands.write_message(StopVoice);
                let mut speak_msg = Speak::new(line).voice(node.voice);
                if let Some(target) = active_snapshot.target {
                    speak_msg = speak_msg.target(target);
                }
                commands.write_message(speak_msg);
            }
            return;
        }
        if let Some(active_mut) = runtime.active.as_mut() {
            active_mut.overlay = DialogueOverlay::None;
        }
        show_current_node(
            commands,
            hooks,
            ui_commands,
            discovery_commands,
            library,
            runtime,
            state,
            discovery_db,
            true,
        );
        return;
    }

    if active_snapshot.overlay == DialogueOverlay::ItemResponse {
        if let Some(active_mut) = runtime.active.as_mut() {
            active_mut.overlay = DialogueOverlay::None;
        }
        show_current_node(
            commands,
            hooks,
            ui_commands,
            discovery_commands,
            library,
            runtime,
            state,
            discovery_db,
            true,
        );
        return;
    }

    let inventory_idx = node.options.len();
    if index == inventory_idx {
        if discovery_db.entries(DiscoveryKind::Item).is_empty() {
            return;
        }
        open_inventory_picker(
            ui_commands,
            discovery_db,
            &active_snapshot.script_id,
            &node.speaker,
            is_headless(Some(&active_snapshot)),
        );
        if let Some(active_mut) = runtime.active.as_mut() {
            active_mut.overlay = DialogueOverlay::InventoryPicker;
        }
        if !is_headless(Some(&active_snapshot)) {
            commands.write_message(StopVoice);
        }
        return;
    }

    if node.options.is_empty() {
        advance_dialogue(
            commands,
            hooks,
            ui_commands,
            discovery_commands,
            library,
            runtime,
            state,
            discovery_db,
        );
        return;
    }

    let option = node
        .options
        .get(index)
        .or_else(|| node.options.first())
        .cloned();
    let Some(option) = option else {
        close_dialogue(commands, runtime, state, ui_commands);
        return;
    };

    for hook in &option.hooks {
        hooks.write(RatHookTriggered {
            hook: hook.clone(),
            script_id: active_snapshot.script_id.clone(),
            node_id: active_snapshot.node_id.clone(),
            option_id: option.id.clone(),
            target: active_snapshot.target,
        });
    }

    if let Some(next) = option.next.or_else(|| node.next.clone()) {
        if let Some(active_mut) = runtime.active.as_mut() {
            active_mut.node_id = next;
            active_mut.overlay = DialogueOverlay::None;
        }
        show_current_node(
            commands,
            hooks,
            ui_commands,
            discovery_commands,
            library,
            runtime,
            state,
            discovery_db,
            true,
        );
    } else {
        close_dialogue(commands, runtime, state, ui_commands);
    }
}

fn show_current_node(
    commands: &mut Commands,
    hooks: &mut MessageWriter<RatHookTriggered>,
    ui_commands: &mut MessageWriter<UiDialogueCommand>,
    _discovery_commands: &mut MessageWriter<UiDiscoveryCommand>,
    library: &RatLibrary,
    runtime: &mut RatRuntime,
    state: &mut RatDialogueState,
    discovery_db: &UiDiscoveryDb,
    speak: bool,
) {
    let Some(active) = runtime.active.clone() else {
        close_dialogue(commands, runtime, state, ui_commands);
        return;
    };
    let Some(script) = library.scripts.get(&active.script_id) else {
        close_dialogue(commands, runtime, state, ui_commands);
        return;
    };
    let Some(node) = script.nodes.get(&active.node_id) else {
        close_dialogue(commands, runtime, state, ui_commands);
        return;
    };

    for hook in &node.hooks {
        hooks.write(RatHookTriggered {
            hook: hook.clone(),
            script_id: active.script_id.clone(),
            node_id: active.node_id.clone(),
            option_id: None,
            target: active.target,
        });
    }

    if !is_headless(Some(&active)) {
        ui_commands.write(UiDialogueCommand::Start(UiDialogueRequest {
            mode: UiDialogueMode::Standard,
            speaker: node.speaker.clone(),
            text: node.text.clone(),
            portrait_path: node.portrait_path.clone(),
            preview: None,
            options: dialogue_options_for_node(node, discovery_db),
            reveal_duration_secs: if speak {
                estimate_speech_duration_secs(&node.text, node.voice)
            } else {
                0.0
            },
        }));
    }

    if speak && !is_headless(Some(&active)) {
        commands.write_message(StopVoice);
        let mut speak_msg = Speak::new(node.text.clone()).voice(node.voice);
        if let Some(target) = active.target {
            speak_msg = speak_msg.target(target);
        }
        commands.write_message(speak_msg);
    }
}

fn dialogue_options_for_node(
    node: &RatNode,
    discovery_db: &UiDiscoveryDb,
) -> Vec<UiDialogueOption> {
    let mut options: Vec<UiDialogueOption> = node
        .options
        .iter()
        .map(|option| UiDialogueOption {
            text: option.text.clone(),
            preview: None,
            item_id: None,
            seen: false,
            enabled: true,
        })
        .collect();

    options.push(UiDialogueOption {
        text: "Show item...".to_string(),
        preview: None,
        item_id: None,
        seen: false,
        enabled: !discovery_db.entries(DiscoveryKind::Item).is_empty(),
    });
    options
}

fn open_inventory_picker(
    ui_commands: &mut MessageWriter<UiDialogueCommand>,
    discovery_db: &UiDiscoveryDb,
    script_id: &str,
    speaker: &str,
    headless: bool,
) {
    if headless {
        return;
    }
    let mut options: Vec<UiDialogueOption> = discovery_db
        .entries(DiscoveryKind::Item)
        .iter()
        .map(|entry| UiDialogueOption {
            text: entry.title.clone(),
            preview: Some(UiDialoguePreview {
                title: entry.title.clone(),
                subtitle: entry.subtitle.clone(),
                description: entry.description.clone(),
                image_path: entry.image_path.clone(),
                model_path: entry.model_path.clone(),
            }),
            item_id: Some(entry.id.clone()),
            seen: discovery_db.was_item_shared_with_speaker(&entry.id, script_id, speaker),
            enabled: true,
        })
        .collect();
    options.push(UiDialogueOption {
        text: "back".to_string(),
        preview: None,
        item_id: None,
        seen: false,
        enabled: true,
    });

    let initial_preview = options.first().and_then(|option| option.preview.clone());

    ui_commands.write(UiDialogueCommand::Start(UiDialogueRequest {
        mode: UiDialogueMode::Inventory,
        speaker: "inventory".to_string(),
        text: "pick an item to show".to_string(),
        portrait_path: "models/npc_a/npc_a.png".to_string(),
        preview: initial_preview,
        options,
        reveal_duration_secs: 0.0,
    }));
}

fn show_item_response(
    ui_commands: &mut MessageWriter<UiDialogueCommand>,
    node: &RatNode,
    item: &crate::ui::DiscoveryEntry,
    already_shared: bool,
    headless: bool,
) -> String {
    let line = format!(
        "hmm... {}. {}",
        item.title,
        if already_shared {
            "i remember this"
        } else {
            "what is this?"
        }
    );
    if !headless {
        ui_commands.write(UiDialogueCommand::Start(UiDialogueRequest {
            mode: UiDialogueMode::Standard,
            speaker: node.speaker.clone(),
            text: line.clone(),
            portrait_path: node.portrait_path.clone(),
            preview: Some(UiDialoguePreview {
                title: item.title.clone(),
                subtitle: item.subtitle.clone(),
                description: item.description.clone(),
                image_path: item.image_path.clone(),
                model_path: item.model_path.clone(),
            }),
            options: vec![UiDialogueOption {
                text: "back".to_string(),
                preview: None,
                item_id: None,
                seen: false,
                enabled: true,
            }],
            reveal_duration_secs: estimate_speech_duration_secs(&line, node.voice),
        }));
    }
    line
}

fn close_dialogue(
    commands: &mut Commands,
    runtime: &mut RatRuntime,
    state: &mut RatDialogueState,
    ui_commands: &mut MessageWriter<UiDialogueCommand>,
) {
    let headless = is_headless(runtime.active.as_ref());
    runtime.active = None;
    state.active = false;
    if !headless {
        ui_commands.write(UiDialogueCommand::Close);
        commands.write_message(StopVoice);
    }
}

fn is_headless(active: Option<&ActiveDialogue>) -> bool {
    active.is_some_and(|dialogue| dialogue.presentation == RatDialoguePresentation::Headless)
}
