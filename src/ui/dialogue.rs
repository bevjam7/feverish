use bevy::{
    camera::{RenderTarget, primitives::Aabb, visibility::RenderLayers},
    input::mouse::{MouseMotion, MouseWheel},
    picking::hover::HoverMap,
    prelude::*,
    render::render_resource::TextureFormat,
    text::{Justify, LineBreak, TextLayout},
    ui::{FocusPolicy, widget::ViewportNode},
    window::PrimaryWindow,
};

use super::{
    components::{
        DialogueUiRoot, UiDialogueCommand, UiDialogueMode, UiDialogueOption, UiDialoguePreview,
        UiDialogueRequest, UiDiscoveryCommand,
    },
    systems::{UiDiscoveryDb, UiFonts},
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
    mode: UiDialogueMode,
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
    preview_label: Entity,
    preview_card_root: Entity,
    active_preview: Option<super::components::UiDialoguePreview>,
    preview_pivot: Option<Entity>,
    preview_world_root: Option<Entity>,
    preview_model_root: Option<Entity>,
    preview_camera: Option<Entity>,
    preview_framed: bool,
    preview_dragging: bool,
    scroll_hint: Entity,
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

#[derive(Component, Debug, Clone, Copy)]
pub(super) struct DialogueQuickActionButton {
    option_index: usize,
}

#[derive(Component)]
pub(super) struct DialogueConfirmButton;

#[derive(Component)]
pub(super) struct DialogueOptionSlot;

#[derive(Component)]
pub(super) struct DialoguePreviewViewport;

#[derive(Component)]
pub(super) struct DialoguePreviewPivot;

#[derive(Component)]
pub(super) struct DialoguePreviewLayerTagged;

#[derive(Component)]
pub(super) struct DialogueTextScrollArea;

#[derive(Component)]
pub(super) struct DialogueTextScrollHint;

const PREVIEW_RENDER_LAYER: usize = 19;
const DIALOGUE_SCROLL_HINT_THRESHOLD: usize = 95;
const DIALOGUE_SLOT_FONT_MAX: f32 = 27.0;
const DIALOGUE_SLOT_FONT_MIN: f32 = 17.0;
const DIALOGUE_SLOT_HARD_LIMIT: usize = 132;
const DIALOGUE_SLOT_MAX_LINES: usize = 2;
const DIALOGUE_SLOT_ESTIMATED_WIDTH: f32 = 610.0;
const DIALOGUE_SLOT_CHAR_WIDTH_FACTOR: f32 = 0.56;
const DIALOGUE_SLOT_CONTENT_HEIGHT: f32 = 44.0;
const DIALOGUE_SLOT_LINE_HEIGHT_FACTOR: f32 = 1.22;

pub(super) fn apply_dialogue_commands(
    mut commands: Commands,
    mut msgs: MessageReader<UiDialogueCommand>,
    mut runtime: ResMut<UiDialogueRuntime>,
    mut state: ResMut<UiDialogueState>,
    fonts: Res<UiFonts>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    settings: Res<GameSettings>,
    discovery_db: Res<UiDiscoveryDb>,
    children: Query<&Children>,
) {
    for msg in msgs.read() {
        match msg {
            UiDialogueCommand::OpenInventory => {
                close_session(&mut commands, &mut runtime, &mut state, &children);
                let mut session = spawn_dialogue(
                    &mut commands,
                    &fonts,
                    &assets,
                    &mut images,
                    inventory_request_from_db(&discovery_db),
                    settings.dialogue_speed,
                );
                reset_text(&mut commands, &mut session, &fonts, &children);
                state.active = true;
                runtime.session = Some(session);
            }
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
                reset_text(&mut commands, &mut session, &fonts, &children);
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
    hover_map: Res<HoverMap>,
    mut commands: Commands,
) {
    if !dialogue_state.active || !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let Some(session) = runtime.session.as_ref() else {
        return;
    };
    if is_cursor_over_preview(session, &hover_map) {
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
    mut mouse_wheel: MessageReader<MouseWheel>,
    windows: Query<&Window, With<PrimaryWindow>>,
    hover_map: Res<HoverMap>,
    mut runtime: ResMut<UiDialogueRuntime>,
    mut pivots: Query<&mut Transform, With<DialoguePreviewPivot>>,
    mut preview_cameras: Query<
        (&GlobalTransform, &Projection, &mut Transform),
        Without<DialoguePreviewPivot>,
    >,
    preview_nodes: Query<
        (
            &Node,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
        ),
        With<DialoguePreviewViewport>,
    >,
    children: Query<&Children>,
    aabbs: Query<(&Aabb, &GlobalTransform)>,
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
    let wheel_delta: f32 = mouse_wheel.read().map(|event| event.y).sum();

    let cursor_pos = windows
        .single()
        .ok()
        .and_then(|window| window.cursor_position());
    let hovered = cursor_pos
        .is_some_and(|cursor| cursor_inside_preview(session, cursor, &preview_nodes))
        || is_cursor_over_preview(session, &hover_map);
    if hovered && wheel_delta.abs() > f32::EPSILON {
        if let Some(cursor_pos) = cursor_pos {
            apply_preview_zoom(
                session,
                wheel_delta,
                cursor_pos,
                &preview_nodes,
                &mut preview_cameras,
            );
        }
    }

    if session.preview_dragging && mouse.pressed(MouseButton::Left) {
        if drag_delta.length_squared() > f32::EPSILON {
            pivot_transform.rotate_y(-drag_delta.x * 0.012);
            pivot_transform.rotate_local_x(-drag_delta.y * 0.010);
        }
        return;
    }

    if hovered && mouse.just_pressed(MouseButton::Left) {
        session.preview_dragging = cursor_pos.is_some_and(|cursor| {
            preview_model_hit_test(
                session,
                cursor,
                &preview_nodes,
                &mut preview_cameras,
                &children,
                &aabbs,
            )
        });
        if !session.preview_dragging {
            return;
        }

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
            *camera_transform = Transform::from_translation(eye)
                .looking_at(Vec3::new(0.0, radius * 0.20, 0.0), Vec3::Y);
        }
    }

    session.preview_framed = true;
}

pub(super) fn handle_dialogue_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<UiDialogueRuntime>,
    dialogue_state: Res<UiDialogueState>,
    mut commands: Commands,
    fonts: Res<UiFonts>,
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

    if session.mode == UiDialogueMode::Inventory
        && apply_inventory_hotkeys(&keys, &mut commands, session, &fonts)
    {
        return;
    }

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
        cycle_option(&mut commands, session, -1, &fonts);
    }
    if any_pressed(&keys, &[KeyCode::ArrowRight, KeyCode::KeyD]) {
        cycle_option(&mut commands, session, 1, &fonts);
    }
    if any_pressed(&keys, &[KeyCode::KeyE, KeyCode::Enter, KeyCode::Space]) {
        if session.mode == UiDialogueMode::Inventory {
            if let Some(item_idx) = selected_item_option_index(session) {
                commands.write_message(RatCommand::Choose(item_idx));
            } else {
                commands.write_message(RatCommand::Choose(session.selected_option));
            }
        } else {
            commands.write_message(RatCommand::Choose(session.selected_option));
        }
    }
}

fn apply_inventory_hotkeys(
    keys: &ButtonInput<KeyCode>,
    commands: &mut Commands,
    session: &mut DialogueSession,
    fonts: &UiFonts,
) -> bool {
    if keys.just_pressed(KeyCode::KeyQ) || keys.just_pressed(KeyCode::Delete) {
        return drop_selected_inventory_item(commands, session, fonts);
    }
    false
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
    fonts: Res<UiFonts>,
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
                    cycle_option(&mut commands, session, arrow.dir, &fonts);
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

pub(super) fn handle_dialogue_quick_action_buttons(
    mut interactions: Query<
        (
            &Interaction,
            &DialogueQuickActionButton,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        Changed<Interaction>,
    >,
    runtime: Res<UiDialogueRuntime>,
    mut commands: Commands,
) {
    let Some(session) = runtime.session.as_ref() else {
        return;
    };

    for (interaction, action, mut bg, mut border) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(theme::BUTTON_BG);
                *border = theme::border(false);
                if session.revealed >= session.text_chars.len()
                    && action.option_index < session.options.len()
                {
                    commands.write_message(RatCommand::Choose(action.option_index));
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

pub(super) fn handle_dialogue_confirm_button(
    mut interactions: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (Changed<Interaction>, With<DialogueConfirmButton>),
    >,
    runtime: Res<UiDialogueRuntime>,
    mut commands: Commands,
) {
    let Some(session) = runtime.session.as_ref() else {
        return;
    };

    for (interaction, mut bg, mut border) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(theme::BUTTON_BG);
                *border = theme::border(false);
                if session.revealed >= session.text_chars.len()
                    && session.selected_option < session.options.len()
                {
                    commands.write_message(RatCommand::Choose(session.selected_option));
                }
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(theme::BUTTON_HOVER);
                *border = theme::border(true);
            }
            Interaction::None => {
                *bg = BackgroundColor(Color::srgb(0.08, 0.10, 0.13));
                *border = theme::border(true);
            }
        }
    }
}

pub(super) fn sync_picker_preview_from_selection(
    mut commands: Commands,
    fonts: Res<UiFonts>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    children: Query<&Children>,
    mut runtime: ResMut<UiDialogueRuntime>,
) {
    let Some(session) = runtime.session.as_mut() else {
        return;
    };

    if !session
        .options
        .iter()
        .any(|option| option.preview.is_some())
    {
        return;
    }

    let Some(option) = session.options.get(session.selected_option) else {
        return;
    };
    let Some(preview) = option.preview.clone() else {
        return;
    };

    if session.active_preview.as_ref() == Some(&preview) {
        return;
    }

    apply_preview_to_right_panel(
        &mut commands,
        session,
        &fonts,
        &assets,
        &mut images,
        &children,
        preview,
    );
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

pub(super) fn update_dialogue_text_scroll_hint(
    runtime: Res<UiDialogueRuntime>,
    mut hints: Query<&mut Text, With<DialogueTextScrollHint>>,
    scroll_areas: Query<&ScrollPosition, With<DialogueTextScrollArea>>,
) {
    let Some(session) = runtime.session.as_ref() else {
        return;
    };
    let Ok(mut hint_text) = hints.get_mut(session.scroll_hint) else {
        return;
    };
    let Ok(scroll) = scroll_areas.get(session.line_row) else {
        return;
    };

    if session.text_chars.len() <= DIALOGUE_SCROLL_HINT_THRESHOLD {
        hint_text.0.clear();
        return;
    }

    hint_text.0 = if scroll.y > 1.0 {
        "↑↓".to_string()
    } else {
        "↓".to_string()
    };
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
    let has_quick_actions = has_quick_actions(&req.options);
    let line_height = if has_quick_actions { 88.0 } else { 118.0 };
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
    let mut quick_actions_row = Entity::PLACEHOLDER;
    let mut scroll_hint = Entity::PLACEHOLDER;
    let mut preview_label = Entity::PLACEHOLDER;
    let mut preview_card_root = Entity::PLACEHOLDER;
    let mut preview_viewport = None;

    commands.entity(root).with_children(|overlay| {
        overlay
            .spawn((
                Name::new("Dialogue Frame"),
                Node {
                    width: Val::Px(940.0),
                    height: Val::Px(272.0),
                    min_height: Val::Px(272.0),
                    max_height: Val::Px(272.0),
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
                        Name::new("Dialogue Text Panel"),
                        Node {
                            flex_grow: 1.0,
                            min_width: Val::Px(0.0),
                            border: UiRect::all(Val::Px(2.0)),
                            padding: UiRect::all(Val::Px(10.0)),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.0),
                            overflow: Overflow::clip(),
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
                                    height: Val::Px(line_height),
                                    min_height: Val::Px(line_height),
                                    max_height: Val::Px(line_height),
                                    flex_wrap: FlexWrap::Wrap,
                                    align_content: AlignContent::FlexStart,
                                    overflow: Overflow::scroll_y(),
                                    padding: UiRect::right(Val::Px(8.0)),
                                    ..default()
                                },
                                DialogueTextScrollArea,
                                ScrollPosition(Vec2::ZERO),
                                BackgroundColor(Color::NONE),
                            ))
                            .id();

                        scroll_hint = left
                            .spawn((
                                DialogueTextScrollHint,
                                Text::new(""),
                                TextFont {
                                    font: fonts.body.clone(),
                                    font_size: 13.0,
                                    ..default()
                                },
                                TextColor(Color::srgba(0.82, 0.82, 0.88, 0.75)),
                                TextLayout::new(Justify::Right, LineBreak::NoWrap),
                                Node {
                                    width: Val::Percent(100.0),
                                    min_height: Val::Px(13.0),
                                    ..default()
                                },
                            ))
                            .id();

                        left.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(48.0),
                                min_height: Val::Px(48.0),
                                max_height: Val::Px(48.0),
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
                                    Button,
                                    DialogueConfirmButton,
                                    Node {
                                        flex_grow: 1.0,
                                        height: Val::Percent(100.0),
                                        border: UiRect::all(Val::Px(2.0)),
                                        padding: UiRect::horizontal(Val::Px(6.0)),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        overflow: Overflow::clip(),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.08, 0.10, 0.13)),
                                    theme::border(true),
                                ))
                                .with_children(|slot| {
                                    slot_text = slot
                                        .spawn((
                                            DialogueOptionSlot,
                                            Node {
                                                width: Val::Percent(100.0),
                                                max_width: Val::Percent(100.0),
                                                ..default()
                                            },
                                            Text::new("no choices"),
                                            TextFont {
                                                font: fonts.body.clone(),
                                                font_size: DIALOGUE_SLOT_FONT_MAX,
                                                ..default()
                                            },
                                            TextColor(theme::TEXT_LIGHT),
                                            TextLayout::new(
                                                Justify::Center,
                                                LineBreak::WordBoundary,
                                            ),
                                            UiTransform::IDENTITY,
                                        ))
                                        .id();
                                });

                            spawn_arrow_button(selector, fonts, 1, ">");
                        });

                        quick_actions_row = left
                            .spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Px(30.0),
                                    min_height: Val::Px(30.0),
                                    max_height: Val::Px(30.0),
                                    column_gap: Val::Px(6.0),
                                    align_items: AlignItems::Center,
                                    display: if has_quick_actions {
                                        Display::Flex
                                    } else {
                                        Display::None
                                    },
                                    ..default()
                                },
                                BackgroundColor(Color::NONE),
                            ))
                            .id();

                        left.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(38.0),
                                min_height: Val::Px(38.0),
                                max_height: Val::Px(38.0),
                                border: UiRect::new(
                                    Val::Px(0.0),
                                    Val::Px(0.0),
                                    Val::Px(2.0),
                                    Val::Px(0.0),
                                ),
                                align_items: AlignItems::Center,
                                padding: UiRect::new(
                                    Val::Px(2.0),
                                    Val::Px(2.0),
                                    Val::Px(0.0),
                                    Val::Px(2.0),
                                ),
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
                                        font_size: 16.0,
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.84, 0.84, 0.88)),
                                    TextLayout::new(Justify::Left, LineBreak::NoWrap),
                                ))
                                .id();
                        });
                    });

                frame
                    .spawn((
                        Name::new("Dialogue Preview Panel"),
                        Node {
                            width: Val::Px(236.0),
                            min_width: Val::Px(236.0),
                            max_width: Val::Px(236.0),
                            border: UiRect::all(Val::Px(2.0)),
                            padding: UiRect::all(Val::Px(7.0)),
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
                        preview_label = right
                            .spawn((
                                Name::new("Dialogue Preview Label"),
                                Text::new(label),
                                TextFont {
                                    font: fonts.pixel.clone(),
                                    font_size: 13.0,
                                    ..default()
                                },
                                TextColor(theme::TEXT_LIGHT),
                                TextLayout::new(Justify::Center, LineBreak::NoWrap),
                            ))
                            .id();

                        preview_card_root = right
                            .spawn((
                                Name::new("Dialogue Preview Card Root"),
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
                                    spawn_preview_card_contents(
                                        card,
                                        fonts,
                                        assets,
                                        preview,
                                        preview_scene.as_ref(),
                                        &mut preview_viewport,
                                    );
                                } else {
                                    card.spawn((
                                        Node {
                                            width: Val::Percent(100.0),
                                            flex_grow: 1.0,
                                            border: UiRect::all(Val::Px(2.0)),
                                            ..default()
                                        },
                                        ImageNode::new(
                                            assets.load::<Image>(req.portrait_path.clone()),
                                        ),
                                        BackgroundColor(Color::srgb(0.03, 0.04, 0.06)),
                                        theme::border(false),
                                    ));
                                }
                            })
                            .id();
                    });
            });
    });

    commands
        .entity(quick_actions_row)
        .with_children(|row| spawn_quick_action_buttons(row, fonts, &req.options));

    let char_count = req.text.chars().count().max(1) as f32;
    let speed = dialogue_speed.clamp(0.5, 2.0);
    let char_interval =
        (req.reveal_duration_secs.max(0.10) / char_count / speed).clamp(0.008, 0.070);

    DialogueSession {
        mode: req.mode,
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
        preview_label,
        preview_card_root,
        active_preview: req.preview,
        preview_pivot: preview_scene.as_ref().map(|scene| scene.pivot),
        preview_world_root: preview_scene.as_ref().map(|scene| scene.root),
        preview_model_root: preview_scene.as_ref().map(|scene| scene.model_root),
        preview_camera: preview_scene.as_ref().map(|scene| scene.camera),
        preview_framed: false,
        preview_dragging: false,
        scroll_hint,
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
                Projection::Perspective(PerspectiveProjection {
                    near: 0.12,
                    far: 80.0,
                    ..default()
                }),
                Msaa::Off, // msaa off because its apparently bad?
                Camera {
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
                    font_size: 14.0,
                    ..default()
                },
                TextColor(theme::TEXT_DARK),
            ));
        });
}

fn spawn_preview_card_contents(
    card: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    assets: &AssetServer,
    preview: &super::components::UiDialoguePreview,
    preview_scene: Option<&PreviewSceneEntities>,
    preview_viewport: &mut Option<Entity>,
) {
    if let Some(preview_scene) = preview_scene {
        *preview_viewport = Some(
            card.spawn((
                Name::new("Dialogue Preview Viewport"),
                DialoguePreviewViewport,
                Interaction::default(),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(116.0),
                    min_height: Val::Px(116.0),
                    max_height: Val::Px(116.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                ViewportNode::new(preview_scene.camera),
                BackgroundColor(Color::srgb(0.03, 0.04, 0.06)),
                theme::border(false),
            ))
            .id(),
        );
    } else if let Some(image_path) = preview.image_path.as_ref() {
        card.spawn((
            Name::new("Dialogue Preview Image"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(116.0),
                min_height: Val::Px(116.0),
                max_height: Val::Px(116.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            ImageNode::new(assets.load::<Image>(image_path.clone())),
            BackgroundColor(Color::srgb(0.03, 0.04, 0.06)),
            theme::border(false),
        ));
    } else {
        card.spawn((
            Name::new("Dialogue Preview Placeholder"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(116.0),
                min_height: Val::Px(116.0),
                max_height: Val::Px(116.0),
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
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(0.60, 0.62, 0.68)),
            ));
        });
    }

    card.spawn((
        Name::new("Dialogue Preview Description Scroll"),
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
            Name::new("Dialogue Preview Description Wrap"),
            Node {
                width: Val::Percent(100.0),
                max_width: Val::Percent(100.0),
                min_height: Val::Px(1.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|wrap| {
            wrap.spawn((
                Name::new("Dialogue Preview Description"),
                Node {
                    width: Val::Percent(100.0),
                    ..default()
                },
                Text::new(preview.description.clone()),
                TextFont {
                    font: fonts.body.clone(),
                    font_size: 20.0,
                    ..default()
                },
                TextColor(theme::TEXT_LIGHT),
                TextLayout::new(Justify::Left, LineBreak::WordBoundary),
            ));
        });
    });
}

fn spawn_quick_action_buttons(
    row: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    options: &[UiDialogueOption],
) {
    for (idx, option) in options.iter().enumerate() {
        let Some(label) = quick_action_label(&option.text) else {
            continue;
        };
        row.spawn((
            Button,
            DialogueQuickActionButton { option_index: idx },
            Node {
                height: Val::Px(28.0),
                border: UiRect::all(Val::Px(2.0)),
                padding: UiRect::horizontal(Val::Px(8.0)),
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
                    font_size: 11.0,
                    ..default()
                },
                TextColor(theme::TEXT_DARK),
            ));
        });
    }
}

fn quick_action_label(text: &str) -> Option<&'static str> {
    let normalized = text.trim().to_ascii_lowercase();
    if normalized.contains("show item") {
        Some("show item")
    } else if normalized.contains("leave") || normalized.contains("go.") {
        Some("leave")
    } else {
        None
    }
}

fn has_quick_actions(options: &[UiDialogueOption]) -> bool {
    options
        .iter()
        .any(|option| quick_action_label(&option.text).is_some())
}

fn reset_text(
    commands: &mut Commands,
    session: &mut DialogueSession,
    fonts: &UiFonts,
    children: &Query<&Children>,
) {
    clear_row(commands, session.line_row, children);
    commands
        .entity(session.line_row)
        .insert(ScrollPosition(Vec2::ZERO));
    session.revealed = 0;
    session.reveal_timer = 0.0;
    session.selected_option = 0;
    refresh_slot_text(commands, session, fonts);
    refresh_prompt(commands, session);
}

fn reveal_all_chars(
    commands: &mut Commands,
    session: &mut DialogueSession,
    fonts: &UiFonts,
    children: &Query<&Children>,
) {
    clear_row(commands, session.line_row, children);
    commands
        .entity(session.line_row)
        .insert(ScrollPosition(Vec2::ZERO));
    for ch in &session.text_chars {
        spawn_char(commands, session.line_row, *ch, fonts);
    }
    session.revealed = session.text_chars.len();
}

fn cycle_option(commands: &mut Commands, session: &mut DialogueSession, dir: i32, fonts: &UiFonts) {
    if session.options.len() < 2 {
        return;
    }
    let len = session.options.len() as i32;
    let idx = session.selected_option as i32;
    session.selected_option = ((idx + dir).rem_euclid(len)) as usize;
    session.slot_anim_timer = 0.16;
    session.slot_anim_dir = dir.signum() as f32;
    refresh_slot_text(commands, session, fonts);
    refresh_prompt(commands, session);
}

fn refresh_slot_text(commands: &mut Commands, session: &DialogueSession, fonts: &UiFonts) {
    let base = if let Some(option) = session.options.get(session.selected_option) {
        let seen_suffix = if session.mode == UiDialogueMode::Inventory && option.seen {
            " [shown]"
        } else {
            ""
        };
        format!(
            "{} / {}  {}{}",
            session.selected_option + 1,
            session.options.len(),
            option.text,
            seen_suffix
        )
    } else {
        "no choices".to_string()
    };
    let fitted_font = slot_font_size(&base);
    let displayed = slot_label_with_ellipsis(&base, fitted_font);
    commands
        .entity(session.slot_text)
        .insert(Text::new(displayed));
    commands.entity(session.slot_text).insert(TextFont {
        font: fonts.body.clone(),
        font_size: fitted_font,
        ..default()
    });
}

fn slot_font_size(label: &str) -> f32 {
    let mut size = DIALOGUE_SLOT_FONT_MAX;
    while size > DIALOGUE_SLOT_FONT_MIN && !slot_text_fits(label, size) {
        size -= 1.0;
    }
    size.clamp(DIALOGUE_SLOT_FONT_MIN, DIALOGUE_SLOT_FONT_MAX)
}

fn slot_label_with_ellipsis(label: &str, font_size: f32) -> String {
    let clamped: String = label.chars().take(DIALOGUE_SLOT_HARD_LIMIT).collect();
    if clamped.chars().count() == label.chars().count() && slot_text_fits(&clamped, font_size) {
        return clamped;
    }

    let mut truncated = String::new();
    for ch in clamped.chars() {
        truncated.push(ch);
        if !slot_text_fits(&truncated, font_size) {
            truncated.pop();
            break;
        }
    }

    if truncated.trim().is_empty() {
        return "…".to_string();
    }

    if let Some((idx, _)) = truncated
        .char_indices()
        .rev()
        .find(|(_, ch)| ch.is_whitespace())
    {
        let word_boundary = truncated[..idx].trim_end();
        if !word_boundary.is_empty() {
            truncated = word_boundary.to_string();
        }
    } else {
        truncated = truncated.trim_end().to_string();
    }

    if truncated.is_empty() {
        return "…".to_string();
    }

    while !truncated.is_empty() {
        let candidate = format!("{truncated}…");
        if slot_text_fits(&candidate, font_size) {
            return candidate;
        }
        truncated.pop();
        truncated = truncated.trim_end().to_string();
    }

    if slot_text_fits("…", font_size) {
        "…".to_string()
    } else {
        label.to_string()
    }
}

fn slot_text_fits(label: &str, font_size: f32) -> bool {
    let line_capacity = slot_line_capacity(font_size);
    let wrapped_lines = estimated_wrapped_lines(label, line_capacity);
    if wrapped_lines > DIALOGUE_SLOT_MAX_LINES {
        return false;
    }

    let required_height =
        (wrapped_lines as f32 * font_size * DIALOGUE_SLOT_LINE_HEIGHT_FACTOR).ceil();
    required_height <= DIALOGUE_SLOT_CONTENT_HEIGHT
}

fn slot_line_capacity(font_size: f32) -> usize {
    ((DIALOGUE_SLOT_ESTIMATED_WIDTH / (font_size * DIALOGUE_SLOT_CHAR_WIDTH_FACTOR))
        .floor()
        .max(8.0)) as usize
}

fn estimated_wrapped_lines(label: &str, line_capacity: usize) -> usize {
    let capacity = line_capacity.max(1);
    let mut total_lines = 0usize;

    for paragraph in label.split('\n') {
        let trimmed = paragraph.trim();
        if trimmed.is_empty() {
            total_lines += 1;
            continue;
        }

        let mut paragraph_lines = 1usize;
        let mut line_len = 0usize;

        for word in trimmed.split_whitespace() {
            let word_len = word.chars().count().max(1);

            if line_len == 0 {
                if word_len <= capacity {
                    line_len = word_len;
                } else {
                    let chunks = word_len.div_ceil(capacity);
                    paragraph_lines += chunks.saturating_sub(1);
                    line_len = word_len % capacity;
                    if line_len == 0 {
                        line_len = capacity;
                    }
                }
                continue;
            }

            if line_len + 1 + word_len <= capacity {
                line_len += 1 + word_len;
                continue;
            }

            paragraph_lines += 1;
            if word_len <= capacity {
                line_len = word_len;
            } else {
                let chunks = word_len.div_ceil(capacity);
                paragraph_lines += chunks.saturating_sub(1);
                line_len = word_len % capacity;
                if line_len == 0 {
                    line_len = capacity;
                }
            }
        }

        total_lines += paragraph_lines;
    }

    total_lines.max(1)
}

fn refresh_prompt(commands: &mut Commands, session: &DialogueSession) {
    if session.mode == UiDialogueMode::Inventory {
        update_prompt(
            commands,
            session.prompt_text,
            "a/d: browse | enter: show | q: drop",
        );
        return;
    }

    let has_quick = has_quick_actions(&session.options);
    if session.revealed < session.text_chars.len() {
        update_prompt(commands, session.prompt_text, "e/enter/click: skip");
    } else if session.options.is_empty() {
        update_prompt(commands, session.prompt_text, "e/enter/click: continue");
    } else if session.options.len() == 1 {
        if has_quick {
            update_prompt(
                commands,
                session.prompt_text,
                "enter: choose | click: quick action",
            );
        } else {
            update_prompt(commands, session.prompt_text, "e/enter/click: continue");
        }
    } else if has_quick {
        update_prompt(
            commands,
            session.prompt_text,
            "a/d or arrows: switch | enter: choose | click: quick action",
        );
    } else {
        update_prompt(
            commands,
            session.prompt_text,
            "a/d or arrows: switch | enter: choose",
        );
    }
}

fn apply_preview_to_right_panel(
    commands: &mut Commands,
    session: &mut DialogueSession,
    fonts: &UiFonts,
    assets: &AssetServer,
    images: &mut Assets<Image>,
    children: &Query<&Children>,
    preview: super::components::UiDialoguePreview,
) {
    commands
        .entity(session.preview_label)
        .insert(Text::new(preview.title.clone()));

    if let Ok(card_children) = children.get(session.preview_card_root) {
        for child in card_children.iter() {
            despawn_tree(commands, child, children);
        }
    }

    if let Some(root) = session.preview_world_root.take() {
        despawn_tree(commands, root, children);
    }

    session.preview_viewport = None;
    session.preview_pivot = None;
    session.preview_model_root = None;
    session.preview_camera = None;
    session.preview_framed = false;
    session.preview_dragging = false;

    let preview_scene = preview
        .model_path
        .as_deref()
        .and_then(|path| spawn_preview_world(commands, assets, images, path));

    commands
        .entity(session.preview_card_root)
        .with_children(|card| {
            spawn_preview_card_contents(
                card,
                fonts,
                assets,
                &preview,
                preview_scene.as_ref(),
                &mut session.preview_viewport,
            );
        });

    session.preview_pivot = preview_scene.as_ref().map(|scene| scene.pivot);
    session.preview_world_root = preview_scene.as_ref().map(|scene| scene.root);
    session.preview_model_root = preview_scene.as_ref().map(|scene| scene.model_root);
    session.preview_camera = preview_scene.as_ref().map(|scene| scene.camera);
    session.active_preview = Some(preview);
}

fn spawn_char(commands: &mut Commands, row: Entity, ch: char, fonts: &UiFonts) {
    const DIALOGUE_BODY_FONT_SIZE: f32 = 34.0;

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
                font_size: DIALOGUE_BODY_FONT_SIZE,
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
    commands.queue(move |world: &mut World| {
        if world.entities().contains(root) {
            let _ = world.despawn(root);
        }
    });
}

fn any_pressed(keys: &ButtonInput<KeyCode>, options: &[KeyCode]) -> bool {
    options.iter().any(|key| keys.just_pressed(*key))
}

fn inventory_request_from_db(discovery_db: &UiDiscoveryDb) -> UiDialogueRequest {
    let mut options: Vec<UiDialogueOption> = discovery_db
        .entries(super::components::DiscoveryKind::Item)
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
            seen: entry.seen,
        })
        .collect();
    options.push(UiDialogueOption {
        text: "back".to_string(),
        preview: None,
        item_id: None,
        seen: false,
    });
    let preview = options.first().and_then(|option| option.preview.clone());

    UiDialogueRequest {
        mode: UiDialogueMode::Inventory,
        speaker: "inventory".to_string(),
        text: "pick an item to show".to_string(),
        portrait_path: "models/npc_a/npc_a.png".to_string(),
        preview,
        options,
        reveal_duration_secs: 0.0,
    }
}

fn selected_item_option_index(session: &DialogueSession) -> Option<usize> {
    session
        .options
        .get(session.selected_option)
        .and_then(|option| option.item_id.as_ref().map(|_| session.selected_option))
}

fn drop_selected_inventory_item(
    commands: &mut Commands,
    session: &mut DialogueSession,
    fonts: &UiFonts,
) -> bool {
    let Some(option) = session.options.get(session.selected_option) else {
        return false;
    };
    let Some(item_id) = option.item_id.as_ref() else {
        return false;
    };
    let item_id = item_id.clone();

    commands.write_message(UiDiscoveryCommand::DropItem {
        id: item_id.clone(),
    });
    let Some(removed_index) = session
        .options
        .iter()
        .position(|entry| entry.item_id.as_ref() == Some(&item_id))
    else {
        return true;
    };
    session.options.remove(removed_index);

    if session
        .options
        .last()
        .is_none_or(|option| option.item_id.is_some())
    {
        session.options.push(UiDialogueOption {
            text: "back".to_string(),
            preview: None,
            item_id: None,
            seen: false,
        });
    }

    let item_count = session
        .options
        .iter()
        .filter(|entry| entry.item_id.is_some())
        .count();
    session.selected_option = if item_count == 0 {
        session.options.len().saturating_sub(1)
    } else {
        session.selected_option.min(item_count.saturating_sub(1))
    };

    refresh_slot_text(commands, session, fonts);
    refresh_prompt(commands, session);
    session.active_preview = None;
    commands.write_message(UiDialogueCommand::OpenInventory);
    true
}

fn is_cursor_over_preview(session: &DialogueSession, hover_map: &HoverMap) -> bool {
    let Some(viewport) = session.preview_viewport else {
        return false;
    };
    hover_map
        .values()
        .any(|pointer_map| pointer_map.contains_key(&viewport))
}

fn apply_preview_zoom(
    session: &DialogueSession,
    wheel_delta: f32,
    cursor_pos: Vec2,
    preview_nodes: &Query<
        (
            &Node,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
        ),
        With<DialoguePreviewViewport>,
    >,
    preview_cameras: &mut Query<
        (&GlobalTransform, &Projection, &mut Transform),
        Without<DialoguePreviewPivot>,
    >,
) {
    let Some((_, _, camera_entity)) =
        preview_cursor_ray(session, cursor_pos, preview_nodes, preview_cameras)
    else {
        return;
    };

    let Ok((_, _, mut camera_transform)) = preview_cameras.get_mut(camera_entity) else {
        return;
    };

    let focus = Vec3::ZERO;
    let current = camera_transform.translation - focus;
    let mut distance = current.length().max(0.05);
    let orbit_dir = current.normalize_or_zero();
    if orbit_dir.length_squared() <= f32::EPSILON {
        return;
    }
    let zoom_speed = (0.35 + distance * 0.28).clamp(0.2, 4.0);
    distance = (distance - wheel_delta * zoom_speed).clamp(0.06, 40.0);
    camera_transform.translation = focus + orbit_dir * distance;
    camera_transform.look_at(focus, Vec3::Y);
}

fn preview_model_hit_test(
    session: &DialogueSession,
    cursor_pos: Vec2,
    preview_nodes: &Query<
        (
            &Node,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
        ),
        With<DialoguePreviewViewport>,
    >,
    preview_cameras: &mut Query<
        (&GlobalTransform, &Projection, &mut Transform),
        Without<DialoguePreviewPivot>,
    >,
    children: &Query<&Children>,
    aabbs: &Query<(&Aabb, &GlobalTransform)>,
) -> bool {
    let Some((ray_origin, ray_dir, _)) =
        preview_cursor_ray(session, cursor_pos, preview_nodes, preview_cameras)
    else {
        return false;
    };
    let Some(model_root) = session.preview_model_root else {
        return false;
    };
    let Some((min, max)) = compute_preview_world_bounds(model_root, children, aabbs) else {
        // if we can raycast from inside the preview viewport, still allow drag start
        // this keeps tiny/thin meshes usable without pixel-perfect clicking on geometry
        return true;
    };

    let base_extent = (max - min).max_element().max(0.12);
    let padding = base_extent * 0.9;
    let expanded_min = min - Vec3::splat(padding);
    let expanded_max = max + Vec3::splat(padding);
    let _mesh_hit = ray_hits_aabb(ray_origin, ray_dir, expanded_min, expanded_max);

    // keep drag start permissive for tiny meshes: ray in viewport is enough
    true
}

fn preview_cursor_ray(
    session: &DialogueSession,
    cursor_pos: Vec2,
    preview_nodes: &Query<
        (
            &Node,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
        ),
        With<DialoguePreviewViewport>,
    >,
    preview_cameras: &mut Query<
        (&GlobalTransform, &Projection, &mut Transform),
        Without<DialoguePreviewPivot>,
    >,
) -> Option<(Vec3, Vec3, Entity)> {
    let (local_x, local_y, size) = preview_local_uv(session, cursor_pos, preview_nodes)?;

    let camera_entity = session.preview_camera?;
    let (camera_global, projection, _) = preview_cameras.get(camera_entity).ok()?;
    let Projection::Perspective(perspective) = projection else {
        return None;
    };

    let ndc_x = local_x * 2.0 - 1.0;
    let ndc_y = 1.0 - local_y * 2.0;
    let aspect = (size.x / size.y).max(0.001);
    let tan_half_fov = (perspective.fov * 0.5).tan().max(0.001);
    let dir_camera =
        Vec3::new(ndc_x * aspect * tan_half_fov, ndc_y * tan_half_fov, -1.0).normalize_or_zero();
    if dir_camera.length_squared() <= f32::EPSILON {
        return None;
    }

    let ray_dir = camera_global.rotation() * dir_camera;
    let ray_origin = camera_global.translation();
    Some((ray_origin, ray_dir, camera_entity))
}

fn preview_local_uv(
    session: &DialogueSession,
    cursor_pos: Vec2,
    preview_nodes: &Query<
        (
            &Node,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
        ),
        With<DialoguePreviewViewport>,
    >,
) -> Option<(f32, f32, Vec2)> {
    let viewport_entity = session.preview_viewport?;
    let (_, computed, ui_transform, visible) = preview_nodes.get(viewport_entity).ok()?;
    if !visible.get() {
        return None;
    }

    let size = computed.size();
    if size.x <= 1.0 || size.y <= 1.0 {
        return None;
    }

    let normalized = computed.normalize_point(*ui_transform, cursor_pos)?;
    if normalized.x < -0.5 || normalized.x > 0.5 || normalized.y < -0.5 || normalized.y > 0.5 {
        return None;
    }

    Some((normalized.x + 0.5, normalized.y + 0.5, size))
}

fn cursor_inside_preview(
    session: &DialogueSession,
    cursor_pos: Vec2,
    preview_nodes: &Query<
        (
            &Node,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
        ),
        With<DialoguePreviewViewport>,
    >,
) -> bool {
    preview_local_uv(session, cursor_pos, preview_nodes).is_some()
}

fn compute_preview_world_bounds(
    model_root: Entity,
    children: &Query<&Children>,
    aabbs: &Query<(&Aabb, &GlobalTransform)>,
) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut found = false;

    for entity in std::iter::once(model_root).chain(children.iter_descendants(model_root)) {
        let Ok((aabb, global)) = aabbs.get(entity) else {
            continue;
        };
        let max_scale = global.compute_transform().scale.max_element().max(0.001);
        let center = global.translation() + Vec3::from(aabb.center);
        let radius = aabb.half_extents.max_element() * max_scale;
        let extent = Vec3::splat(radius);
        min = min.min(center - extent);
        max = max.max(center + extent);
        found = true;
    }

    found.then_some((min, max))
}

fn ray_hits_aabb(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> bool {
    let mut tmin = 0.0_f32;
    let mut tmax = f32::INFINITY;

    for axis in 0..3 {
        let o = origin[axis];
        let d = dir[axis];
        let lo = min[axis];
        let hi = max[axis];

        if d.abs() < 1e-6 {
            if o < lo || o > hi {
                return false;
            }
            continue;
        }

        let inv_d = 1.0 / d;
        let mut t1 = (lo - o) * inv_d;
        let mut t2 = (hi - o) * inv_d;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }

        tmin = tmin.max(t1);
        tmax = tmax.min(t2);
        if tmin > tmax {
            return false;
        }
    }

    tmax >= 0.0
}
