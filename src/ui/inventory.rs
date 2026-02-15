use bevy::{
    camera::{RenderTarget, primitives::Aabb, visibility::RenderLayers},
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
    render::render_resource::TextureFormat,
    text::{Justify, LineBreak, TextLayout},
    ui::{ComputedNode, FocusPolicy, UiGlobalTransform, UiScale, widget::ViewportNode},
    window::PrimaryWindow,
};

use super::{
    components::{
        DialogueUiRoot, DiscoveryKind, InventoryUiRoot, MainMenuUi, MenuRoot, PauseMenuUi,
        UiDiscoveryCommand,
    },
    systems::UiDiscoveryDb,
    theme,
};

#[derive(Resource, Default)]
pub(super) struct UiInventoryRuntime {
    root: Option<Entity>,
    selected: usize,
    last_revision: u64,
    preview_viewport: Option<Entity>,
    preview_label: Option<Entity>,
    preview_card_root: Option<Entity>,
    preview_pivot: Option<Entity>,
    preview_world_root: Option<Entity>,
    preview_model_root: Option<Entity>,
    preview_camera: Option<Entity>,
    preview_framed: bool,
    preview_dragging: bool,
    dragged_item_index: Option<usize>,
    dragged_item_original_index: Option<usize>,
    drag_start_cursor: Option<Vec2>,
    drag_visual_entity: Option<Entity>,
    drop_zone_active: bool,
    preview_image_target: Option<Handle<Image>>,
}

const INVENTORY_PREVIEW_RENDER_LAYER: usize = 20;
const INVENTORY_PANEL_WIDTH: f32 = 860.0;
const INVENTORY_PANEL_HEIGHT: f32 = 500.0;
const INVENTORY_DROP_ZONE_MARGIN: f32 = 72.0;
const INVENTORY_DRAG_START_THRESHOLD: f32 = 8.0;
const INVENTORY_GHOST_SHRINK: f32 = 0.82;

#[derive(Component)]
pub(super) struct InventoryPreviewViewport;

#[derive(Component)]
pub(super) struct InventoryPreviewPivot;

#[derive(Component)]
pub(super) struct InventoryPreviewLayerTagged;

#[derive(Component, Debug, Clone)]
pub(super) struct InventoryItemSlot {
    pub index: usize,
}

#[derive(Component)]
pub(super) struct InventoryPanelFrame;

#[derive(Component)]
pub(super) struct InventoryDragGhost;

#[derive(Message, Debug, Clone, Copy)]
pub(super) enum UiInventoryCommand {
    Toggle,
    Open,
    Close,
}

#[derive(Component, Debug, Clone)]
pub(super) struct InventoryItemButton {
    pub index: usize,
    pub item_id: String,
}

pub(super) fn handle_inventory_tab_shortcut(
    keys: Res<ButtonInput<KeyCode>>,
    dialogue_state: Res<super::dialogue::UiDialogueState>,
    main_menu: Query<(), With<MainMenuUi>>,
    pause_menu: Query<(), With<PauseMenuUi>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::Tab) {
        return;
    }
    if dialogue_state.active {
        return;
    }
    if !main_menu.is_empty() || !pause_menu.is_empty() {
        return;
    }
    commands.write_message(UiInventoryCommand::Toggle);
}

pub(super) fn close_inventory_when_blocked(
    runtime: Res<UiInventoryRuntime>,
    dialogue_ui: Query<(), With<DialogueUiRoot>>,
    menu_roots: Query<(), With<MenuRoot>>,
    mut commands: Commands,
) {
    if runtime.root.is_none() {
        return;
    }
    if !dialogue_ui.is_empty() || !menu_roots.is_empty() {
        commands.write_message(UiInventoryCommand::Close);
    }
}

pub(super) fn apply_inventory_commands(
    mut commands: Commands,
    mut msgs: MessageReader<UiInventoryCommand>,
    mut runtime: ResMut<UiInventoryRuntime>,
    fonts: Res<super::systems::UiFonts>,
    discovery_db: Res<UiDiscoveryDb>,
    roots: Query<(), With<InventoryUiRoot>>,
    children: Query<&Children>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    for msg in msgs.read() {
        match msg {
            UiInventoryCommand::Toggle =>
                if runtime.root.is_some() {
                    close_inventory(&mut commands, &mut runtime, &roots, &children);
                } else {
                    open_inventory(
                        &mut commands,
                        &fonts,
                        &discovery_db,
                        &mut runtime,
                        &roots,
                        &children,
                        &assets,
                        &mut images,
                    );
                },
            UiInventoryCommand::Open => {
                open_inventory(
                    &mut commands,
                    &fonts,
                    &discovery_db,
                    &mut runtime,
                    &roots,
                    &children,
                    &assets,
                    &mut images,
                );
            }
            UiInventoryCommand::Close => {
                close_inventory(&mut commands, &mut runtime, &roots, &children);
            }
        }
    }
}

pub(super) fn handle_inventory_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<UiInventoryRuntime>,
    discovery_db: Res<UiDiscoveryDb>,
    mut commands: Commands,
) {
    if runtime.root.is_none() {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        commands.write_message(UiInventoryCommand::Close);
        return;
    }

    let items = discovery_db.entries(DiscoveryKind::Item);
    if items.is_empty() {
        return;
    }
    runtime.selected = runtime.selected.min(items.len().saturating_sub(1));

    let mut changed = false;
    if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::KeyA) {
        if runtime.selected > 0 {
            runtime.selected -= 1;
            changed = true;
        }
    }
    if keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::KeyD) {
        if runtime.selected + 1 < items.len() {
            runtime.selected += 1;
            changed = true;
        }
    }
    if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) {
        let prev = runtime.selected.saturating_sub(3);
        if prev != runtime.selected {
            runtime.selected = prev;
            changed = true;
        }
    }
    if keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) {
        let next = (runtime.selected + 3).min(items.len().saturating_sub(1));
        if next != runtime.selected {
            runtime.selected = next;
            changed = true;
        }
    }

    if keys.just_pressed(KeyCode::KeyQ) || keys.just_pressed(KeyCode::Delete) {
        if let Some(entry) = items.get(runtime.selected) {
            commands.write_message(UiDiscoveryCommand::DropItem {
                id: entry.id.clone(),
            });
            if runtime.selected >= items.len().saturating_sub(1) {
                runtime.selected = runtime.selected.saturating_sub(1);
            }
            commands.write_message(UiInventoryCommand::Open);
        }
        return;
    }

    if keys.just_pressed(KeyCode::KeyR) {
        if let Some(entry) = items.get(runtime.selected) {
            let target = runtime.selected.saturating_sub(1);
            if target != runtime.selected {
                commands.write_message(UiDiscoveryCommand::MoveItem {
                    id: entry.id.clone(),
                    to_index: target,
                });
                runtime.selected = target;
                commands.write_message(UiInventoryCommand::Open);
                return;
            }
        }
    }
    if keys.just_pressed(KeyCode::KeyF) {
        if let Some(entry) = items.get(runtime.selected) {
            let target = (runtime.selected + 1).min(items.len().saturating_sub(1));
            if target != runtime.selected {
                commands.write_message(UiDiscoveryCommand::MoveItem {
                    id: entry.id.clone(),
                    to_index: target,
                });
                runtime.selected = target;
                commands.write_message(UiInventoryCommand::Open);
                return;
            }
        }
    }

    if changed {
        commands.write_message(UiInventoryCommand::Open);
    }
}

pub(super) fn refresh_inventory_on_db_change(
    runtime: Res<UiInventoryRuntime>,
    discovery_db: Res<UiDiscoveryDb>,
    mut commands: Commands,
) {
    if runtime.root.is_none() {
        return;
    }
    if runtime.last_revision != discovery_db.revision() {
        commands.write_message(UiInventoryCommand::Open);
    }
}

pub(super) fn handle_inventory_item_interactions(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    ui_scale: Res<UiScale>,
    mut runtime: ResMut<UiInventoryRuntime>,
    mut button_query: Query<
        (
            Entity,
            &InventoryItemButton,
            &InventoryItemSlot,
            &Interaction,
            &mut BackgroundColor,
        ),
        With<Button>,
    >,
    windows: Query<&Window, With<PrimaryWindow>>,
    discovery_db: Res<UiDiscoveryDb>,
) {
    if runtime.root.is_none() {
        return;
    }

    let mouse_just_pressed = mouse.just_pressed(MouseButton::Left);
    let mouse_just_released = mouse.just_released(MouseButton::Left);
    let mouse_held = mouse.pressed(MouseButton::Left);

    let cursor_pos = windows
        .single()
        .ok()
        .and_then(|window| window.cursor_position());
    let drop_zone_active = cursor_pos.is_some_and(|cursor| {
        windows
            .single()
            .ok()
            .is_some_and(|window| cursor_in_drop_zone(cursor, window))
    });

    let mut hover_target: Option<usize> = None;

    for (_entity, _button, slot, interaction, mut bg) in &mut button_query {
        let is_original = runtime.dragged_item_original_index == Some(slot.index);
        let is_dragging = runtime.dragged_item_index == Some(slot.index);

        match *interaction {
            Interaction::Hovered => {
                hover_target = Some(slot.index);
                if mouse_just_pressed {
                    runtime.selected = slot.index;
                    runtime.dragged_item_index = Some(slot.index);
                    runtime.dragged_item_original_index = Some(slot.index);
                    runtime.drag_start_cursor = cursor_pos;
                }
                if is_original || is_dragging || runtime.selected == slot.index {
                    *bg = BackgroundColor(Color::srgb(0.82, 0.82, 0.79));
                } else {
                    *bg = BackgroundColor(Color::srgb(0.65, 0.65, 0.62));
                }
            }
            Interaction::Pressed => {
                hover_target = Some(slot.index);
                *bg = BackgroundColor(Color::srgb(0.82, 0.82, 0.79));
            }
            Interaction::None =>
                if !is_original && !is_dragging && runtime.selected != slot.index {
                    *bg = BackgroundColor(theme::BUTTON_BG);
                } else if runtime.selected == slot.index {
                    *bg = BackgroundColor(Color::srgb(0.82, 0.82, 0.79));
                },
        }
    }

    if mouse_held && runtime.dragged_item_original_index.is_some() {
        if runtime.drag_visual_entity.is_none() {
            if let (Some(start), Some(cursor)) = (runtime.drag_start_cursor, cursor_pos) {
                if cursor.distance(start) >= INVENTORY_DRAG_START_THRESHOLD {
                    ensure_drag_ghost(&mut commands, &mut runtime);
                }
            }
        }

        if runtime.drag_visual_entity.is_some() {
            runtime.dragged_item_index = hover_target;
            runtime.drop_zone_active = drop_zone_active;
            if let Some(cursor) = cursor_pos {
                update_drag_ghost_position(&mut commands, &runtime, cursor, ui_scale.0);
            }
        }
    }

    if mouse_just_released {
        let items = discovery_db.entries(DiscoveryKind::Item);
        let was_dragging = runtime.drag_visual_entity.is_some();
        if let Some(original_idx) = runtime.dragged_item_original_index {
            if let Some(dragged_item) = items.get(original_idx) {
                if was_dragging && runtime.drop_zone_active {
                    commands.write_message(UiDiscoveryCommand::DropItem {
                        id: dragged_item.id.clone(),
                    });
                    if runtime.selected >= items.len().saturating_sub(1) {
                        runtime.selected = runtime.selected.saturating_sub(1);
                    }
                    commands.write_message(UiInventoryCommand::Open);
                } else if was_dragging {
                    if let Some(target_idx) = hover_target {
                        if target_idx != original_idx && target_idx < items.len() {
                            commands.write_message(UiDiscoveryCommand::MoveItem {
                                id: dragged_item.id.clone(),
                                to_index: target_idx,
                            });
                            runtime.selected = target_idx;
                            commands.write_message(UiInventoryCommand::Open);
                        } else {
                            runtime.selected = original_idx;
                            commands.write_message(UiDiscoveryCommand::SetSeen {
                                kind: DiscoveryKind::Item,
                                id: dragged_item.id.clone(),
                                seen: true,
                            });
                        }
                    }
                } else if hover_target == Some(original_idx) {
                    runtime.selected = original_idx;
                    commands.write_message(UiDiscoveryCommand::SetSeen {
                        kind: DiscoveryKind::Item,
                        id: dragged_item.id.clone(),
                        seen: true,
                    });
                    commands.write_message(UiInventoryCommand::Open);
                }
            }
        }
        runtime.dragged_item_index = None;
        runtime.dragged_item_original_index = None;
        runtime.drag_start_cursor = None;
        runtime.drop_zone_active = false;
        despawn_drag_ghost(&mut commands, &mut runtime);
    }
}

pub(super) fn sync_inventory_preview_from_selection(
    mut commands: Commands,
    _fonts: Res<super::systems::UiFonts>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    children: Query<&Children>,
    mut runtime: ResMut<UiInventoryRuntime>,
    discovery_db: Res<UiDiscoveryDb>,
) {
    let items = discovery_db.entries(DiscoveryKind::Item);
    let Some(item) = items.get(runtime.selected) else {
        return;
    };

    let model_path = match &item.model_path {
        Some(path) if !path.trim().is_empty() => path.clone(),
        _ => return,
    };

    if runtime.preview_world_root.is_some() && runtime.preview_model_root.is_some() {
        return;
    }

    if let Some(viewport) = runtime.preview_viewport {
        detach_viewport_target(&mut commands, viewport);
    }

    if let Some(old_root) = runtime.preview_world_root.take() {
        despawn_tree(&mut commands, old_root, &children);
    }

    runtime.preview_viewport = None;
    runtime.preview_pivot = None;
    runtime.preview_model_root = None;
    runtime.preview_camera = None;
    runtime.preview_framed = false;
    runtime.preview_dragging = false;

    let preview_scene =
        spawn_inventory_preview_world(&mut commands, &assets, &mut images, &model_path);

    if let Some(scene) = preview_scene {
        runtime.preview_pivot = Some(scene.pivot);
        runtime.preview_world_root = Some(scene.root);
        runtime.preview_model_root = Some(scene.model_root);
        runtime.preview_camera = Some(scene.camera);
        runtime.preview_image_target = Some(scene.target);
    }
}

pub(super) fn sync_and_frame_inventory_preview(
    mut commands: Commands,
    mut runtime: ResMut<UiInventoryRuntime>,
    children: Query<&Children>,
    tagged: Query<(), With<InventoryPreviewLayerTagged>>,
    aabbs: Query<(&Aabb, &GlobalTransform)>,
    mut transforms: Query<&mut Transform>,
) {
    let Some(model_root) = runtime.preview_model_root else {
        return;
    };

    if !tagged.contains(model_root) {
        commands.queue(move |world: &mut World| {
            if let Ok(mut entity) = world.get_entity_mut(model_root) {
                entity.insert((
                    RenderLayers::layer(INVENTORY_PREVIEW_RENDER_LAYER),
                    InventoryPreviewLayerTagged,
                ));
            }
        });
    }

    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut found_mesh = false;

    for entity in children.iter_descendants(model_root) {
        if !tagged.contains(entity) {
            commands.queue(move |world: &mut World| {
                if let Ok(mut current) = world.get_entity_mut(entity) {
                    current.insert((
                        RenderLayers::layer(INVENTORY_PREVIEW_RENDER_LAYER),
                        InventoryPreviewLayerTagged,
                    ));
                }
            });
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

    if runtime.preview_framed || !found_mesh {
        return;
    }

    let center = (min + max) * 0.5;
    let radius = ((max - min) * 0.5).max_element().max(0.20);

    if let Ok(mut model_transform) = transforms.get_mut(model_root) {
        model_transform.translation = -center;
    }
    if let Some(camera) = runtime.preview_camera {
        if let Ok(mut camera_transform) = transforms.get_mut(camera) {
            let eye = Vec3::new(radius * 0.35, radius * 0.75 + 0.12, radius * 3.00 + 0.55);
            *camera_transform = Transform::from_translation(eye)
                .looking_at(Vec3::new(0.0, radius * 0.20, 0.0), Vec3::Y);
        }
    }

    runtime.preview_framed = true;
}

pub(super) fn rotate_inventory_preview(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut runtime: ResMut<UiInventoryRuntime>,
    mut pivots: Query<&mut Transform, With<InventoryPreviewPivot>>,
    _preview_cameras: Query<
        (&GlobalTransform, &Projection, &mut Transform),
        Without<InventoryPreviewPivot>,
    >,
    preview_nodes: Query<
        (&ComputedNode, &UiGlobalTransform, &InheritedVisibility),
        With<InventoryPreviewViewport>,
    >,
) {
    let Some(pivot_entity) = runtime.preview_pivot else {
        return;
    };
    let Ok(mut pivot_transform) = pivots.get_mut(pivot_entity) else {
        return;
    };

    let mut drag_delta = Vec2::ZERO;
    for event in mouse_motion.read() {
        drag_delta += event.delta;
    }

    let cursor_pos = windows
        .single()
        .ok()
        .and_then(|window| window.cursor_position());
    let hovered = cursor_pos.is_some_and(|cursor| cursor_inside_preview(cursor, &preview_nodes));

    if runtime.preview_dragging && mouse.pressed(MouseButton::Left) {
        if drag_delta.length_squared() > f32::EPSILON {
            pivot_transform.rotate_y(-drag_delta.x * 0.012);
            pivot_transform.rotate_local_x(-drag_delta.y * 0.010);
        }
        return;
    }

    if hovered && mouse.just_pressed(MouseButton::Left) {
        runtime.preview_dragging = true;
        if drag_delta.length_squared() > f32::EPSILON {
            pivot_transform.rotate_y(-drag_delta.x * 0.012);
            pivot_transform.rotate_local_x(-drag_delta.y * 0.010);
        }
        return;
    }

    runtime.preview_dragging = false;
    let dt = time.delta_secs();
    pivot_transform.rotate_y(dt * 0.56);
    pivot_transform.rotate_local_x(dt * 0.23);
    pivot_transform.rotate_local_z(dt * 0.17);
}

fn cursor_inside_preview(
    cursor_pos: Vec2,
    preview_nodes: &Query<
        (&ComputedNode, &UiGlobalTransform, &InheritedVisibility),
        With<InventoryPreviewViewport>,
    >,
) -> bool {
    for (computed, ui_transform, visible) in preview_nodes.iter() {
        if !visible.get() {
            continue;
        }
        let size = computed.size();
        if size.x <= 1.0 || size.y <= 1.0 {
            continue;
        }
        if let Some(normalized) = computed.normalize_point(*ui_transform, cursor_pos) {
            if normalized.x >= -0.5
                && normalized.x <= 0.5
                && normalized.y >= -0.5
                && normalized.y <= 0.5
            {
                return true;
            }
        }
    }
    false
}

pub(super) fn handle_inventory_preview_zoom(
    mut scroll_events: MessageReader<MouseWheel>,
    windows: Query<&Window, With<PrimaryWindow>>,
    runtime: Res<UiInventoryRuntime>,
    preview_nodes: Query<
        (&ComputedNode, &UiGlobalTransform, &InheritedVisibility),
        With<InventoryPreviewViewport>,
    >,
    mut preview_cameras: Query<
        (&GlobalTransform, &Projection, &mut Transform),
        Without<InventoryPreviewPivot>,
    >,
) {
    if runtime.preview_camera.is_none() {
        return;
    }

    let cursor_pos = windows.single().ok().and_then(|w| w.cursor_position());
    let Some(cursor) = cursor_pos else {
        return;
    };
    if !cursor_inside_preview(cursor, &preview_nodes) {
        return;
    }

    for event in scroll_events.read() {
        let camera = match runtime.preview_camera {
            Some(c) => c,
            None => continue,
        };
        let Ok((_, _, mut camera_transform)) = preview_cameras.get_mut(camera) else {
            continue;
        };

        let focus = Vec3::ZERO;
        let current = camera_transform.translation - focus;
        let mut distance = current.length().max(0.05);
        let orbit_dir = current.normalize_or_zero();
        if orbit_dir.length_squared() <= f32::EPSILON {
            continue;
        }
        let zoom_speed = (0.35 + distance * 0.28).clamp(0.2, 4.0);
        distance = (distance - event.y * zoom_speed).clamp(0.06, 40.0);
        camera_transform.translation = focus + orbit_dir * distance;
        camera_transform.look_at(focus, Vec3::Y);
    }
}

struct InventoryPreviewSceneEntities {
    root: Entity,
    pivot: Entity,
    model_root: Entity,
    camera: Entity,
    target: Handle<Image>,
}

fn spawn_inventory_preview_world(
    commands: &mut Commands,
    assets: &AssetServer,
    images: &mut Assets<Image>,
    model_path: &str,
) -> Option<InventoryPreviewSceneEntities> {
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
            Name::new("Inventory Preview Root"),
            Transform::default(),
            Visibility::Inherited,
            RenderLayers::layer(INVENTORY_PREVIEW_RENDER_LAYER),
            InventoryPreviewLayerTagged,
        ))
        .id();
    let mut pivot = Entity::PLACEHOLDER;
    let mut model_root = Entity::PLACEHOLDER;
    let mut camera = Entity::PLACEHOLDER;
    commands.entity(root).with_children(|parent| {
        pivot = parent
            .spawn((
                Name::new("Inventory Preview Pivot"),
                InventoryPreviewPivot,
                Transform::default(),
                Visibility::Inherited,
                RenderLayers::layer(INVENTORY_PREVIEW_RENDER_LAYER),
                InventoryPreviewLayerTagged,
            ))
            .with_children(|pivot_parent| {
                model_root = pivot_parent
                    .spawn((
                        Name::new("Inventory Preview Model"),
                        SceneRoot(model_handle),
                        Transform::from_scale(Vec3::splat(1.0)),
                        Visibility::Inherited,
                        RenderLayers::layer(INVENTORY_PREVIEW_RENDER_LAYER),
                        InventoryPreviewLayerTagged,
                    ))
                    .id();
            })
            .id();

        parent.spawn((
            Name::new("Inventory Preview Light"),
            PointLight {
                intensity: 3_800_000.0,
                shadows_enabled: false,
                range: 30.0,
                ..default()
            },
            Transform::from_xyz(2.0, 2.6, 2.4),
            RenderLayers::layer(INVENTORY_PREVIEW_RENDER_LAYER),
            InventoryPreviewLayerTagged,
        ));

        camera = parent
            .spawn((
                Name::new("Inventory Preview Camera"),
                Camera3d::default(),
                Projection::Perspective(PerspectiveProjection {
                    near: 0.12,
                    far: 80.0,
                    ..default()
                }),
                Msaa::Off,
                Camera {
                    order: 0,
                    clear_color: ClearColorConfig::Custom(Color::srgb(0.02, 0.025, 0.03)),
                    ..default()
                },
                RenderTarget::Image(target.clone().into()),
                Transform::from_xyz(0.0, 0.75, 2.8).looking_at(Vec3::new(0.0, 0.45, 0.0), Vec3::Y),
                RenderLayers::layer(INVENTORY_PREVIEW_RENDER_LAYER),
                InventoryPreviewLayerTagged,
            ))
            .id();
    });

    Some(InventoryPreviewSceneEntities {
        root,
        pivot,
        model_root,
        camera,
        target,
    })
}

fn despawn_tree(commands: &mut Commands, root: Entity, children_query: &Query<&Children>) {
    if let Ok(children) = children_query.get(root) {
        for child in children.iter() {
            despawn_tree(commands, child, children_query);
        }
    }
    commands.queue(move |world: &mut World| {
        let _ = world.despawn(root);
    });
}

fn detach_viewport_target(commands: &mut Commands, viewport: Entity) {
    commands.queue(move |world: &mut World| {
        if let Ok(mut entity) = world.get_entity_mut(viewport) {
            entity.remove::<ViewportNode>();
        }
    });
}

fn close_inventory(
    commands: &mut Commands,
    runtime: &mut UiInventoryRuntime,
    roots: &Query<(), With<InventoryUiRoot>>,
    children: &Query<&Children>,
) {
    if let Some(viewport) = runtime.preview_viewport {
        detach_viewport_target(commands, viewport);
    }

    if let Some(root) = runtime.root.take() {
        if roots.get(root).is_ok() {
            commands.queue(move |world: &mut World| {
                let _ = world.despawn(root);
            });
        }
    }

    if let Some(preview_root) = runtime.preview_world_root.take() {
        if let Ok(preview_children) = children.get(preview_root) {
            for child in preview_children.iter() {
                despawn_tree(commands, child, &children);
            }
        }
        commands.queue(move |world: &mut World| {
            let _ = world.despawn(preview_root);
        });
    }

    runtime.preview_viewport = None;
    runtime.preview_label = None;
    runtime.preview_card_root = None;
    runtime.preview_pivot = None;
    runtime.preview_model_root = None;
    runtime.preview_camera = None;
    runtime.preview_framed = false;
    runtime.preview_dragging = false;
    runtime.preview_image_target = None;
    runtime.dragged_item_index = None;
    runtime.dragged_item_original_index = None;
    runtime.drag_start_cursor = None;
    runtime.drag_visual_entity = None;
    runtime.drop_zone_active = false;
}

fn open_inventory(
    commands: &mut Commands,
    fonts: &super::systems::UiFonts,
    discovery_db: &UiDiscoveryDb,
    runtime: &mut UiInventoryRuntime,
    roots: &Query<(), With<InventoryUiRoot>>,
    children: &Query<&Children>,
    assets: &AssetServer,
    images: &mut Assets<Image>,
) {
    close_inventory(commands, runtime, roots, children);
    let items = discovery_db.entries(DiscoveryKind::Item);
    runtime.selected = runtime.selected.min(items.len().saturating_sub(1));

    if let Some(selected_item) = items.get(runtime.selected) {
        if !selected_item.seen {
            commands.write_message(UiDiscoveryCommand::SetSeen {
                kind: DiscoveryKind::Item,
                id: selected_item.id.clone(),
                seen: true,
            });
        }
    }

    let root = commands
        .spawn((
            Name::new("Inventory UI"),
            InventoryUiRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme::OVERLAY),
            GlobalZIndex(92),
            FocusPolicy::Block,
        ))
        .id();

    let selected_item = items.get(runtime.selected).cloned();
    let preview_scene = selected_item
        .as_ref()
        .and_then(|item| item.model_path.as_deref())
        .filter(|p| !p.trim().is_empty())
        .and_then(|model_path| spawn_inventory_preview_world(commands, assets, images, model_path));

    if let Some(ref scene) = preview_scene {
        runtime.preview_pivot = Some(scene.pivot);
        runtime.preview_world_root = Some(scene.root);
        runtime.preview_model_root = Some(scene.model_root);
        runtime.preview_camera = Some(scene.camera);
        runtime.preview_image_target = Some(scene.target.clone());
        runtime.preview_framed = false;
    }

    commands.entity(root).with_children(|overlay| {
        overlay
            .spawn((
                Node {
                    width: Val::Px(INVENTORY_PANEL_WIDTH),
                    height: Val::Px(INVENTORY_PANEL_HEIGHT),
                    border: UiRect::all(Val::Px(3.0)),
                    padding: UiRect::all(Val::Px(10.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                InventoryPanelFrame,
                UiTransform::IDENTITY,
                BackgroundColor(theme::PANEL_BG),
                theme::border(true),
            ))
            .with_children(|frame| {
                frame
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(42.0),
                            border: UiRect::all(Val::Px(2.0)),
                            padding: UiRect::horizontal(Val::Px(12.0)),
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(theme::PANEL_ALT),
                        theme::border(false),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            Text::new("INVENTORY"),
                            TextFont {
                                font: fonts.pixel.clone(),
                                font_size: 18.0,
                                ..default()
                            },
                            TextColor(theme::TEXT_DARK),
                        ));
                        bar.spawn((
                            Text::new("TAB close  |  R/F or drag to reorder  |  Q drop"),
                            TextFont {
                                font: fonts.body.clone(),
                                font_size: 24.0,
                                ..default()
                            },
                            TextColor(theme::TEXT_DARK),
                        ));
                    });

                frame
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        min_height: Val::Px(0.0),
                        column_gap: Val::Px(10.0),
                        ..default()
                    })
                    .with_children(|body| {
                        body.spawn((
                            Node {
                                width: Val::Px(520.0),
                                min_width: Val::Px(520.0),
                                max_width: Val::Px(520.0),
                                height: Val::Percent(100.0),
                                border: UiRect::all(Val::Px(2.0)),
                                padding: UiRect::all(Val::Px(8.0)),
                                flex_wrap: FlexWrap::Wrap,
                                column_gap: Val::Px(8.0),
                                row_gap: Val::Px(8.0),
                                align_content: AlignContent::FlexStart,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.06, 0.07, 0.08)),
                            theme::border(false),
                        ))
                        .with_children(|grid| {
                            if items.is_empty() {
                                grid.spawn((
                                    Text::new("no key items yet"),
                                    TextFont {
                                        font: fonts.body.clone(),
                                        font_size: 24.0,
                                        ..default()
                                    },
                                    TextColor(theme::TEXT_LIGHT),
                                ));
                            } else {
                                for (index, item) in items.iter().enumerate() {
                                    let selected = index == runtime.selected;
                                    let bg = if selected {
                                        Color::srgb(0.82, 0.82, 0.79)
                                    } else if item.seen {
                                        Color::srgb(0.50, 0.50, 0.48)
                                    } else {
                                        theme::BUTTON_BG
                                    };
                                    grid.spawn((
                                        Button,
                                        InventoryItemButton {
                                            index,
                                            item_id: item.id.clone(),
                                        },
                                        InventoryItemSlot { index },
                                        Node {
                                            width: Val::Px(160.0),
                                            min_width: Val::Px(160.0),
                                            max_width: Val::Px(160.0),
                                            height: Val::Px(90.0),
                                            border: UiRect::all(Val::Px(2.0)),
                                            padding: UiRect::all(Val::Px(6.0)),
                                            flex_direction: FlexDirection::Column,
                                            row_gap: Val::Px(4.0),
                                            justify_content: JustifyContent::SpaceBetween,
                                            ..default()
                                        },
                                        BackgroundColor(bg),
                                        theme::border(!selected),
                                    ))
                                    .with_children(|slot| {
                                        slot.spawn((
                                            Text::new(item.title.clone()),
                                            TextFont {
                                                font: fonts.pixel.clone(),
                                                font_size: 12.0,
                                                ..default()
                                            },
                                            TextColor(theme::TEXT_DARK),
                                        ));
                                        let shown = if item.seen { "shown" } else { "new" };
                                        slot.spawn((
                                            Text::new(shown),
                                            TextFont {
                                                font: fonts.body.clone(),
                                                font_size: 22.0,
                                                ..default()
                                            },
                                            TextColor(theme::TEXT_DARK),
                                        ));
                                    });
                                }
                            }
                        });

                        body.spawn((
                            Name::new("Inventory Detail Panel"),
                            Node {
                                flex_grow: 1.0,
                                min_width: Val::Px(0.0),
                                height: Val::Percent(100.0),
                                border: UiRect::all(Val::Px(2.0)),
                                padding: UiRect::all(Val::Px(8.0)),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(6.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.05, 0.06, 0.07)),
                            theme::border(false),
                        ))
                        .with_children(|detail| {
                            if let Some(item) = items.get(runtime.selected) {
                                detail.spawn((
                                    Text::new(item.title.clone()),
                                    TextFont {
                                        font: fonts.pixel.clone(),
                                        font_size: 14.0,
                                        ..default()
                                    },
                                    TextColor(theme::TEXT_LIGHT),
                                ));
                                detail.spawn((
                                    Text::new(item.subtitle.clone()),
                                    TextFont {
                                        font: fonts.body.clone(),
                                        font_size: 18.0,
                                        ..default()
                                    },
                                    TextColor(theme::TEXT_LIGHT),
                                ));

                                if let Some(model_path) = &item.model_path {
                                    if !model_path.trim().is_empty() {
                                        if let Some(ref scene) = preview_scene {
                                            detail.spawn((
                                                Name::new("Inventory Preview Viewport"),
                                                InventoryPreviewViewport,
                                                Interaction::default(),
                                                Node {
                                                    width: Val::Percent(100.0),
                                                    height: Val::Px(140.0),
                                                    min_height: Val::Px(140.0),
                                                    max_height: Val::Px(140.0),
                                                    border: UiRect::all(Val::Px(2.0)),
                                                    ..default()
                                                },
                                                ViewportNode::new(scene.camera),
                                                BackgroundColor(Color::srgb(0.03, 0.04, 0.06)),
                                                theme::border(false),
                                            ));
                                        }
                                    }
                                }

                                detail
                                    .spawn((
                                        Name::new("Inventory Description Scroll"),
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
                                            Name::new("Inventory Description Text"),
                                            Node {
                                                width: Val::Percent(100.0),
                                                ..default()
                                            },
                                            Text::new(item.description.clone()),
                                            TextFont {
                                                font: fonts.body.clone(),
                                                font_size: 20.0,
                                                ..default()
                                            },
                                            TextColor(theme::TEXT_LIGHT),
                                            TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                                        ));
                                    });
                            } else {
                                detail.spawn((
                                    Text::new("select an item"),
                                    TextFont {
                                        font: fonts.body.clone(),
                                        font_size: 24.0,
                                        ..default()
                                    },
                                    TextColor(theme::TEXT_LIGHT),
                                ));
                            }
                        });
                    });
            });
    });

    runtime.root = Some(root);
    runtime.last_revision = discovery_db.revision();
}

fn ensure_drag_ghost(commands: &mut Commands, runtime: &mut UiInventoryRuntime) {
    if runtime.drag_visual_entity.is_some() {
        return;
    }
    let Some(root) = runtime.root else {
        return;
    };
    let ghost = commands
        .spawn((
            InventoryDragGhost,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(160.0),
                height: Val::Px(90.0),
                border: UiRect::all(Val::Px(2.0)),
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.82, 0.82, 0.79, 0.65)),
            theme::border(false),
            FocusPolicy::Pass,
            GlobalZIndex(120),
            Pickable::IGNORE,
        ))
        .id();
    commands.queue(move |world: &mut World| {
        if world.get_entity(root).is_err() || world.get_entity(ghost).is_err() {
            let _ = world.despawn(ghost);
            return;
        }
        world.entity_mut(root).add_child(ghost);
    });
    runtime.drag_visual_entity = Some(ghost);
}

fn update_drag_ghost_position(
    commands: &mut Commands,
    runtime: &UiInventoryRuntime,
    cursor: Vec2,
    ui_scale: f32,
) {
    let Some(ghost) = runtime.drag_visual_entity else {
        return;
    };
    let scale = ui_scale.max(f32::EPSILON);
    let ui_cursor = cursor / scale;
    let factor = if runtime.drop_zone_active {
        INVENTORY_GHOST_SHRINK
    } else {
        1.0
    };
    let width = 160.0 * factor;
    let height = 90.0 * factor;
    let next_node = Node {
        position_type: PositionType::Absolute,
        width: Val::Px(width),
        height: Val::Px(height),
        border: UiRect::all(Val::Px(2.0)),
        left: Val::Px((ui_cursor.x - width * 0.5).max(0.0)),
        top: Val::Px((ui_cursor.y - height * 0.5).max(0.0)),
        ..default()
    };
    commands.queue(move |world: &mut World| {
        if let Ok(mut entity) = world.get_entity_mut(ghost) {
            entity.insert(next_node);
        }
    });
}

fn despawn_drag_ghost(commands: &mut Commands, runtime: &mut UiInventoryRuntime) {
    let Some(ghost) = runtime.drag_visual_entity.take() else {
        return;
    };
    commands.queue(move |world: &mut World| {
        let _ = world.despawn(ghost);
    });
}

fn cursor_in_drop_zone(cursor: Vec2, window: &Window) -> bool {
    let margin = INVENTORY_DROP_ZONE_MARGIN;
    cursor.x <= margin
        || cursor.y <= margin
        || cursor.x >= (window.width() - margin)
        || cursor.y >= (window.height() - margin)
}
