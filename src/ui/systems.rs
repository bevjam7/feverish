use std::collections::HashMap;

use bevy::{
    input::mouse::{MouseScrollUnit, MouseWheel},
    prelude::*,
    ui::{ComputedNode, FocusPolicy, UiScale, ui_transform::UiGlobalTransform},
    window::{CursorOptions, PrimaryWindow},
};

use super::{
    components::{
        ButtonAction, ConfirmAction, ConfirmDialogMessage, ConfirmDialogRoot, DialogueUiRoot,
        DisabledButton, DiscoveryEntry, DiscoveryKind, DitherPixel, GalleryDetailDescription,
        GalleryDetailStatus, GalleryDetailSubtitle, GalleryDetailTitle, GalleryListCache,
        GalleryListRoot, MainMenuGalleryPanel, MainMenuHeading, MainMenuLine, MainMenuPage,
        MainMenuSettingsPanel, MainMenuState, MainMenuTerminalPanel, MainMenuTicker, MainMenuUi,
        MenuButton, MenuConfirmState, MenuOwner, PauseMenuPage, PauseMenuSettingsPanel,
        PauseMenuState, PauseMenuStatusPanel, PauseMenuUi, SettingsValueText, UiCursorSprite,
        UiDiscoveryCommand, UiMenuAction,
    },
    main_menu::spawn_main_menu,
    pause_menu::spawn_pause_menu,
    theme,
};
use crate::{AppState, GameState, Paused, assets::GameAssets, settings::GameSettings};

#[derive(Resource)]
pub(super) struct UiFonts {
    pub pixel: Handle<Font>,
    pub body: Handle<Font>,
}

#[derive(Resource)]
pub(super) struct UiCursorIcons {
    pub pointing: Handle<Image>,
    pub closed: Handle<Image>,
}

#[derive(Resource, Default)]
pub(super) struct UiRegistry {
    pub main_roots: HashMap<Entity, Entity>,
    pub pause_roots: HashMap<Entity, Entity>,
}

#[derive(Resource, Debug)]
pub struct UiDiscoveryDb {
    items: Vec<DiscoveryEntry>,
    npcs: Vec<DiscoveryEntry>,
    revision: u64,
}

#[derive(EntityEvent, Debug)]
#[entity_event(propagate, auto_propagate)]
pub(super) struct UiScrollEvent {
    entity: Entity,
    delta: Vec2,
}

impl Default for UiDiscoveryDb {
    fn default() -> Self {
        Self {
            items: vec![],
            npcs: vec![
                DiscoveryEntry::new("D.", "Mr. D.")
                    .subtitle("room 400")
                    .description("what day is today? Doomsday?")
                    .seen(true),
            ],
            revision: 1,
        }
    }
}

impl UiDiscoveryDb {
    pub fn upsert(&mut self, kind: DiscoveryKind, entry: DiscoveryEntry) {
        let entries = self.entries_mut(kind);
        if let Some(existing) = entries.iter_mut().find(|it| it.id == entry.id) {
            *existing = entry;
        } else {
            entries.push(entry);
        }
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn remove(&mut self, kind: DiscoveryKind, id: &str) -> bool {
        let entries = self.entries_mut(kind);
        let before = entries.len();
        entries.retain(|it| it.id != id);
        let removed = entries.len() != before;
        if removed {
            self.revision = self.revision.wrapping_add(1);
        }
        removed
    }

    pub fn set_seen(&mut self, kind: DiscoveryKind, id: &str, seen: bool) -> bool {
        if let Some(entry) = self.entries_mut(kind).iter_mut().find(|it| it.id == id) {
            if entry.seen != seen {
                entry.seen = seen;
                self.revision = self.revision.wrapping_add(1);
            }
            return true;
        }
        false
    }

    pub fn clear_kind(&mut self, kind: DiscoveryKind) {
        self.entries_mut(kind).clear();
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn entries(&self, kind: DiscoveryKind) -> &[DiscoveryEntry] {
        match kind {
            DiscoveryKind::Item => &self.items,
            DiscoveryKind::Npc => &self.npcs,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    fn entries_mut(&mut self, kind: DiscoveryKind) -> &mut Vec<DiscoveryEntry> {
        match kind {
            DiscoveryKind::Item => &mut self.items,
            DiscoveryKind::Npc => &mut self.npcs,
        }
    }
}

pub(super) fn populate_ui_fonts_and_cursor(mut commands: Commands, assets: Res<GameAssets>) {
    commands.insert_resource(UiFonts {
        pixel: assets.font_pixel.clone(),
        body: assets.font_body.clone(),
    });
    commands.insert_resource(UiCursorIcons {
        pointing: assets.image_cursor.clone(),
        closed: assets.image_cursor_closed.clone(),
    });
}

pub(super) fn update_ui_scale(
    mut ui_scale: ResMut<UiScale>,
    windows: Query<&Window, With<PrimaryWindow>>,
    settings: Res<GameSettings>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let width = window.resolution.width();
    let height = window.resolution.height();
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let auto_scale = (width / theme::UI_WIDTH).min(height / theme::UI_HEIGHT);
    let next_scale = if settings.ui_scale_auto {
        auto_scale
    } else {
        settings.manual_ui_scale.clamp(0.6, 2.0)
    };
    if (ui_scale.0 - next_scale).abs() > 0.001 {
        ui_scale.0 = next_scale;
    }
}

pub(super) fn update_ui_cursor(
    mut commands: Commands,
    ui_visible: Query<(), Or<(With<MainMenuUi>, With<PauseMenuUi>, With<DialogueUiRoot>)>>,
    cursors: Res<UiCursorIcons>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    ui_scale: Res<UiScale>,
    settings: Res<GameSettings>,
    mut click_timer: Local<f32>,
    mut last_cursor_pos: Local<Option<Vec2>>,
    mut move_energy: Local<f32>,
    mut windows: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    mut cursor_sprite: Query<
        (Entity, &mut Node, &mut ImageNode, &mut UiTransform),
        With<UiCursorSprite>,
    >,
) {
    let is_ui_visible = !ui_visible.is_empty();
    let pressed = mouse.pressed(MouseButton::Left);
    if mouse.just_pressed(MouseButton::Left) {
        *click_timer = 0.0;
    } else {
        *click_timer += time.delta_secs();
    }

    let Ok((window, mut cursor_options)) = windows.single_mut() else {
        return;
    };

    cursor_options.visible = !is_ui_visible;
    if !is_ui_visible {
        *last_cursor_pos = None;
        *move_energy = 0.0;
    }

    if is_ui_visible && cursor_sprite.is_empty() {
        commands.spawn((
            UiCursorSprite,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(32.0),
                height: Val::Px(32.0),
                display: Display::None,
                ..default()
            },
            ImageNode::new(cursors.pointing.clone()),
            UiTransform::IDENTITY,
            FocusPolicy::Pass,
            GlobalZIndex(999),
            Pickable::IGNORE,
        ));
    }

    for (_, mut node, mut image, mut transform) in &mut cursor_sprite {
        if !is_ui_visible {
            node.display = Display::None;
            continue;
        }

        let Some(pos) = window.cursor_position() else {
            node.display = Display::None;
            continue;
        };

        node.display = Display::Flex;
        let ui_x = (pos.x / ui_scale.0).round();
        let ui_y = (pos.y / ui_scale.0).round();
        // tried to clibrate hotspot so the icon finger tip aligns with the real pointer, maybe this shoulld work?
        let offset = Vec2::new(0.0, 3.0);
        node.left = Val::Px(ui_x + offset.x);
        node.top = Val::Px(ui_y + offset.y);

        let delta = if let Some(prev) = *last_cursor_pos {
            pos - prev
        } else {
            Vec2::ZERO
        };
        *last_cursor_pos = Some(pos);

        let speed = delta.length() / time.delta_secs().max(0.0001);
        let normalized_speed = (speed / 1200.0).clamp(0.0, 1.0);
        let target_energy = if delta.length_squared() > 0.0 {
            normalized_speed
        } else {
            0.0
        };
        *move_energy = *move_energy * 0.84 + target_energy * 0.16;

        image.image = if pressed {
            cursors.closed.clone()
        } else {
            cursors.pointing.clone()
        };
        image.color = if pressed {
            theme::CURSOR_TINT_PRESSED
        } else {
            theme::CURSOR_TINT
        };

        if !settings.cursor_motion {
            transform.translation = Val2::ZERO;
            transform.scale = Vec2::splat(2.0);
            transform.rotation = Rot2::IDENTITY;
            continue;
        }

        let t = (*click_timer).min(0.18);
        let click_pulse = if t < 0.18 {
            (1.0 - t / 0.18) * (t * 80.0).sin().abs()
        } else {
            0.0
        };
        let move_wave = (time.elapsed_secs() * (18.0 + 36.0 * *move_energy)).sin();
        let move_bob = (time.elapsed_secs() * (22.0 + 40.0 * *move_energy)).cos();
        let move_jitter = Vec2::new(move_wave, move_bob) * (0.5 + 1.8 * *move_energy);
        let click_jitter = if pressed {
            Vec2::new(
                (time.elapsed_secs() * 90.0).sin(),
                (time.elapsed_secs() * 110.0).cos(),
            ) * 1.4
        } else {
            Vec2::ZERO
        };
        let jitter = move_jitter + click_jitter;

        transform.translation = Val2::px(jitter.x.round(), jitter.y.round());
        let scale_bump = if click_pulse > 0.08 { 1.0 } else { 0.0 };
        transform.scale = Vec2::splat(2.0 + scale_bump);
        transform.rotation = Rot2::IDENTITY;
    }
}

pub(super) fn emulate_button_interaction_for_offscreen_ui(
    windows: Query<&Window, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    ui_visible: Query<(), Or<(With<MainMenuUi>, With<PauseMenuUi>, With<DialogueUiRoot>)>>,
    mut interactables: Query<(
        &ComputedNode,
        &UiGlobalTransform,
        &mut Interaction,
        Option<&InheritedVisibility>,
        Option<&DisabledButton>,
    )>,
) {
    let Ok(window) = windows.single() else {
        return;
    };

    let has_visible_ui = !ui_visible.is_empty();
    let cursor = window.cursor_position();
    let pressed = mouse.pressed(MouseButton::Left);
    let just_released = mouse.just_released(MouseButton::Left);

    for (node, transform, mut interaction, inherited_visibility, disabled) in &mut interactables {
        if !has_visible_ui
            || disabled.is_some()
            || inherited_visibility.is_some_and(|visibility| !visibility.get())
            || node.size() == Vec2::ZERO
        {
            interaction.set_if_neq(Interaction::None);
            continue;
        }

        let Some(cursor_position) = cursor else {
            interaction.set_if_neq(Interaction::None);
            continue;
        };
        let contains = node.contains_point(*transform, cursor_position);

        let next = if contains {
            if just_released {
                Interaction::Pressed
            } else if pressed {
                Interaction::Hovered
            } else {
                Interaction::Hovered
            }
        } else {
            Interaction::None
        };

        interaction.set_if_neq(next);
    }
}

pub(super) fn animate_main_menu_ticker(
    time: Res<Time>,
    mut ticker: Query<(&mut MainMenuTicker, &mut Text, &mut UiTransform)>,
) {
    const SPEED: f32 = 130.0;
    const START_X: f32 = theme::UI_WIDTH + 380.0;
    const END_MARGIN: f32 = 420.0;
    const PAUSE_SECS: f32 = 0.7;

    for (mut tag, mut text, mut transform) in &mut ticker {
        if tag.tips.is_empty() {
            continue;
        }

        if tag.pause_timer > 0.0 {
            tag.pause_timer -= time.delta_secs();
            transform.translation = Val2::px(tag.offset_x.round(), 0.0);
            continue;
        }

        tag.offset_x -= SPEED * time.delta_secs();

        let current_tip = &tag.tips[tag.current];
        let estimated_width = (current_tip.chars().count() as f32) * 16.0;
        if tag.offset_x + estimated_width < -(theme::UI_WIDTH + END_MARGIN) {
            tag.current = (tag.current + 1) % tag.tips.len();
            *text = Text::new(tag.tips[tag.current].clone());
            tag.offset_x = START_X;
            tag.pause_timer = PAUSE_SECS;
        }

        transform.translation = Val2::px(tag.offset_x.round(), 0.0);
    }
}

pub(super) fn reset_ticker_on_scale_change(
    ui_scale: Res<UiScale>,
    mut tickers: Query<&mut MainMenuTicker>,
) {
    if !ui_scale.is_changed() {
        return;
    }
    for mut ticker in &mut tickers {
        ticker.offset_x = theme::UI_WIDTH + 380.0;
        ticker.pause_timer = 0.0;
    }
}

pub(super) fn cleanup_ui_cursor(
    mut commands: Commands,
    ui_visible: Query<(), Or<(With<MainMenuUi>, With<PauseMenuUi>, With<DialogueUiRoot>)>>,
    cursor_sprite: Query<Entity, With<UiCursorSprite>>,
) {
    if !ui_visible.is_empty() {
        return;
    }
    for entity in &cursor_sprite {
        commands.entity(entity).despawn();
    }
}

pub(super) fn restore_native_cursor_on_exit(
    ui_visible: Query<(), Or<(With<MainMenuUi>, With<PauseMenuUi>, With<DialogueUiRoot>)>>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if !ui_visible.is_empty() {
        return;
    }
    for mut cursor_options in &mut windows {
        cursor_options.visible = true;
    }
}

pub(super) fn handle_menu_actions(
    mut commands: Commands,
    mut actions: MessageReader<UiMenuAction>,
    mut exit: MessageWriter<AppExit>,
    mut next_app: ResMut<NextState<AppState>>,
    mut next_game: Option<ResMut<NextState<GameState>>>,
    mut next_paused: ResMut<NextState<Paused>>,
    current_app_state: Option<Res<State<AppState>>>,
) {
    for action in actions.read() {
        match *action {
            UiMenuAction::Play(owner) | UiMenuAction::Continue(owner) => {
                commands.entity(owner).remove::<MainMenuUi>();
                commands.entity(owner).remove::<PauseMenuUi>();
                next_paused.set(Paused(false));
                let app_is_game = current_app_state
                    .as_ref()
                    .is_some_and(|state| *state.get() == AppState::Main);
                if !app_is_game {
                    next_app.set(AppState::Main);
                }
                if let Some(next_game) = &mut next_game {
                    next_game.set(GameState::Prepare);
                }
            }
            UiMenuAction::Resume(owner) => {
                commands.entity(owner).remove::<PauseMenuUi>();
                next_paused.set(Paused(false));
            }
            UiMenuAction::BackToMainMenu(owner) => {
                commands.entity(owner).remove::<PauseMenuUi>();
                commands.entity(owner).insert(MainMenuUi);
                next_paused.set(Paused(false));
            }
            UiMenuAction::QuitGame(_) => {
                exit.write(AppExit::Success);
            }
        }
    }
}

pub(super) fn handle_pause_shortcut(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut next_paused: ResMut<NextState<Paused>>,
    owners: Query<(Entity, Option<&Name>, Has<MainMenuUi>, Has<PauseMenuUi>)>,
    dialogue_ui: Query<(), With<DialogueUiRoot>>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if !dialogue_ui.is_empty() {
        return;
    }

    for (entity, name, has_main_menu, has_pause_menu) in &owners {
        let Some(name) = name else {
            continue;
        };
        if name.as_str() != "Main Menu Driver" {
            continue;
        }

        if has_main_menu {
            return;
        }

        if has_pause_menu {
            commands.entity(entity).remove::<PauseMenuUi>();
            next_paused.set(Paused(false));
        } else {
            commands.entity(entity).insert(PauseMenuUi);
            next_paused.set(Paused(true));
        }
        return;
    }
}

pub(super) fn send_scroll_events(
    mut mouse_wheel_events: MessageReader<MouseWheel>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut commands: Commands,
    scrollables: Query<
        (
            Entity,
            &Node,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
            Option<&GlobalZIndex>,
        ),
        With<ScrollPosition>,
    >,
) {
    const SCROLL_LINE: f32 = 24.0;
    let Ok(window) = windows.single() else {
        return;
    };
    let cursor = window.cursor_position();

    for event in mouse_wheel_events.read() {
        let mut delta = -Vec2::new(event.x, event.y);
        if event.unit == MouseScrollUnit::Line {
            delta *= SCROLL_LINE;
        }
        if keyboard_input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) {
            std::mem::swap(&mut delta.x, &mut delta.y);
        }

        let Some(entity) = pick_scroll_owner_at_cursor(cursor, &scrollables) else {
            continue;
        };
        commands.trigger(UiScrollEvent { entity, delta });
    }
}

fn pick_scroll_owner_at_cursor(
    cursor: Option<Vec2>,
    scrollables: &Query<
        (
            Entity,
            &Node,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
            Option<&GlobalZIndex>,
        ),
        With<ScrollPosition>,
    >,
) -> Option<Entity> {
    let cursor = cursor?;
    let mut best: Option<(i32, f32, Entity)> = None;

    for (entity, node, computed, transform, visibility, z) in scrollables.iter() {
        if !visibility.get()
            || computed.size() == Vec2::ZERO
            || (node.overflow.x != OverflowAxis::Scroll && node.overflow.y != OverflowAxis::Scroll)
            || !computed.contains_point(*transform, cursor)
        {
            continue;
        }

        let area = computed.size().x * computed.size().y;
        let z = z.map_or(0, |it| it.0);
        match best {
            Some((best_z, best_area, _)) if z < best_z || (z == best_z && area >= best_area) => {}
            _ => best = Some((z, area, entity)),
        }
    }

    best.map(|(_, _, entity)| entity)
}

pub(super) fn on_ui_scroll(
    mut scroll: On<UiScrollEvent>,
    mut query: Query<(
        &mut ScrollPosition,
        &Node,
        &ComputedNode,
        &InheritedVisibility,
    )>,
) {
    let Ok((mut scroll_position, node, computed, visibility)) = query.get_mut(scroll.entity) else {
        return;
    };
    if !visibility.get() {
        return;
    }

    let max_offset = (computed.content_size() - computed.size()) * computed.inverse_scale_factor();
    let max_offset = Vec2::new(max_offset.x.max(0.0), max_offset.y.max(0.0));

    let delta = &mut scroll.delta;
    if node.overflow.x == OverflowAxis::Scroll && delta.x != 0.0 {
        let at_edge = if delta.x > 0.0 {
            scroll_position.x >= max_offset.x
        } else {
            scroll_position.x <= 0.0
        };

        if !at_edge {
            scroll_position.x = (scroll_position.x + delta.x).clamp(0.0, max_offset.x);
            delta.x = 0.0;
        }
    }

    if node.overflow.y == OverflowAxis::Scroll && delta.y != 0.0 {
        let at_edge = if delta.y > 0.0 {
            scroll_position.y >= max_offset.y
        } else {
            scroll_position.y <= 0.0
        };

        if !at_edge {
            scroll_position.y = (scroll_position.y + delta.y).clamp(0.0, max_offset.y);
            delta.y = 0.0;
        }
    }

    if *delta == Vec2::ZERO {
        scroll.propagate(false);
    }
}

pub(super) fn refresh_confirm_dialogs(
    confirm_states: Query<&MenuConfirmState>,
    mut roots: Query<(&ConfirmDialogRoot, &mut Node)>,
    mut messages: Query<(&ConfirmDialogMessage, &mut Text)>,
) {
    let owners: HashMap<Entity, Option<ConfirmAction>> = confirm_states
        .iter()
        .map(|state| (state.owner, state.pending))
        .collect();

    for (tag, mut node) in &mut roots {
        let show = owners.get(&tag.owner).copied().flatten().is_some();
        node.display = if show { Display::Flex } else { Display::None };
    }

    for (tag, mut text) in &mut messages {
        let msg = match owners.get(&tag.owner).copied().flatten() {
            Some(ConfirmAction::QuitGame) => "are you sure you want to quit?",
            Some(ConfirmAction::BackToMainMenu) => "are you sure you want to return to main menu?",
            None => "are you sure?",
        };
        *text = Text::new(msg);
    }
}

pub(super) fn apply_discovery_commands(
    mut commands_in: MessageReader<UiDiscoveryCommand>,
    mut db: ResMut<UiDiscoveryDb>,
) {
    for cmd in commands_in.read() {
        match cmd {
            UiDiscoveryCommand::Upsert { kind, entry } => db.upsert(*kind, entry.clone()),
            UiDiscoveryCommand::Remove { kind, id } => {
                db.remove(*kind, id);
            }
            UiDiscoveryCommand::SetSeen { kind, id, seen } => {
                db.set_seen(*kind, id, *seen);
            }
            UiDiscoveryCommand::ClearKind { kind } => db.clear_kind(*kind),
        }
    }
}

pub(super) fn spawn_main_menu_on_added(
    mut commands: Commands,
    mut registry: ResMut<UiRegistry>,
    fonts: Res<UiFonts>,
    added: Query<Entity, Added<MainMenuUi>>,
) {
    for owner in &added {
        if registry.main_roots.contains_key(&owner) {
            continue;
        }
        let root = spawn_main_menu(&mut commands, &fonts, owner);
        registry.main_roots.insert(owner, root);
    }
}

pub(super) fn spawn_pause_menu_on_added(
    mut commands: Commands,
    mut registry: ResMut<UiRegistry>,
    fonts: Res<UiFonts>,
    added: Query<Entity, Added<PauseMenuUi>>,
) {
    for owner in &added {
        if registry.pause_roots.contains_key(&owner) {
            continue;
        }
        let root = spawn_pause_menu(&mut commands, &fonts, owner);
        registry.pause_roots.insert(owner, root);
    }
}

pub(super) fn cleanup_removed_main_menu(
    mut commands: Commands,
    mut registry: ResMut<UiRegistry>,
    mut removed: RemovedComponents<MainMenuUi>,
    children_query: Query<&Children>,
) {
    for owner in removed.read() {
        if let Some(root) = registry.main_roots.remove(&owner) {
            despawn_ui_tree(&mut commands, root, &children_query);
        }
    }
}

pub(super) fn cleanup_removed_pause_menu(
    mut commands: Commands,
    mut registry: ResMut<UiRegistry>,
    mut removed: RemovedComponents<PauseMenuUi>,
    children_query: Query<&Children>,
) {
    for owner in removed.read() {
        if let Some(root) = registry.pause_roots.remove(&owner) {
            despawn_ui_tree(&mut commands, root, &children_query);
        }
    }
}

pub(super) fn handle_button_interactions(
    mut interactions: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            &MenuButton,
            &MenuOwner,
            Has<DisabledButton>,
        ),
        Changed<Interaction>,
    >,
    mut states: Query<&mut MainMenuState>,
    mut pause_states: Query<&mut PauseMenuState>,
    mut confirms: Query<&mut MenuConfirmState>,
    mut actions: MessageWriter<UiMenuAction>,
    db: Res<UiDiscoveryDb>,
    mut settings: ResMut<GameSettings>,
) {
    for (interaction, mut background, mut border, button, owner, disabled) in &mut interactions {
        if disabled {
            *background = BackgroundColor(theme::BUTTON_DISABLED);
            *border = theme::border(false);
            continue;
        }

        match *interaction {
            Interaction::Pressed => {
                *background = BackgroundColor(theme::BUTTON_BG);
                *border = theme::border(false);

                match button.action {
                    ButtonAction::SelectPage(page) => {
                        for mut state in &mut states {
                            if state.owner != owner.0 {
                                continue;
                            }
                            state.page = page;
                            if page == MainMenuPage::DiscoveredItems
                                && state.selected_item.is_none()
                                && !db.entries(DiscoveryKind::Item).is_empty()
                            {
                                state.selected_item = Some(0);
                            }
                            if page == MainMenuPage::PhoneList
                                && state.selected_npc.is_none()
                                && !db.entries(DiscoveryKind::Npc).is_empty()
                            {
                                state.selected_npc = Some(0);
                            }
                        }
                        for mut state in &mut pause_states {
                            if state.owner != owner.0 {
                                continue;
                            }
                            match page {
                                MainMenuPage::Settings => state.page = PauseMenuPage::Settings,
                                _ => state.page = PauseMenuPage::Status,
                            }
                        }
                    }
                    ButtonAction::SelectDiscovery(kind, index) =>
                        for mut state in &mut states {
                            if state.owner != owner.0 {
                                continue;
                            }
                            match kind {
                                DiscoveryKind::Item => {
                                    state.page = MainMenuPage::DiscoveredItems;
                                    state.selected_item = Some(index);
                                }
                                DiscoveryKind::Npc => {
                                    state.page = MainMenuPage::PhoneList;
                                    state.selected_npc = Some(index);
                                }
                            }
                        },
                    ButtonAction::AdjustSetting(key, step) => {
                        settings.adjust(key, step);
                    }
                    ButtonAction::Play => {
                        actions.write(UiMenuAction::Play(owner.0));
                    }
                    ButtonAction::Continue => {
                        actions.write(UiMenuAction::Continue(owner.0));
                    }
                    ButtonAction::Resume => {
                        actions.write(UiMenuAction::Resume(owner.0));
                    }
                    ButtonAction::BackToMainMenu =>
                        for mut confirm in &mut confirms {
                            if confirm.owner == owner.0 {
                                confirm.pending = Some(ConfirmAction::BackToMainMenu);
                            }
                        },
                    ButtonAction::QuitGame =>
                        for mut confirm in &mut confirms {
                            if confirm.owner == owner.0 {
                                confirm.pending = Some(ConfirmAction::QuitGame);
                            }
                        },
                    ButtonAction::ConfirmProceed => {
                        let mut confirmed = None;
                        for mut confirm in &mut confirms {
                            if confirm.owner == owner.0 {
                                confirmed = confirm.pending.take();
                            }
                        }
                        match confirmed {
                            Some(ConfirmAction::BackToMainMenu) => {
                                actions.write(UiMenuAction::BackToMainMenu(owner.0));
                            }
                            Some(ConfirmAction::QuitGame) => {
                                actions.write(UiMenuAction::QuitGame(owner.0));
                            }
                            None => {}
                        }
                    }
                    ButtonAction::ConfirmCancel =>
                        for mut confirm in &mut confirms {
                            if confirm.owner == owner.0 {
                                confirm.pending = None;
                            }
                        },
                }
            }
            Interaction::Hovered => {
                *background = BackgroundColor(theme::BUTTON_HOVER);
                *border = theme::border(button.raised);
            }
            Interaction::None => {
                *background = BackgroundColor(theme::BUTTON_BG);
                *border = theme::border(button.raised);
            }
        }
    }
}

pub(super) fn ensure_gallery_selection_exists(
    mut states: Query<&mut MainMenuState>,
    db: Res<UiDiscoveryDb>,
) {
    for mut state in &mut states {
        if state.page == MainMenuPage::DiscoveredItems {
            let len = db.entries(DiscoveryKind::Item).len();
            if len == 0 {
                state.selected_item = None;
            } else if state.selected_item.is_none_or(|idx| idx >= len) {
                state.selected_item = Some(0);
            }
        }

        if state.page == MainMenuPage::PhoneList {
            let len = db.entries(DiscoveryKind::Npc).len();
            if len == 0 {
                state.selected_npc = None;
            } else if state.selected_npc.is_none_or(|idx| idx >= len) {
                state.selected_npc = Some(0);
            }
        }
    }
}

pub(super) fn refresh_main_menu_content(
    changed_states: Query<&MainMenuState, Or<(Changed<MainMenuState>, Added<MainMenuState>)>>,
    mut text_sets: ParamSet<(
        Query<(&MainMenuHeading, &mut Text)>,
        Query<(&MainMenuLine, &mut Text)>,
    )>,
) {
    // this avoids b0001 by separating channels
    for state in &changed_states {
        let (heading, content) = main_menu_page_content(state.page);

        for (tag, mut text) in &mut text_sets.p0() {
            if tag.owner == state.owner {
                *text = Text::new(heading);
            }
        }

        for (tag, mut text) in &mut text_sets.p1() {
            if tag.owner == state.owner {
                let line_text = content.get(tag.index).copied().unwrap_or("");
                *text = Text::new(line_text);
            }
        }
    }
}

pub(super) fn refresh_main_menu_panels(
    changed_states: Query<&MainMenuState, Or<(Changed<MainMenuState>, Added<MainMenuState>)>>,
    mut panel_sets: ParamSet<(
        Query<(&MainMenuTerminalPanel, &mut Node)>,
        Query<(&MainMenuGalleryPanel, &mut Node)>,
        Query<(&MainMenuSettingsPanel, &mut Node)>,
    )>,
) {
    for state in &changed_states {
        let show_gallery = matches!(
            state.page,
            MainMenuPage::DiscoveredItems | MainMenuPage::PhoneList
        );
        let show_settings = state.page == MainMenuPage::Settings;
        let show_terminal = !show_gallery && !show_settings;

        for (tag, mut node) in &mut panel_sets.p0() {
            if tag.owner == state.owner {
                node.display = if show_terminal {
                    Display::Flex
                } else {
                    Display::None
                };
            }
        }

        for (tag, mut node) in &mut panel_sets.p1() {
            if tag.owner == state.owner {
                node.display = if show_gallery {
                    Display::Flex
                } else {
                    Display::None
                };
            }
        }

        for (tag, mut node) in &mut panel_sets.p2() {
            if tag.owner == state.owner {
                node.display = if show_settings {
                    Display::Flex
                } else {
                    Display::None
                };
            }
        }
    }
}

pub(super) fn refresh_pause_menu_panels(
    changed_states: Query<&PauseMenuState, Or<(Changed<PauseMenuState>, Added<PauseMenuState>)>>,
    mut panel_sets: ParamSet<(
        Query<(&PauseMenuStatusPanel, &mut Node)>,
        Query<(&PauseMenuSettingsPanel, &mut Node)>,
    )>,
) {
    for state in &changed_states {
        let show_settings = state.page == PauseMenuPage::Settings;
        let show_status = !show_settings;

        for (tag, mut node) in &mut panel_sets.p0() {
            if tag.owner == state.owner {
                node.display = if show_status {
                    Display::Flex
                } else {
                    Display::None
                };
            }
        }

        for (tag, mut node) in &mut panel_sets.p1() {
            if tag.owner == state.owner {
                node.display = if show_settings {
                    Display::Flex
                } else {
                    Display::None
                };
            }
        }
    }
}

pub(super) fn rebuild_gallery_lists(
    mut commands: Commands,
    fonts: Res<UiFonts>,
    db: Res<UiDiscoveryDb>,
    states: Query<&MainMenuState>,
    mut list_roots: Query<(Entity, &mut GalleryListCache, &GalleryListRoot)>,
    children_query: Query<&Children>,
) {
    let owners: HashMap<Entity, MainMenuState> = states.iter().map(|it| (it.owner, *it)).collect();

    for (list_entity, mut cache, root) in &mut list_roots {
        let Some(state) = owners.get(&root.owner) else {
            continue;
        };

        let kind = if state.page == MainMenuPage::PhoneList {
            DiscoveryKind::Npc
        } else {
            DiscoveryKind::Item
        };

        let selected = match kind {
            DiscoveryKind::Item => state.selected_item,
            DiscoveryKind::Npc => state.selected_npc,
        };

        let needs_rebuild =
            cache.kind != kind || cache.revision != db.revision() || cache.selected != selected;

        if !needs_rebuild {
            continue;
        }

        if let Ok(children) = children_query.get(list_entity) {
            for child in children.iter() {
                despawn_ui_tree(&mut commands, child, &children_query);
            }
        }

        let entries = db.entries(kind);

        commands.entity(list_entity).with_children(|list| {
            let title = match kind {
                DiscoveryKind::Item => "items",
                DiscoveryKind::Npc => "contacts",
            };

            list.spawn((
                Text::new(format!("{title} [{}]", entries.len())),
                TextFont {
                    font: fonts.pixel.clone(),
                    font_size: 10.0,
                    ..default()
                },
                TextColor(theme::TEXT_LIGHT),
            ));

            if entries.is_empty() {
                list.spawn((
                    Text::new("no entries yet"),
                    TextFont {
                        font: fonts.body.clone(),
                        font_size: 24.0,
                        ..default()
                    },
                    TextColor(theme::CRT_GREEN),
                ));
                return;
            }

            // this is a messy workaround but should work
            for (index, entry) in entries.iter().enumerate() {
                let is_active = selected == Some(index);
                let marker = if entry.seen { "[x]" } else { "[ ]" };

                list.spawn((
                    Button,
                    MenuOwner(root.owner),
                    MenuButton {
                        action: ButtonAction::SelectDiscovery(kind, index),
                        raised: !is_active,
                    },
                    Node {
                        width: Val::Percent(100.0),
                        border: UiRect::all(Val::Px(2.0)),
                        padding: UiRect::all(Val::Px(4.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(1.0),
                        ..default()
                    },
                    BackgroundColor(if is_active {
                        theme::ACCENT_PRIMARY
                    } else {
                        theme::BUTTON_BG
                    }),
                    theme::border(!is_active),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new(format!("{marker} {}", entry.title)),
                        TextFont {
                            font: fonts.body.clone(),
                            font_size: 24.0,
                            ..default()
                        },
                        TextColor(theme::TEXT_DARK),
                    ));
                    button.spawn((
                        Text::new(entry.subtitle.clone()),
                        TextFont {
                            font: fonts.body.clone(),
                            font_size: 19.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.18, 0.18, 0.18)),
                    ));
                });
            }
        });

        cache.owner = root.owner;
        cache.kind = kind;
        cache.revision = db.revision();
        cache.selected = selected;
    }
}

pub(super) fn refresh_gallery_details(
    states: Query<&MainMenuState>,
    db: Res<UiDiscoveryDb>,
    mut details: ParamSet<(
        Query<(&GalleryDetailTitle, &mut Text)>,
        Query<(&GalleryDetailSubtitle, &mut Text)>,
        Query<(&GalleryDetailStatus, &mut Text)>,
        Query<(&GalleryDetailDescription, &mut Text)>,
    )>,
) {
    let owners: HashMap<Entity, MainMenuState> = states.iter().map(|it| (it.owner, *it)).collect();

    for (tag, mut text) in &mut details.p0() {
        let value = owners
            .get(&tag.owner)
            .map(|state| {
                if state.page == MainMenuPage::PhoneList {
                    "CONTACT PROFILE"
                } else {
                    "DISCOVERY PROFILE"
                }
            })
            .unwrap_or("--");
        *text = Text::new(value);
    }

    for (tag, mut text) in &mut details.p1() {
        *text = Text::new(
            resolve_selected_entry(&owners, &db, tag.owner)
                .map_or("no entry selected".to_string(), |it| it.title.clone()),
        );
    }

    for (tag, mut text) in &mut details.p2() {
        let status = resolve_selected_entry(&owners, &db, tag.owner).map_or("".to_string(), |it| {
            let seen = if it.seen { "seen" } else { "not seen" };
            format!("{}  |  {seen}", it.subtitle)
        });
        *text = Text::new(status);
    }

    for (tag, mut text) in &mut details.p3() {
        let desc = resolve_selected_entry(&owners, &db, tag.owner)
            .map_or("select an entry to inspect details".to_string(), |it| {
                it.description.clone()
            });
        *text = Text::new(desc);
    }
}

pub(super) fn refresh_settings_values(
    settings: Res<GameSettings>,
    mut texts: Query<(&SettingsValueText, &mut Text)>,
) {
    for (tag, mut text) in &mut texts {
        let _ = tag.owner;
        *text = Text::new(settings.value_text(tag.key));
    }
}

fn resolve_selected_entry<'a>(
    owners: &HashMap<Entity, MainMenuState>,
    db: &'a UiDiscoveryDb,
    owner: Entity,
) -> Option<&'a DiscoveryEntry> {
    let state = owners.get(&owner)?;
    match state.page {
        MainMenuPage::PhoneList => state
            .selected_npc
            .and_then(|idx| db.entries(DiscoveryKind::Npc).get(idx)),
        MainMenuPage::DiscoveredItems => state
            .selected_item
            .and_then(|idx| db.entries(DiscoveryKind::Item).get(idx)),
        _ => state
            .selected_item
            .and_then(|idx| db.entries(DiscoveryKind::Item).get(idx)),
    }
}

pub(super) fn refresh_button_highlights(
    states: Query<&MainMenuState>,
    mut buttons: Query<(
        &MenuButton,
        &MenuOwner,
        &Interaction,
        &mut BackgroundColor,
        &mut BorderColor,
        Has<DisabledButton>,
    )>,
) {
    let owners: HashMap<Entity, MainMenuState> = states.iter().map(|it| (it.owner, *it)).collect();

    for (button, owner, interaction, mut background, mut border, disabled) in &mut buttons {
        if disabled {
            *background = BackgroundColor(theme::BUTTON_DISABLED);
            *border = theme::border(false);
            continue;
        }

        let Some(state) = owners.get(&owner.0) else {
            continue;
        };

        let active = match button.action {
            ButtonAction::SelectPage(page) => state.page == page,
            ButtonAction::SelectDiscovery(kind, index) => match kind {
                DiscoveryKind::Item =>
                    state.page == MainMenuPage::DiscoveredItems
                        && state.selected_item == Some(index),
                DiscoveryKind::Npc =>
                    state.page == MainMenuPage::PhoneList && state.selected_npc == Some(index),
            },
            _ => false,
        };

        if active {
            *background = BackgroundColor(theme::ACCENT_PRIMARY);
            *border = theme::border(false);
        } else {
            match *interaction {
                Interaction::Hovered => {
                    *background = BackgroundColor(theme::BUTTON_HOVER);
                    *border = theme::border(button.raised);
                }
                Interaction::Pressed => {
                    *background = BackgroundColor(theme::BUTTON_BG);
                    *border = theme::border(false);
                }
                Interaction::None => {
                    *background = BackgroundColor(theme::BUTTON_BG);
                    *border = theme::border(button.raised);
                }
            }
        }
    }
}

pub(super) fn animate_dither_pixels(
    time: Res<Time>,
    mut pixels: Query<(&DitherPixel, &mut BackgroundColor)>,
) {
    let t = time.elapsed_secs();
    for (pixel, mut bg) in &mut pixels {
        let intensity = ((t * pixel.speed + pixel.phase).sin() * 0.5 + 0.5) * 0.7;
        let [br, bgc, bb, _] = pixel.base.to_srgba().to_f32_array();
        let [ar, ag, ab, _] = pixel.accent.to_srgba().to_f32_array();
        let r = br + (ar - br) * intensity;
        let g = bgc + (ag - bgc) * intensity;
        let b = bb + (ab - bb) * intensity;
        bg.0 = Color::srgb(r, g, b);
    }
}

fn main_menu_page_content(page: MainMenuPage) -> (&'static str, &'static [&'static str]) {
    match page {
        MainMenuPage::Home => (
            "SYSTEM STATUS",
            &[
                "terminal online",
                "",
                "new game: available",
                "continue: waiting for save state",
                "",
                "open credits or gallery on left",
            ],
        ),
        MainMenuPage::Credits => (
            "CREDITS",
            &[
                "creative direction: doomy & guilhhotina",
                "engineering: rust + bevy 0.18",
                "",
                "font: press start 2p (ofl)",
                "font: vt323 (ofl)",
                "",
            ],
        ),
        MainMenuPage::DiscoveredItems => (
            "DISCOVERED ITEMS",
            &[
                "gallery mode online",
                "click on an item",
                "",
                "left panel = list",
                "right panel = details",
                "",
                "",
                "",
            ],
        ),
        MainMenuPage::PhoneList => (
            "PHONE LIST",
            &[
                "contact gallery online",
                "click on a contact",
                "",
                "left panel = list",
                "right panel = details",
                "",
                "",
                "",
            ],
        ),
        MainMenuPage::Settings => (
            "SETTINGS",
            &[
                "runtime controls online",
                "audio, voice and ui tuning",
                "",
                "left button = decrease/toggle",
                "right button = increase/toggle",
                "",
            ],
        ),
    }
}

fn despawn_ui_tree(commands: &mut Commands, entity: Entity, children_query: &Query<&Children>) {
    if let Ok(children) = children_query.get(entity) {
        for child in children.iter() {
            despawn_ui_tree(commands, child, children_query);
        }
    }
    commands.entity(entity).despawn();
}
