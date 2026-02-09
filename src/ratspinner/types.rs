use std::collections::HashMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::voice::VoicePreset;

#[derive(Message, Debug, Clone)]
pub enum RatCommand {
    Start(RatStart),
    Advance,
    Choose(usize),
    Close,
    Register(RatScript),
}

#[derive(Debug, Clone)]
pub struct RatStart {
    pub script_id: String,
    pub target: Option<Entity>,
}

impl RatStart {
    pub fn new(script_id: impl Into<String>) -> Self {
        Self {
            script_id: script_id.into(),
            target: None,
        }
    }

    pub fn target(mut self, entity: Entity) -> Self {
        self.target = Some(entity);
        self
    }
}

#[derive(Message, Debug, Clone)]
pub struct RatHookTriggered {
    pub hook: String,
    pub script_id: String,
    pub node_id: String,
    pub option_id: Option<String>,
    pub target: Option<Entity>,
}

#[allow(dead_code)]
pub trait RatCommandsExt {
    fn rat(&mut self, cmd: RatCommand);
    fn rat_start(&mut self, start: RatStart);
    fn rat_register(&mut self, script: RatScript);
}

impl RatCommandsExt for Commands<'_, '_> {
    fn rat(&mut self, cmd: RatCommand) {
        self.write_message(cmd);
    }

    fn rat_start(&mut self, start: RatStart) {
        self.write_message(RatCommand::Start(start));
    }

    fn rat_register(&mut self, script: RatScript) {
        self.write_message(RatCommand::Register(script));
    }
}

#[derive(Debug, Clone)]
pub struct RatScript {
    pub id: String,
    pub entry: String,
    pub nodes: HashMap<String, RatNode>,
}

#[derive(Asset, TypePath, Debug, Clone)]
pub struct RatScriptAsset {
    pub script: RatScript,
}

impl RatScriptAsset {
    pub fn new(script: RatScript) -> Self {
        Self { script }
    }
}

#[derive(Debug, Clone)]
pub struct RatNode {
    pub id: String,
    pub speaker: String,
    pub text: String,
    pub portrait_path: String,
    pub voice: VoicePreset,
    pub next: Option<String>,
    pub hooks: Vec<String>,
    pub options: Vec<RatOption>,
}

#[derive(Debug, Clone)]
pub struct RatOption {
    pub id: Option<String>,
    pub text: String,
    pub next: Option<String>,
    pub hooks: Vec<String>,
}

impl RatScript {
    pub fn single(
        id: impl Into<String>,
        speaker: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        let id = id.into();
        let entry = "start".to_string();
        let node = RatNodeBuilder::new("start")
            .speaker(speaker)
            .text(text)
            .build();
        Self {
            id,
            entry,
            nodes: HashMap::from([(node.id.clone(), node)]),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RatScriptBuilder {
    id: String,
    entry: String,
    nodes: Vec<RatNodeBuilder>,
}

impl RatScriptBuilder {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            entry: "start".to_string(),
            nodes: Vec::new(),
        }
    }

    pub fn entry(mut self, node_id: impl Into<String>) -> Self {
        self.entry = node_id.into();
        self
    }

    pub fn node(mut self, node: RatNodeBuilder) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn build(self) -> RatScript {
        let mut nodes = HashMap::new();
        for node in self.nodes {
            let built = node.build();
            nodes.insert(built.id.clone(), built);
        }
        RatScript {
            id: self.id,
            entry: self.entry,
            nodes,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RatNodeBuilder {
    id: String,
    speaker: String,
    text: String,
    portrait_path: String,
    voice: VoicePreset,
    next: Option<String>,
    hooks: Vec<String>,
    options: Vec<RatOptionBuilder>,
}

impl RatNodeBuilder {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            speaker: "unknown".to_string(),
            text: String::new(),
            portrait_path: "models/npc_a/npc_a.png".to_string(),
            voice: VoicePreset::NeutralNpc,
            next: None,
            hooks: Vec::new(),
            options: Vec::new(),
        }
    }

    pub fn speaker(mut self, speaker: impl Into<String>) -> Self {
        self.speaker = speaker.into();
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    pub fn portrait(mut self, path: impl Into<String>) -> Self {
        self.portrait_path = path.into();
        self
    }

    pub fn voice(mut self, preset: VoicePreset) -> Self {
        self.voice = preset;
        self
    }

    pub fn next(mut self, node_id: impl Into<String>) -> Self {
        self.next = Some(node_id.into());
        self
    }

    pub fn hook(mut self, hook: impl Into<String>) -> Self {
        self.hooks.push(hook.into());
        self
    }

    pub fn option(mut self, option: RatOptionBuilder) -> Self {
        self.options.push(option);
        self
    }

    pub fn build(self) -> RatNode {
        RatNode {
            id: self.id,
            speaker: self.speaker,
            text: self.text,
            portrait_path: self.portrait_path,
            voice: self.voice,
            next: self.next,
            hooks: self.hooks,
            options: self
                .options
                .into_iter()
                .map(RatOptionBuilder::build)
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RatOptionBuilder {
    id: Option<String>,
    text: String,
    next: Option<String>,
    hooks: Vec<String>,
}

impl RatOptionBuilder {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            id: None,
            text: text.into(),
            next: None,
            hooks: Vec::new(),
        }
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn goto(mut self, node_id: impl Into<String>) -> Self {
        self.next = Some(node_id.into());
        self
    }

    pub fn hook(mut self, hook: impl Into<String>) -> Self {
        self.hooks.push(hook.into());
        self
    }

    pub fn build(self) -> RatOption {
        RatOption {
            id: self.id,
            text: self.text,
            next: self.next,
            hooks: self.hooks,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatScriptRon {
    pub id: String,
    pub entry: String,
    pub nodes: Vec<RatNodeRon>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatNodeRon {
    pub id: String,
    pub speaker: String,
    pub text: String,
    pub portrait_path: String,
    #[serde(default)]
    pub voice: RatVoiceRon,
    pub next: Option<String>,
    #[serde(default)]
    pub hooks: Vec<String>,
    #[serde(default)]
    pub options: Vec<RatOptionRon>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatOptionRon {
    pub id: Option<String>,
    pub text: String,
    pub next: Option<String>,
    #[serde(default)]
    pub hooks: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RatVoiceRon {
    HostileEntity,
    LostChild,
    CorruptedTransmission,
    #[default]
    NeutralNpc,
}

impl From<RatVoiceRon> for VoicePreset {
    fn from(value: RatVoiceRon) -> Self {
        match value {
            RatVoiceRon::HostileEntity => VoicePreset::HostileEntity,
            RatVoiceRon::LostChild => VoicePreset::LostChild,
            RatVoiceRon::CorruptedTransmission => VoicePreset::CorruptedTransmission,
            RatVoiceRon::NeutralNpc => VoicePreset::NeutralNpc,
        }
    }
}

impl From<VoicePreset> for RatVoiceRon {
    fn from(value: VoicePreset) -> Self {
        match value {
            VoicePreset::HostileEntity => RatVoiceRon::HostileEntity,
            VoicePreset::LostChild => RatVoiceRon::LostChild,
            VoicePreset::CorruptedTransmission => RatVoiceRon::CorruptedTransmission,
            VoicePreset::NeutralNpc => RatVoiceRon::NeutralNpc,
        }
    }
}

impl From<&RatScript> for RatScriptRon {
    fn from(value: &RatScript) -> Self {
        let mut nodes: Vec<&RatNode> = value.nodes.values().collect();
        nodes.sort_by(|a, b| a.id.cmp(&b.id));

        Self {
            id: value.id.clone(),
            entry: value.entry.clone(),
            nodes: nodes
                .into_iter()
                .map(|node| RatNodeRon {
                    id: node.id.clone(),
                    speaker: node.speaker.clone(),
                    text: node.text.clone(),
                    portrait_path: node.portrait_path.clone(),
                    voice: node.voice.into(),
                    next: node.next.clone(),
                    hooks: node.hooks.clone(),
                    options: node
                        .options
                        .iter()
                        .map(|option| RatOptionRon {
                            id: option.id.clone(),
                            text: option.text.clone(),
                            next: option.next.clone(),
                            hooks: option.hooks.clone(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

impl TryFrom<RatScriptRon> for RatScript {
    type Error = String;

    fn try_from(value: RatScriptRon) -> Result<Self, Self::Error> {
        let mut nodes = HashMap::new();
        for node in value.nodes {
            if nodes.contains_key(&node.id) {
                return Err(format!("duplicate node id '{}'", node.id));
            }
            let built = RatNode {
                id: node.id.clone(),
                speaker: node.speaker,
                text: node.text,
                portrait_path: node.portrait_path,
                voice: node.voice.into(),
                next: node.next,
                hooks: node.hooks,
                options: node
                    .options
                    .into_iter()
                    .map(|opt| RatOption {
                        id: opt.id,
                        text: opt.text,
                        next: opt.next,
                        hooks: opt.hooks,
                    })
                    .collect(),
            };
            nodes.insert(node.id, built);
        }

        if !nodes.contains_key(&value.entry) {
            return Err(format!(
                "entry node '{}' not found in script '{}'",
                value.entry, value.id
            ));
        }

        Ok(RatScript {
            id: value.id,
            entry: value.entry,
            nodes,
        })
    }
}
