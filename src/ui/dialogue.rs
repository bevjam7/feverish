use bevy::{
    camera::{primitives::Aabb, visibility::RenderLayers},
    camera::RenderTarget,
    input::mouse::MouseMotion,
    prelude::*,
    render::render_resource::TextureFormat,
    text::{Justify, LineBreak, TextLayout},
    ui::{FocusPolicy, widget::ViewportNode},
    window::PrimaryWindow,
};

use super::{
    components::{DialogueUiRoot, UiDialogueCommand, UiDialogueOption, UiDialogueRequest},
    systems::UiFonts,
    theme,
};
use crate::{ratspinner::RatCommand, settings::GameSettings};

#[derive(Resource, Default)]
pub(super) struct UiDialogueState {
    pub active: bool,
}

#[derive(Resource, Default)]
pub(super) struct UiDialogueRuntime {
    session: Option<DialogueSession>,
}

struct DialogueSession {
    root: Entity,
    prompt_text: Entity,
    line_row: Entity,
    slot_text: Entity,
    text_chars: Vec<char>,
    options: Vec<UiDialogueOption>,
    selected_option: usize,
    revealed: usize,
    reveal_timer: f32,
    char_interval: f32,
    slot_anim_timer: f32,
    slot_anim_dir: f32,
    preview_viewport: Option<Entity>,
    preview_pivot: Option<Entity>,
    preview_world_root: Option<Entity>,
    preview_model_root: Option<Entity>,
    preview_camera: Option<Entity>,
    preview_framed: bool,
    preview_dragging: bool,
}

#[derive(Component)]
pub(super) struct DialogueGlyphFx {
    age: f32,
    duration: f32,
    amplitude: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct DialogueArrowButton {
    dir: i32,
}

#[derive(Component)]
pub(super) struct DialogueOptionSlot;

#[derive(Component)]
pub(super) struct DialoguePreviewViewport;

#[derive(Component)]
pub(super) struct DialoguePreviewPivot;

#[derive(Component)]
pub(super) struct DialoguePreviewLayerTagged;

const PREVIEW_RENDER_LAYER: usize = 19;

pub(super) fn apply_dialogue_commands(
    mut commands: Commands,
    mut msgs: MessageReader<UiDialogueCommand>,
    mut runtime: ResMut<UiDialogueRuntime>,
    mut state: ResMut<UiDialogueState>,
    fonts: Res<UiFonts>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    settings: Res<GameSettings>,
    children: Query<&Children>,
) {
    for msg in msgs.read() {
        match msg {
            UiDialogueCommand::Start(req) => {
                close_session(&mut commands, &mut runtime, &mut state, &children);
                let mut session = spawn_dialogue(
                    &mut commands,
                    &fonts,
                    &assets,
                    &mut images,
                    req.clone(),
                    settings.dialogue_speed,
                );
                reset_text(&mut commands, &mut session, &children);
                state.active = true;
                runtime.session = Some(session);
            }
            UiDialogueCommand::Advance => {
                let Some(session) = runtime.session.as_mut() else {
                    continue;
                };
                if session.revealed < session.text_chars.len() {
                    reveal_all_chars(&mut commands, session, &fonts, &children);
                    refresh_prompt(&mut commands, session);
                    continue;
                }

                if session.options.is_empty() {
                    commands.write_message(RatCommand::Advance);
                } else {
                    commands.write_message(RatCommand::Choose(session.selected_option));
                }
            }
            UiDialogueCommand::Close => {
                close_session(&mut commands, &mut runtime, &mut state, &children);
            }
        }
    }
}

pub(super) fn advance_dialogue_with_mouse(
    mouse: Res<ButtonInput<MouseButton>>,
    dialogue_state: Res<UiDialogueState>,
    runtime: Res<UiDialogueRuntime>,
    windows: Query<&Window, With<PrimaryWindow>>,
    viewports: Query<(&ComputedNode, &UiGlobalTransform), With<DialoguePreviewViewport>>,
    mut commands: Commands,
) {
    if !dialogue_state.active || !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let Some(session) = runtime.session.as_ref() else {
        return;
    };
    if is_cursor_over_preview(session, &windows, &viewports) {
        return;
    }

    if session.revealed < session.text_chars.len() {
        commands.write_message(UiDialogueCommand::Advance);
    } else if session.options.is_empty() {
        commands.write_message(RatCommand::Advance);
    }
}

pub(super) fn rotate_dialogue_preview(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    windows: Query<&Window, With<PrimaryWindow>>,
    viewports: Query<(&ComputedNode, &UiGlobalTransform), With<DialoguePreviewViewport>>,
    mut runtime: ResMut<UiDialogueRuntime>,
    mut pivots: Query<&mut Transform, With<DialoguePreviewPivot>>,
) {
    let Some(session) = runtime.session.as_mut() else {
        return;
    };
    let Some(pivot_entity) = session.preview_pivot else {
        return;
    };
    let Ok(mut pivot_transform) = pivots.get_mut(pivot_entity) else {
        return;
    };

    let mut drag_delta = Vec2::ZERO;
    for event in mouse_motion.read() {
        drag_delta += event.delta;
    }

    let hovered = is_cursor_over_preview(session, &windows, &viewports);
    if session.preview_dragging && mouse.pressed(MouseButton::Left) {
        if drag_delta.length_squared() > f32::EPSILON {
            pivot_transform.rotate_y(-drag_delta.x * 0.012);
            pivot_transform.rotate_local_x(-drag_delta.y * 0.010);
        }
        return;
    }

    if hovered && mouse.pressed(MouseButton::Left) {
        session.preview_dragging = true;
        if drag_delta.length_squared() > f32::EPSILON {
            pivot_transform.rotate_y(-drag_delta.x * 0.012);
            pivot_transform.rotate_local_x(-drag_delta.y * 0.010);
        }
        return;
    }

    session.preview_dragging = false;
    let dt = time.delta_secs();
    pivot_transform.rotate_y(dt * 0.56);
    pivot_transform.rotate_local_x(dt * 0.23);
    pivot_transform.rotate_local_z(dt * 0.17);
}

pub(super) fn sync_and_frame_dialogue_preview(
    mut commands: Commands,
    mut runtime: ResMut<UiDialogueRuntime>,
    children: Query<&Children>,
    tagged: Query<(), With<DialoguePreviewLayerTagged>>,
    aabbs: Query<(&Aabb, &GlobalTransform)>,
    mut transforms: Query<&mut Transform>,
) {
    let Some(session) = runtime.session.as_mut() else {
        return;
    };
    let Some(model_root) = session.preview_model_root else {
        return;
    };

    if !tagged.contains(model_root) {
        commands.entity(model_root).insert((
            RenderLayers::layer(PREVIEW_RENDER_LAYER),
            DialoguePreviewLayerTagged,
        ));
    }

    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut found_mesh = false;

    for entity in children.iter_descendants(model_root) {
        if !tagged.contains(entity) {
            commands.entity(entity).insert((
                RenderLayers::layer(PREVIEW_RENDER_LAYER),
                DialoguePreviewLayerTagged,
            ));
        }
        let Ok((aabb, global)) = aabbs.get(entity) else {
            continue;
        };
        let max_scale = global.compute_transform().scale.max_element().max(0.001);
        let center = global.translation() + Vec3::from(aabb.center);
        let radius = aabb.half_extents.max_element() * max_scale;
        let extent = Vec3::splat(radius);
        min = min.min(center - extent);
        max = max.max(center + extent);
        found_mesh = true;
    }

    if session.preview_framed || !found_mesh {
        return;
    }

    let center = (min + max) * 0.5;
    let radius = ((max - min) * 0.5).max_element().max(0.20);

    if let Ok(mut model_transform) = transforms.get_mut(model_root) {
        model_transform.translation = -center;
    }
    if let Some(camera) = session.preview_camera {
        if let Ok(mut camera_transform) = transforms.get_mut(camera) {
            let eye = Vec3::new(radius * 0.35, radius * 0.75 + 0.12, radius * 3.00 + 0.55);
            *camera_transform =
                Transform::from_translation(eye).looking_at(Vec3::new(0.0, radius * 0.20, 0.0), Vec3::Y);
        }
    }

    session.preview_framed = true;
}

pub(super) fn handle_dialogue_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<UiDialogueRuntime>,
    dialogue_state: Res<UiDialogueState>,
    mut commands: Commands,
) {
    if !dialogue_state.active {
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        commands.write_message(RatCommand::Close);
        return;
    }

    let Some(session) = runtime.session.as_mut() else {
        return;
    };

    if session.revealed < session.text_chars.len() {
        if any_pressed(&keys, &[KeyCode::KeyE, KeyCode::Enter, KeyCode::Space]) {
            commands.write_message(UiDialogueCommand::Advance);
        }
        return;
    }

    if session.options.is_empty() {
        if any_pressed(&keys, &[KeyCode::KeyE, KeyCode::Enter, KeyCode::Space]) {
            commands.write_message(RatCommand::Advance);
        }
        return;
    }

    if any_pressed(&keys, &[KeyCode::ArrowLeft, KeyCode::KeyA]) {
        cycle_option(&mut commands, session, -1);
    }
    if any_pressed(&keys, &[KeyCode::ArrowRight, KeyCode::KeyD]) {
        cycle_option(&mut commands, session, 1);
    }
    if any_pressed(&keys, &[KeyCode::KeyE, KeyCode::Enter, KeyCode::Space]) {
        commands.write_message(RatCommand::Choose(session.selected_option));
    }
}

pub(super) fn handle_dialogue_arrow_buttons(
    mut interactions: Query<
        (
            &Interaction,
            &DialogueArrowButton,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        Changed<Interaction>,
    >,
    mut runtime: ResMut<UiDialogueRuntime>,
    mut commands: Commands,
) {
    let Some(session) = runtime.session.as_mut() else {
        return;
    };

    for (interaction, arrow, mut bg, mut border) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(theme::BUTTON_BG);
                *border = theme::border(false);
                if session.revealed >= session.text_chars.len() && !session.options.is_empty() {
                    cycle_option(&mut commands, session, arrow.dir);
                }
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(theme::BUTTON_HOVER);
                *border = theme::border(true);
            }
            Interaction::None => {
                *bg = BackgroundColor(theme::BUTTON_BG);
                *border = theme::border(true);
            }
        }
    }
}

pub(super) fn update_typewriter_dialogue(
    time: Res<Time>,
    mut commands: Commands,
    fonts: Res<UiFonts>,
    mut runtime: ResMut<UiDialogueRuntime>,
    _children: Query<&Children>,
) {
    let Some(session) = runtime.session.as_mut() else {
        return;
    };
    if session.revealed >= session.text_chars.len() {
        return;
    }

    session.reveal_timer += time.delta_secs();
    while session.reveal_timer >= session.char_interval
        && session.revealed < session.text_chars.len()
    {
        session.reveal_timer -= session.char_interval;
        let ch = session.text_chars[session.revealed];
        spawn_char(&mut commands, session.line_row, ch, &fonts);
        session.revealed += 1;
        if matches!(ch, '.' | ',' | ';' | ':' | '!' | '?') {
            session.reveal_timer -= session.char_interval * 0.55;
        }
    }

    if session.revealed >= session.text_chars.len() {
        refresh_prompt(&mut commands, session);
    }
}

pub(super) fn animate_dialogue_glyphs(
    time: Res<Time>,
    mut glyphs: Query<(Entity, &mut UiTransform, &mut DialogueGlyphFx)>,
    mut commands: Commands,
) {
    for (entity, mut transform, mut fx) in &mut glyphs {
        fx.age += time.delta_secs();
        let t = (fx.age / fx.duration).clamp(0.0, 1.0);
        let inv = 1.0 - t;
        transform.translation = Val2::px(0.0, (-fx.amplitude * inv * inv).round());
        transform.scale = Vec2::splat(1.0 + inv * 0.24);
        if t >= 1.0 {
            transform.translation = Val2::ZERO;
            transform.scale = Vec2::ONE;
            commands.entity(entity).remove::<DialogueGlyphFx>();
        }
    }
}

pub(super) fn animate_option_slot_transition(
    time: Res<Time>,
    mut runtime: ResMut<UiDialogueRuntime>,
    mut slots: Query<&mut UiTransform, With<DialogueOptionSlot>>,
) {
    let Some(session) = runtime.session.as_mut() else {
        return;
    };
    let Ok(mut transform) = slots.get_mut(session.slot_text) else {
        return;
    };

    if session.slot_anim_timer <= 0.0 {
        transform.translation = Val2::ZERO;
        transform.scale = Vec2::ONE;
        return;
    }

    session.slot_anim_timer = (session.slot_anim_timer - time.delta_secs()).max(0.0);
    let t = 1.0 - (session.slot_anim_timer / 0.16);
    let wave = (t * std::f32::consts::PI).sin();
    transform.translation = Val2::px((session.slot_anim_dir * wave * 18.0).round(), 0.0);
    transform.scale = Vec2::splat(1.0 + wave * 0.07);
}

fn spawn_dialogue(
    commands: &mut Commands,
    fonts: &UiFonts,
    assets: &AssetServer,
    images: &mut Assets<Image>,
    req: UiDialogueRequest,
    dialogue_speed: f32,
) -> DialogueSession {
    let preview_scene = req
        .preview
        .as_ref()
        .and_then(|preview| preview.model_path.as_deref())
        .and_then(|model_path| spawn_preview_world(commands, assets, images, model_path));

    let root = commands
        .spawn((
            Name::new("Dialogue UI"),
            DialogueUiRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::End,
                padding: UiRect::new(Val::Px(20.0), Val::Px(20.0), Val::Px(20.0), Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            FocusPolicy::Block,
            GlobalZIndex(120),
        ))
        .id();

    let mut prompt_text = Entity::PLACEHOLDER;
    let mut line_row = Entity::PLACEHOLDER;
    let mut slot_text = Entity::PLACEHOLDER;
    let mut preview_viewport = None;

    commands.entity(root).with_children(|overlay| {
        overlay
            .spawn((
                Node {
                    width: Val::Px(920.0),
                    height: Val::Px(260.0),
                    min_height: Val::Px(260.0),
                    max_height: Val::Px(260.0),
                    border: UiRect::all(Val::Px(3.0)),
                    padding: UiRect::all(Val::Px(10.0)),
                    column_gap: Val::Px(10.0),
                    ..default()
                },
                BackgroundColor(theme::PANEL_BG),
                theme::border(true),
            ))
            .with_children(|frame| {
                frame
                    .spawn((
                        Node {
                            flex_grow: 1.0,
                            min_width: Val::Px(0.0),
                            border: UiRect::all(Val::Px(2.0)),
                            padding: UiRect::all(Val::Px(10.0)),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.05, 0.06, 0.08)),
                        theme::border(false),
                    ))
                    .with_children(|left| {
                        line_row = left
                            .spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Px(118.0),
                                    min_height: Val::Px(118.0),
                                    max_height: Val::Px(118.0),
                                    flex_wrap: FlexWrap::Wrap,
                                    align_content: AlignContent::FlexStart,
                                    overflow: Overflow::clip_y(),
                                    ..default()
                                },
                                BackgroundColor(Color::NONE),
                            ))
                            .id();

                        left.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(44.0),
                                min_height: Val::Px(44.0),
                                max_height: Val::Px(44.0),
                                border: UiRect::all(Val::Px(2.0)),
                                padding: UiRect::horizontal(Val::Px(6.0)),
                                column_gap: Val::Px(6.0),
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.03, 0.04, 0.06)),
                            theme::border(false),
                        ))
                        .with_children(|selector| {
                            spawn_arrow_button(selector, fonts, -1, "<");

                            selector
                                .spawn((
                                    Node {
                                        flex_grow: 1.0,
                                        height: Val::Percent(100.0),
                                        border: UiRect::all(Val::Px(2.0)),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.08, 0.10, 0.13)),
                                    theme::border(true),
                                ))
                                .with_children(|slot| {
                                    slot_text = slot
                                        .spawn((
                                            DialogueOptionSlot,
                                            Text::new("no choices"),
                                            TextFont {
                                                font: fonts.body.clone(),
                                                font_size: 26.0,
                                                ..default()
                                            },
                                            TextColor(theme::TEXT_LIGHT),
                                            TextLayout::new(Justify::Center, LineBreak::NoWrap),
                                            UiTransform::IDENTITY,
                                        ))
                                        .id();
                                });

                            spawn_arrow_button(selector, fonts, 1, ">");
                        });

                        left.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(30.0),
                                min_height: Val::Px(30.0),
                                max_height: Val::Px(30.0),
                                border: UiRect::new(
                                    Val::Px(0.0),
                                    Val::Px(0.0),
                                    Val::Px(2.0),
                                    Val::Px(0.0),
                                ),
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            theme::border(true),
                        ))
                        .with_children(|footer| {
                            prompt_text = footer
                                .spawn((
                                    Text::new("press e or click to skip"),
                                    TextFont {
                                        font: fonts.body.clone(),
                                        font_size: 24.0,
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.72, 0.72, 0.76)),
                                    TextLayout::new(Justify::Left, LineBreak::NoWrap),
                                ))
                                .id();
                        });
                    });

                frame
                    .spawn((
                        Node {
                            width: Val::Px(228.0),
                            min_width: Val::Px(228.0),
                            max_width: Val::Px(228.0),
                            border: UiRect::all(Val::Px(2.0)),
                            padding: UiRect::all(Val::Px(6.0)),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.11, 0.11, 0.11)),
                        theme::border(false),
                    ))
                    .with_children(|right| {
                        let label = req
                            .preview
                            .as_ref()
                            .map(|preview| preview.title.clone())
                            .unwrap_or_else(|| req.speaker.clone());
                        right.spawn((
                            Text::new(label),
                            TextFont {
                                font: fonts.pixel.clone(),
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor(theme::TEXT_LIGHT),
                            TextLayout::new(Justify::Center, LineBreak::NoWrap),
                        ));

                        right.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                flex_grow: 1.0,
                                min_height: Val::Px(0.0),
                                border: UiRect::all(Val::Px(2.0)),
                                padding: UiRect::all(Val::Px(6.0)),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(6.0),
                                overflow: Overflow::clip_y(),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.05, 0.06, 0.08)),
                            theme::border(false),
                        ))
                        .with_children(|card| {
                            if let Some(preview) = req.preview.as_ref() {
                                if let Some(preview_scene) = preview_scene.as_ref() {
                                    preview_viewport = Some(
                                        card.spawn((
                                            DialoguePreviewViewport,
                                            Node {
                                                width: Val::Percent(100.0),
                                                height: Val::Px(112.0),
                                                min_height: Val::Px(112.0),
                                                max_height: Val::Px(112.0),
                                                border: UiRect::all(Val::Px(2.0)),
                                                ..default()
                                            },
                                            ViewportNode::new(preview_scene.camera),
                                            BackgroundColor(Color::srgb(0.03, 0.04, 0.06)),
                                            theme::border(false),
                                        ))
                                        .id(),
                                    );
                                } else if preview.image_path.is_some() {
                                    card.spawn((
                                        Node {
                                            width: Val::Percent(100.0),
                                            height: Val::Px(112.0),
                                            min_height: Val::Px(112.0),
                                            max_height: Val::Px(112.0),
                                            border: UiRect::all(Val::Px(2.0)),
                                            ..default()
                                        },
                                        ImageNode::new(
                                            assets.load(
                                                preview
                                                    .image_path
                                                    .clone()
                                                    .unwrap_or_else(|| req.portrait_path.clone()),
                                            ),
                                        ),
                                        BackgroundColor(Color::srgb(0.03, 0.04, 0.06)),
                                        theme::border(false),
                                    ));
                                } else {
                                    card.spawn((
                                        Node {
                                            width: Val::Percent(100.0),
                                            height: Val::Px(112.0),
                                            min_height: Val::Px(112.0),
                                            max_height: Val::Px(112.0),
                                            border: UiRect::all(Val::Px(2.0)),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgb(0.03, 0.04, 0.06)),
                                        theme::border(false),
                                    ))
                                    .with_children(|empty| {
                                        empty.spawn((
                                            Text::new("no preview"),
                                            TextFont {
                                                font: fonts.pixel.clone(),
                                                font_size: 10.0,
                                                ..default()
                                            },
                                            TextColor(Color::srgb(0.60, 0.62, 0.68)),
                                        ));
                                    });
                                }

                                card.spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_grow: 1.0,
                                        min_height: Val::Px(0.0),
                                        border: UiRect::all(Val::Px(2.0)),
                                        padding: UiRect::all(Val::Px(5.0)),
                                        overflow: Overflow::scroll_y(),
                                        ..default()
                                    },
                                    ScrollPosition(Vec2::ZERO),
                                    BackgroundColor(Color::srgb(0.09, 0.10, 0.13)),
                                    theme::border(false),
                                ))
                                .with_children(|desc| {
                                    desc.spawn((
                                        Node {
                                            width: Val::Percent(100.0),
                                            ..default()
                                        },
                                        Text::new(preview.description.clone()),
                                        TextFont {
                                            font: fonts.body.clone(),
                                            font_size: 19.0,
                                            ..default()
                                        },
                                        TextColor(theme::TEXT_LIGHT),
                                        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                                    ));
                                });
                            } else {
                                card.spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_grow: 1.0,
                                        border: UiRect::all(Val::Px(2.0)),
                                        ..default()
                                    },
                                    ImageNode::new(assets.load(req.portrait_path.clone())),
                                    BackgroundColor(Color::srgb(0.03, 0.04, 0.06)),
                                    theme::border(false),
                                ));
                            }
                        });
                    });
            });
    });

    let char_count = req.text.chars().count().max(1) as f32;
    let speed = dialogue_speed.clamp(0.5, 2.0);
    let char_interval =
        (req.reveal_duration_secs.max(0.10) / char_count / speed).clamp(0.008, 0.070);

    DialogueSession {
        root,
        prompt_text,
        line_row,
        slot_text,
        text_chars: req.text.chars().collect(),
        options: req.options,
        selected_option: 0,
        revealed: 0,
        reveal_timer: 0.0,
        char_interval,
        slot_anim_timer: 0.0,
        slot_anim_dir: 0.0,
        preview_viewport,
        preview_pivot: preview_scene.as_ref().map(|scene| scene.pivot),
        preview_world_root: preview_scene.as_ref().map(|scene| scene.root),
        preview_model_root: preview_scene.as_ref().map(|scene| scene.model_root),
        preview_camera: preview_scene.as_ref().map(|scene| scene.camera),
        preview_framed: false,
        preview_dragging: false,
    }
}

struct PreviewSceneEntities {
    root: Entity,
    pivot: Entity,
    model_root: Entity,
    camera: Entity,
}

fn spawn_preview_world(
    commands: &mut Commands,
    assets: &AssetServer,
    images: &mut Assets<Image>,
    model_path: &str,
) -> Option<PreviewSceneEntities> {
    if model_path.trim().is_empty() {
        return None;
    }

    let target = images.add(Image::new_target_texture(
        240,
        240,
        TextureFormat::Bgra8UnormSrgb,
        None,
    ));

    let model_handle: Handle<Scene> = assets.load(model_path.to_string());
    let root = commands
        .spawn((
            Name::new("Dialogue Preview Root"),
            Transform::default(),
            RenderLayers::layer(PREVIEW_RENDER_LAYER),
            DialoguePreviewLayerTagged,
        ))
        .id();
    let mut pivot = Entity::PLACEHOLDER;
    let mut model_root = Entity::PLACEHOLDER;
    let mut camera = Entity::PLACEHOLDER;
    commands.entity(root).with_children(|parent| {
        pivot = parent
            .spawn((
                Name::new("Dialogue Preview Pivot"),
                DialoguePreviewPivot,
                Transform::default(),
                RenderLayers::layer(PREVIEW_RENDER_LAYER),
                DialoguePreviewLayerTagged,
            ))
            .with_children(|pivot_parent| {
                model_root = pivot_parent
                    .spawn((
                    Name::new("Dialogue Preview Model"),
                    SceneRoot(model_handle),
                    Transform::from_scale(Vec3::splat(1.0)),
                    RenderLayers::layer(PREVIEW_RENDER_LAYER),
                    DialoguePreviewLayerTagged,
                ))
                    .id();
            })
            .id();

        parent.spawn((
            Name::new("Dialogue Preview Light"),
            PointLight {
                intensity: 3_800_000.0,
                shadows_enabled: false,
                range: 30.0,
                ..default()
            },
            Transform::from_xyz(2.0, 2.6, 2.4),
            RenderLayers::layer(PREVIEW_RENDER_LAYER),
            DialoguePreviewLayerTagged,
        ));

        camera = parent
            .spawn((
                Name::new("Dialogue Preview Camera"),
                Camera3d::default(),
                Camera {
                    // Keep world-order so our camera compatibility shim won't force
                    // alpha-blend/no-clear output, which causes ghosting artifacts.
                    order: 0,
                    clear_color: ClearColorConfig::Custom(Color::srgb(0.02, 0.025, 0.03)),
                    ..default()
                },
                RenderTarget::Image(target.into()),
                Transform::from_xyz(0.0, 0.75, 2.8).looking_at(Vec3::new(0.0, 0.45, 0.0), Vec3::Y),
                RenderLayers::layer(PREVIEW_RENDER_LAYER),
                DialoguePreviewLayerTagged,
            ))
            .id();
    });

    Some(PreviewSceneEntities {
        root,
        pivot,
        model_root,
        camera,
    })
}

fn spawn_arrow_button(parent: &mut ChildSpawnerCommands, fonts: &UiFonts, dir: i32, label: &str) {
    parent
        .spawn((
            Button,
            DialogueArrowButton { dir },
            Node {
                width: Val::Px(34.0),
                min_width: Val::Px(34.0),
                max_width: Val::Px(34.0),
                height: Val::Percent(100.0),
                border: UiRect::all(Val::Px(2.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme::BUTTON_BG),
            theme::border(true),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextFont {
                    font: fonts.pixel.clone(),
                    font_size: 13.0,
                    ..default()
                },
                TextColor(theme::TEXT_DARK),
            ));
        });
}

fn reset_text(commands: &mut Commands, session: &mut DialogueSession, children: &Query<&Children>) {
    clear_row(commands, session.line_row, children);
    session.revealed = 0;
    session.reveal_timer = 0.0;
    session.selected_option = 0;
    refresh_slot_text(commands, session);
    refresh_prompt(commands, session);
}

fn reveal_all_chars(
    commands: &mut Commands,
    session: &mut DialogueSession,
    fonts: &UiFonts,
    children: &Query<&Children>,
) {
    clear_row(commands, session.line_row, children);
    for ch in &session.text_chars {
        spawn_char(commands, session.line_row, *ch, fonts);
    }
    session.revealed = session.text_chars.len();
}

fn cycle_option(commands: &mut Commands, session: &mut DialogueSession, dir: i32) {
    if session.options.len() < 2 {
        return;
    }
    let len = session.options.len() as i32;
    let idx = session.selected_option as i32;
    session.selected_option = ((idx + dir).rem_euclid(len)) as usize;
    session.slot_anim_timer = 0.16;
    session.slot_anim_dir = dir.signum() as f32;
    refresh_slot_text(commands, session);
    refresh_prompt(commands, session);
}

fn refresh_slot_text(commands: &mut Commands, session: &DialogueSession) {
    let text = if let Some(option) = session.options.get(session.selected_option) {
        format!(
            "{} / {}  {}",
            session.selected_option + 1,
            session.options.len(),
            option.text
        )
    } else {
        "no choices".to_string()
    };
    commands.entity(session.slot_text).insert(Text::new(text));
}

fn refresh_prompt(commands: &mut Commands, session: &DialogueSession) {
    if session.revealed < session.text_chars.len() {
        update_prompt(
            commands,
            session.prompt_text,
            "press e, enter, or click to skip",
        );
    } else if session.options.is_empty() {
        update_prompt(
            commands,
            session.prompt_text,
            "press e, enter, or click to continue",
        );
    } else {
        update_prompt(
            commands,
            session.prompt_text,
            "a/d or arrows to switch, enter to choose",
        );
    }
}

fn spawn_char(commands: &mut Commands, row: Entity, ch: char, fonts: &UiFonts) {
    commands.entity(row).with_children(|line| {
        if ch == '\n' {
            line.spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(0.0),
                ..default()
            });
            return;
        }

        line.spawn((
            Node::default(),
            Text::new(ch.to_string()),
            TextFont {
                font: fonts.body.clone(),
                font_size: 33.0,
                ..default()
            },
            TextColor(theme::TEXT_LIGHT),
            UiTransform::IDENTITY,
            DialogueGlyphFx {
                age: 0.0,
                duration: 0.13,
                amplitude: 8.0,
            },
        ));
    });
}

fn update_prompt(commands: &mut Commands, entity: Entity, value: &str) {
    commands.entity(entity).insert(Text::new(value));
}

fn close_session(
    commands: &mut Commands,
    runtime: &mut UiDialogueRuntime,
    state: &mut UiDialogueState,
    children: &Query<&Children>,
) {
    if let Some(session) = runtime.session.take() {
        if let Some(preview_root) = session.preview_world_root {
            despawn_tree(commands, preview_root, children);
        }
        despawn_tree(commands, session.root, children);
    }
    state.active = false;
}

fn clear_row(commands: &mut Commands, row: Entity, children: &Query<&Children>) {
    if let Ok(row_children) = children.get(row) {
        for child in row_children.iter() {
            despawn_tree(commands, child, children);
        }
    }
}

fn despawn_tree(commands: &mut Commands, root: Entity, children_query: &Query<&Children>) {
    if let Ok(children) = children_query.get(root) {
        for child in children.iter() {
            despawn_tree(commands, child, children_query);
        }
    }
    commands.entity(root).despawn();
}

fn any_pressed(keys: &ButtonInput<KeyCode>, options: &[KeyCode]) -> bool {
    options.iter().any(|key| keys.just_pressed(*key))
}

fn is_cursor_over_preview(
    session: &DialogueSession,
    windows: &Query<&Window, With<PrimaryWindow>>,
    viewports: &Query<(&ComputedNode, &UiGlobalTransform), With<DialoguePreviewViewport>>,
) -> bool {
    let Some(viewport) = session.preview_viewport else {
        return false;
    };
    let Ok(window) = windows.single() else {
        return false;
    };
    let Some(cursor) = window.cursor_position() else {
        return false;
    };
    let Ok((node, transform)) = viewports.get(viewport) else {
        return false;
    };
    let (_, _, translation) = transform.to_scale_angle_translation();
    let half = node.size() * 0.5;
    let min = translation - half;
    let max = translation + half;
    cursor.x >= min.x && cursor.x <= max.x && cursor.y >= min.y && cursor.y <= max.y
}
