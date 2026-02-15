use std::collections::HashMap;

use bevy::{
    input::ButtonInput,
    prelude::*,
    text::{Justify, LineBreak, TextLayout},
};

use super::{MainMenuUi, systems::UiFonts, theme};

const ENDING_REVEAL_CHARS_PER_SEC: f32 = 58.0;
const ENDING_HOLD_SECS: f32 = 0.9;
const ENDING_HOLD_DECAY_SECS: f32 = 0.5;
const ENDING_HOLD_BAR_STEPS: usize = 14;

#[derive(Component, Debug, Clone, Copy)]
pub struct EndingUiRoot;

#[derive(Resource, Default)]
pub(super) struct UiEndingRuntime {
    root: Option<Entity>,
    body_text: Option<Entity>,
    hold_text: Option<Entity>,
    narrative_chars: Vec<char>,
    revealed_chars: usize,
    reveal_accum: f32,
    hold_progress: f32,
}

#[derive(Debug, Clone)]
pub struct UiEndingPayload {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub narrative: String,
    pub status_lines: Vec<String>,
}

impl UiEndingPayload {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: "END // UNKNOWN".into(),
            subtitle: "archive pending".into(),
            narrative: "No narrative provided.".into(),
            status_lines: vec!["timeline integrity: [reconstructing]".into()],
        }
    }

    pub fn title(mut self, value: impl Into<String>) -> Self {
        self.title = value.into();
        self
    }

    pub fn subtitle(mut self, value: impl Into<String>) -> Self {
        self.subtitle = value.into();
        self
    }

    pub fn narrative(mut self, value: impl Into<String>) -> Self {
        self.narrative = value.into();
        self
    }

    pub fn status_lines<I, S>(mut self, lines: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.status_lines = lines.into_iter().map(Into::into).collect();
        self
    }
}

#[derive(Resource, Debug, Default)]
pub struct UiEndingCatalog {
    endings: HashMap<String, UiEndingPayload>,
    debug_order: Vec<String>,
}

impl UiEndingCatalog {
    pub fn upsert(&mut self, payload: UiEndingPayload) {
        let id = payload.id.clone();
        if !self.debug_order.iter().any(|it| it == &id) {
            self.debug_order.push(id.clone());
        }
        self.endings.insert(id, payload);
    }

    pub fn remove(&mut self, id: &str) {
        self.endings.remove(id);
        self.debug_order.retain(|it| it != id);
    }

    pub fn get(&self, id: &str) -> Option<&UiEndingPayload> {
        self.endings.get(id)
    }

    pub fn debug_ids(&self) -> &[String] {
        &self.debug_order
    }
}

fn default_endings() -> [UiEndingPayload; 1] {
    [UiEndingPayload::new("collapse")
        .title("END | SIGNAL COLLAPSE")
        .subtitle("your mind broke before sunrise")
        .narrative("You never had the chance to get out of there.")
        .status_lines(["timeline integrity: fragmented"])]
}

pub(super) fn populate_default_endings(mut catalog: ResMut<UiEndingCatalog>) {
    if !catalog.endings.is_empty() {
        return;
    }
    for payload in default_endings() {
        catalog.upsert(payload);
    }
}

#[derive(Message, Debug, Clone)]
pub enum UiEndingCommand {
    Upsert(UiEndingPayload),
    Remove { id: String },
    Show { id: String },
    ShowPayload(UiEndingPayload),
    Close,
}

#[allow(dead_code)]
pub trait UiEndingCommandsExt {
    fn upsert_ending(&mut self, payload: UiEndingPayload);
    fn remove_ending(&mut self, id: impl Into<String>);
    fn show_ending(&mut self, id: impl Into<String>);
    fn show_ending_payload(&mut self, payload: UiEndingPayload);
    fn close_ending(&mut self);
}

impl UiEndingCommandsExt for Commands<'_, '_> {
    fn upsert_ending(&mut self, payload: UiEndingPayload) {
        self.write_message(UiEndingCommand::Upsert(payload));
    }

    fn remove_ending(&mut self, id: impl Into<String>) {
        self.write_message(UiEndingCommand::Remove { id: id.into() });
    }

    fn show_ending(&mut self, id: impl Into<String>) {
        self.write_message(UiEndingCommand::Show { id: id.into() });
    }

    fn show_ending_payload(&mut self, payload: UiEndingPayload) {
        self.write_message(UiEndingCommand::ShowPayload(payload));
    }

    fn close_ending(&mut self) {
        self.write_message(UiEndingCommand::Close);
    }
}

pub(super) fn apply_ending_commands(
    mut commands: Commands,
    mut msgs: MessageReader<UiEndingCommand>,
    mut runtime: ResMut<UiEndingRuntime>,
    mut catalog: ResMut<UiEndingCatalog>,
    fonts: Res<UiFonts>,
    roots: Query<(), With<EndingUiRoot>>,
    children: Query<&Children>,
) {
    for msg in msgs.read() {
        match msg {
            UiEndingCommand::Upsert(payload) => catalog.upsert(payload.clone()),
            UiEndingCommand::Remove { id } => catalog.remove(id),
            UiEndingCommand::Show { id } =>
                if let Some(payload) = catalog.get(id).cloned() {
                    open_ending(
                        &mut commands,
                        &mut runtime,
                        &fonts,
                        &roots,
                        &children,
                        payload,
                    );
                },
            UiEndingCommand::ShowPayload(payload) => {
                open_ending(
                    &mut commands,
                    &mut runtime,
                    &fonts,
                    &roots,
                    &children,
                    payload.clone(),
                );
            }
            UiEndingCommand::Close => close_ending(&mut commands, &mut runtime, &roots, &children),
        }
    }
}

pub(super) fn update_ending_reveal(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut runtime: ResMut<UiEndingRuntime>,
    mut texts: Query<&mut Text>,
) {
    let Some(body_entity) = runtime.body_text else {
        return;
    };
    let Ok(mut body_text) = texts.get_mut(body_entity) else {
        return;
    };

    let skip = keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::Enter)
        || mouse.just_pressed(MouseButton::Left);
    if skip {
        runtime.revealed_chars = runtime.narrative_chars.len();
    } else if runtime.revealed_chars < runtime.narrative_chars.len() {
        runtime.reveal_accum += ENDING_REVEAL_CHARS_PER_SEC * time.delta_secs();
        let step = runtime.reveal_accum.floor() as usize;
        if step > 0 {
            runtime.reveal_accum -= step as f32;
            runtime.revealed_chars =
                (runtime.revealed_chars + step).min(runtime.narrative_chars.len());
        }
    }

    body_text.0 = runtime.narrative_chars[..runtime.revealed_chars]
        .iter()
        .collect::<String>();
}

pub(super) fn update_ending_hold_to_continue(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut runtime: ResMut<UiEndingRuntime>,
    mut texts: Query<&mut Text>,
    mut commands: Commands,
    roots: Query<(), With<EndingUiRoot>>,
    children: Query<&Children>,
    drivers: Query<(Entity, &Name, Has<MainMenuUi>)>,
) {
    if runtime.body_text.is_none() || runtime.revealed_chars < runtime.narrative_chars.len() {
        return;
    }

    let hold_active = keys.pressed(KeyCode::Space) || mouse.pressed(MouseButton::Left);
    if hold_active {
        runtime.hold_progress =
            (runtime.hold_progress + time.delta_secs() / ENDING_HOLD_SECS).min(1.0);
    } else {
        runtime.hold_progress =
            (runtime.hold_progress - time.delta_secs() / ENDING_HOLD_DECAY_SECS).max(0.0);
    }

    if let Some(hold_entity) = runtime.hold_text
        && let Ok(mut hold_text) = texts.get_mut(hold_entity)
    {
        let filled = (runtime.hold_progress * ENDING_HOLD_BAR_STEPS as f32).round() as usize;
        let empty = ENDING_HOLD_BAR_STEPS.saturating_sub(filled);
        hold_text.0 = format!(
            "hold to continue [{}{}]",
            "■".repeat(filled),
            "·".repeat(empty)
        );
    }

    if runtime.hold_progress >= 1.0 {
        close_ending(&mut commands, &mut runtime, &roots, &children);
        ensure_main_menu_driver(&mut commands, &drivers);
    }
}

fn open_ending(
    commands: &mut Commands,
    runtime: &mut UiEndingRuntime,
    fonts: &UiFonts,
    roots: &Query<(), With<EndingUiRoot>>,
    children: &Query<&Children>,
    payload: UiEndingPayload,
) {
    close_ending(commands, runtime, roots, children);

    let mut body_entity = Entity::PLACEHOLDER;
    let mut hold_entity = Entity::PLACEHOLDER;
    let status_block = payload.status_lines.join("\n");

    let root = commands
        .spawn((
            Name::new("Ending UI"),
            EndingUiRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.01, 0.03, 0.86)),
            GlobalZIndex(140),
        ))
        .id();

    commands.entity(root).with_children(|overlay| {
        overlay
            .spawn((
                Name::new("Ending Panel"),
                Node {
                    width: Val::Px(920.0),
                    border: UiRect::all(Val::Px(3.0)),
                    padding: UiRect::all(Val::Px(18.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(12.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.04, 0.08, 0.92)),
                theme::border(true),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new(payload.title),
                    TextFont {
                        font: fonts.pixel.clone(),
                        font_size: 34.0,
                        ..default()
                    },
                    TextColor(theme::TEXT_LIGHT),
                ));

                panel.spawn((
                    Text::new(payload.subtitle),
                    TextFont {
                        font: fonts.body.clone(),
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.86, 0.90)),
                ));

                body_entity = panel
                    .spawn((
                        Text::new(""),
                        TextFont {
                            font: fonts.body.clone(),
                            font_size: 21.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                        Node {
                            width: Val::Percent(100.0),
                            min_height: Val::Px(122.0),
                            ..default()
                        },
                    ))
                    .id();

                panel.spawn((
                    Text::new(status_block),
                    TextFont {
                        font: fonts.body.clone(),
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.76, 0.78, 0.84)),
                    TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                    Node {
                        width: Val::Percent(100.0),
                        ..default()
                    },
                ));

                hold_entity = panel
                    .spawn((
                        Text::new(format!(
                            "hold to continue [{}]",
                            "·".repeat(ENDING_HOLD_BAR_STEPS)
                        )),
                        TextFont {
                            font: fonts.pixel.clone(),
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(theme::TEXT_LIGHT),
                        TextLayout::new(Justify::Center, LineBreak::NoWrap),
                        Node {
                            width: Val::Percent(100.0),
                            margin: UiRect::top(Val::Px(8.0)),
                            ..default()
                        },
                    ))
                    .id();
            });
    });

    runtime.root = Some(root);
    runtime.body_text = Some(body_entity);
    runtime.hold_text = Some(hold_entity);
    runtime.narrative_chars = payload.narrative.chars().collect();
    runtime.revealed_chars = 0;
    runtime.reveal_accum = 0.0;
    runtime.hold_progress = 0.0;
}

fn close_ending(
    commands: &mut Commands,
    runtime: &mut UiEndingRuntime,
    roots: &Query<(), With<EndingUiRoot>>,
    children: &Query<&Children>,
) {
    if let Some(root) = runtime.root.take()
        && roots.get(root).is_ok()
    {
        despawn_tree(commands, root, children);
    }
    runtime.body_text = None;
    runtime.hold_text = None;
    runtime.narrative_chars.clear();
    runtime.revealed_chars = 0;
    runtime.reveal_accum = 0.0;
    runtime.hold_progress = 0.0;
}

fn ensure_main_menu_driver(
    commands: &mut Commands,
    drivers: &Query<(Entity, &Name, Has<MainMenuUi>)>,
) {
    for (entity, name, has_main_menu) in drivers {
        if name.as_str() != "Main Menu Driver" {
            continue;
        }
        if !has_main_menu {
            commands.entity(entity).insert(MainMenuUi);
        }
        return;
    }
    commands.spawn((Name::new("Main Menu Driver"), MainMenuUi));
}

fn despawn_tree(commands: &mut Commands, root: Entity, children_query: &Query<&Children>) {
    if let Ok(children) = children_query.get(root) {
        for child in children.iter() {
            despawn_tree(commands, child, children_query);
        }
    }
    commands.queue(move |world: &mut World| {
        if world.entities().contains(root) {
            let _ = world.despawn(root);
        }
    });
}
