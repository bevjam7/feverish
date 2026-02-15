use bevy::{
    prelude::*,
    text::{Justify, LineBreak, TextLayout},
};

use super::{systems::UiFonts, theme};

const DEFAULT_HINT_FONT_SIZE: f32 = 24.0;
const DEFAULT_HINT_GLITCH_SECS: f32 = 0.48;
const DEFAULT_HINT_FADE_SECS: f32 = 0.36;
const HINT_FRAME_BASE_ALPHA: f32 = 0.80;
const HINT_TEXT_BASE_ALPHA: f32 = 1.0;

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct UiHintRoot;

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct UiHintText;

#[derive(Component, Debug, Clone)]
pub(super) struct UiHintFade {
    timer: Timer,
}

#[derive(Component, Debug, Clone)]
pub(super) struct UiHintGlitch {
    timer: Timer,
}

#[derive(Resource, Default, Debug, Clone, Copy)]
pub(super) struct UiHintRuntime {
    root: Option<Entity>,
    text: Option<Entity>,
    frame: Option<Entity>,
}

#[derive(Debug, Clone)]
pub struct UiHintRequest {
    pub text: String,
    pub font_size: f32,
    pub glitch_secs: f32,
}

impl UiHintRequest {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            font_size: DEFAULT_HINT_FONT_SIZE,
            glitch_secs: DEFAULT_HINT_GLITCH_SECS,
        }
    }

    pub fn font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size.max(8.0);
        self
    }

    pub fn glitch_secs(mut self, glitch_secs: f32) -> Self {
        self.glitch_secs = glitch_secs.max(0.0);
        self
    }
}

impl From<&str> for UiHintRequest {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for UiHintRequest {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Message, Debug, Clone)]
pub enum UiHintCommand {
    Show(UiHintRequest),
    FadeOut { duration_secs: f32 },
    Hide,
}

#[allow(dead_code)]
pub trait UiHintCommandsExt {
    fn show_ui_hint(&mut self, text: impl Into<String>);
    fn show_ui_hint_with(&mut self, request: UiHintRequest);
    fn fade_out_ui_hint(&mut self, duration_secs: f32);
    fn hide_ui_hint(&mut self);
}

impl UiHintCommandsExt for Commands<'_, '_> {
    fn show_ui_hint(&mut self, text: impl Into<String>) {
        self.write_message(UiHintCommand::Show(UiHintRequest::new(text)));
    }

    fn show_ui_hint_with(&mut self, request: UiHintRequest) {
        self.write_message(UiHintCommand::Show(request));
    }

    fn fade_out_ui_hint(&mut self, duration_secs: f32) {
        let duration_secs = if duration_secs <= 0.0 {
            DEFAULT_HINT_FADE_SECS
        } else {
            duration_secs
        };
        self.write_message(UiHintCommand::FadeOut { duration_secs });
    }

    fn hide_ui_hint(&mut self) {
        self.write_message(UiHintCommand::Hide);
    }
}

pub(super) fn apply_hint_commands(
    mut commands: Commands,
    mut msgs: MessageReader<UiHintCommand>,
    mut runtime: ResMut<UiHintRuntime>,
    fonts: Res<UiFonts>,
    roots: Query<(), With<UiHintRoot>>,
    children: Query<&Children>,
) {
    for msg in msgs.read() {
        match msg {
            UiHintCommand::Show(request) => {
                open_hint(
                    &mut commands,
                    &mut runtime,
                    &fonts,
                    &roots,
                    &children,
                    request.clone(),
                );
            }
            UiHintCommand::FadeOut { duration_secs } => {
                let Some(root) = runtime.root else {
                    continue;
                };
                if roots.get(root).is_err() {
                    continue;
                }
                let fade_secs = if *duration_secs <= 0.0 {
                    DEFAULT_HINT_FADE_SECS
                } else {
                    *duration_secs
                };
                commands.entity(root).insert(UiHintFade {
                    timer: Timer::from_seconds(fade_secs.max(0.02), TimerMode::Once),
                });
            }
            UiHintCommand::Hide => {
                close_hint(&mut commands, &mut runtime, &roots, &children);
            }
        }
    }
}

pub(super) fn animate_hint_glitch(
    time: Res<Time>,
    mut commands: Commands,
    mut hints: Query<
        (Entity, &mut UiHintGlitch, &mut UiTransform, &mut TextColor),
        With<UiHintText>,
    >,
) {
    for (entity, mut glitch, mut transform, mut text_color) in &mut hints {
        glitch.timer.tick(time.delta());
        let total = glitch.timer.duration().as_secs_f32().max(f32::EPSILON);
        let t = (glitch.timer.elapsed_secs() / total).clamp(0.0, 1.0);
        let intensity = 1.0 - t;
        let elapsed = glitch.timer.elapsed_secs();

        let jitter_x = ((elapsed * 53.0).sin() * 3.2 + (elapsed * 91.0).cos() * 1.8) * intensity;
        let jitter_y = ((elapsed * 39.0).sin() * 1.4) * intensity;
        transform.translation = Val2::px(jitter_x.round(), jitter_y.round());

        let blink = (elapsed * 21.0).sin() * 0.5 + 0.5;
        let alpha = (0.78 + blink * 0.22 * intensity).clamp(0.0, 1.0);
        text_color.0 = Color::srgba(0.97, 0.98, 1.0, alpha);

        if glitch.timer.is_finished() {
            transform.translation = Val2::ZERO;
            text_color.0 = Color::srgba(0.97, 0.98, 1.0, HINT_TEXT_BASE_ALPHA);
            commands.entity(entity).remove::<UiHintGlitch>();
        }
    }
}

pub(super) fn animate_hint_fade(
    time: Res<Time>,
    mut commands: Commands,
    mut runtime: ResMut<UiHintRuntime>,
    mut fades: Query<&mut UiHintFade, With<UiHintRoot>>,
    mut backgrounds: Query<&mut BackgroundColor>,
    mut text_colors: Query<&mut TextColor, With<UiHintText>>,
    roots: Query<(), With<UiHintRoot>>,
    children: Query<&Children>,
) {
    let Some(root) = runtime.root else {
        return;
    };

    let Ok(mut fade) = fades.get_mut(root) else {
        return;
    };
    fade.timer.tick(time.delta());
    let total = fade.timer.duration().as_secs_f32().max(f32::EPSILON);
    let t = (fade.timer.elapsed_secs() / total).clamp(0.0, 1.0);
    let alpha = 1.0 - t;

    if let Some(frame) = runtime.frame {
        if let Ok(mut bg) = backgrounds.get_mut(frame) {
            bg.0 = Color::srgba(0.04, 0.05, 0.09, HINT_FRAME_BASE_ALPHA * alpha);
        }
    }
    if let Some(text) = runtime.text {
        if let Ok(mut color) = text_colors.get_mut(text) {
            color.0 = Color::srgba(0.97, 0.98, 1.0, HINT_TEXT_BASE_ALPHA * alpha);
        }
    }

    if fade.timer.is_finished() {
        close_hint(&mut commands, &mut runtime, &roots, &children);
    }
}

fn open_hint(
    commands: &mut Commands,
    runtime: &mut UiHintRuntime,
    fonts: &UiFonts,
    roots: &Query<(), With<UiHintRoot>>,
    children: &Query<&Children>,
    request: UiHintRequest,
) {
    close_hint(commands, runtime, roots, children);

    let mut frame = Entity::PLACEHOLDER;
    let mut text = Entity::PLACEHOLDER;

    let root = commands
        .spawn((
            Name::new("UI Hint"),
            UiHintRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::End,
                padding: UiRect::bottom(Val::Px(48.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Pickable::IGNORE,
            GlobalZIndex(110),
        ))
        .id();

    commands.entity(root).with_children(|overlay| {
        let mut frame_entity = overlay.spawn((
            Name::new("UI Hint Frame"),
            Node {
                max_width: Val::Px(860.0),
                min_width: Val::Px(220.0),
                border: UiRect::all(Val::Px(2.0)),
                padding: UiRect::new(Val::Px(16.0), Val::Px(16.0), Val::Px(10.0), Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.05, 0.09, HINT_FRAME_BASE_ALPHA)),
            theme::border(true),
        ));
        frame = frame_entity.id();

        frame_entity.with_children(|frame_node| {
            let mut text_entity = frame_node.spawn((
                Name::new("UI Hint Text"),
                UiHintText,
                Text::new(request.text.clone()),
                TextFont {
                    font: fonts.body.clone(),
                    font_size: request.font_size,
                    ..default()
                },
                TextColor(theme::TEXT_LIGHT),
                TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                Node {
                    width: Val::Percent(100.0),
                    ..default()
                },
                UiTransform::IDENTITY,
            ));

            if request.glitch_secs > 0.0 {
                text_entity.insert(UiHintGlitch {
                    timer: Timer::from_seconds(request.glitch_secs, TimerMode::Once),
                });
            }
            text = text_entity.id();
        });
    });

    runtime.root = Some(root);
    runtime.text = Some(text);
    runtime.frame = Some(frame);
}

fn close_hint(
    commands: &mut Commands,
    runtime: &mut UiHintRuntime,
    roots: &Query<(), With<UiHintRoot>>,
    children: &Query<&Children>,
) {
    if let Some(root) = runtime.root.take() {
        if roots.get(root).is_ok() {
            despawn_tree(commands, root, children);
        }
    }
    runtime.text = None;
    runtime.frame = None;
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
