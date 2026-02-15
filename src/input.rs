use avian3d::prelude::{Collider, RayCaster, RayHits, SpatialQueryFilter};
use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use bevy_enhanced_input::prelude::*;

use crate::{
    GameState,
    gameplay::{ColliderHierarchyChildOf, PhysLayer, Player},
    ratspinner::RatDialogueState,
    ui::{
        DialogueUiRoot, EndingUiRoot, InventoryUiRoot, MainMenuUi, PauseMenuUi, UiDialogueCommand,
    },
};

pub(crate) struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_sub_state::<PlayerInputState>()
            .add_input_context::<PlayerInput>()
            .add_observer(apply_use)
            .add_systems(
                Update,
                (sync_player_input_state, update_use_caster).run_if(in_state(GameState::Main)),
            )
            .add_systems(OnEnter(PlayerInputState::Active), activate_player_input)
            .add_systems(OnEnter(PlayerInputState::Locked), lock_player_input);
    }
}

#[derive(SubStates, Clone, Eq, PartialEq, Hash, Debug, Default)]
#[source(GameState = GameState::Main)]
pub(crate) enum PlayerInputState {
    #[default]
    Active,
    Locked,
}

pub(crate) fn controller_bundle() -> impl Bundle {
    (
        bevy_ahoy::CharacterController {
            standing_view_height: 1.6,
            stop_speed: 2.0,
            friction_hz: 16.0,
            crouch_speed_scale: 1.0 / 2.0,
            speed: 5.0,
            max_speed: 50.0,
            jump_height: 1.0,
            ..default()
        },
        PlayerInput,
        actions!(PlayerInput[
            (
                Action::<bevy_ahoy::input::Movement>::new(),
                DeadZone::default(),
                Bindings::spawn((
                    Cardinal::wasd_keys(),
                    Axial::left_stick()
                ))
            ),
            (
                Action::<UseAction>::new(),
                bindings![KeyCode::KeyE,  GamepadButton::West],
            ),
            (
                Action::<bevy_ahoy::input::Jump>::new(),
                bindings![KeyCode::Space, GamepadButton::South],
            ),
            (
                Action::<bevy_ahoy::input::Crouch>::new(),
                bindings![KeyCode::ControlLeft, GamepadButton::LeftTrigger],
            ),
            (
                Action::<bevy_ahoy::input::RotateCamera>::new(),
                Scale::splat(0.04),
                Bindings::spawn((
                    Spawn(Binding::mouse_motion()),
                    Axial::right_stick()
                ))
            ),
        ]),
    )
}

#[derive(Component)]
pub struct PlayerInput;

#[derive(Debug, InputAction)]
#[action_output(bool)]
pub struct UseAction;

/// Interact with various things in the environment
#[derive(EntityEvent, Reflect)]
pub struct Use(pub Entity);

#[derive(Component)]
#[require(IsHovering)]
pub struct UseRaycaster;

/// Tells us if we're hovering an item in the world
#[derive(Component, Default)]
pub struct IsHovering(pub(crate) Vec<Entity>);

fn apply_use(
    _action: On<Start<UseAction>>,
    mut cmd: Commands,
    caster: Single<
        (Entity, &RayCaster, &GlobalTransform, &mut IsHovering),
        (With<UseRaycaster>, With<Player>),
    >,
    dialogue_ui: Query<(), With<DialogueUiRoot>>,
) {
    if !dialogue_ui.is_empty() {
        cmd.write_message(UiDialogueCommand::Advance);
        return;
    }

    if let Some(hit) = caster.3.0.iter().next() {
        cmd.entity(*hit).trigger(Use);
    }
}

fn update_use_caster(
    mut caster: Single<
        (Entity, &RayCaster, &GlobalTransform, &mut IsHovering),
        (With<UseRaycaster>, With<Player>),
    >,
    spatial: avian3d::spatial_query::SpatialQuery,
    hits: Query<&RayHits>,
    hierarchy: Query<&ColliderHierarchyChildOf>,
) {
    const USE_RADIUS: f32 = 0.6;

    // Get either the hit, or the endpoint of the raycaster

    let endpoint = match hits.get(caster.0) {
        Ok(hits) =>
            (caster.2.forward()
                * hits
                    .get(0)
                    .map(|x| x.distance)
                    .unwrap_or(caster.1.max_distance))
                + caster.2.translation(),
        Err(_) => (caster.2.forward() * caster.1.max_distance) + caster.2.translation(),
    };

    // Do another sphere cast at the endpoint
    let intersections = spatial.shape_intersections(
        &Collider::sphere(USE_RADIUS),
        endpoint,
        Quat::default(),
        &SpatialQueryFilter {
            mask: [PhysLayer::Usable].into(),
            excluded_entities: [].into(),
        },
    );

    // Find collider roots, if applicable
    caster.3.0 = intersections
        .iter()
        .map(|intersection| match hierarchy.get(*intersection).ok() {
            Some(ColliderHierarchyChildOf(entity)) => entity,
            None => intersection,
        })
        .copied()
        .collect();
}

fn sync_player_input_state(
    dialogue: Res<RatDialogueState>,
    ui_lock: Query<
        (),
        Or<(
            With<MainMenuUi>,
            With<PauseMenuUi>,
            With<DialogueUiRoot>,
            With<InventoryUiRoot>,
            With<EndingUiRoot>,
        )>,
    >,
    state: Res<State<PlayerInputState>>,
    mut next_state: ResMut<NextState<PlayerInputState>>,
) {
    let should_lock = dialogue.active || !ui_lock.is_empty();
    let target = if should_lock {
        PlayerInputState::Locked
    } else {
        PlayerInputState::Active
    };
    if state.get() != &target {
        next_state.set(target);
    }
}

fn activate_player_input(
    mut commands: Commands,
    players: Query<Entity, (With<Player>, With<PlayerInput>)>,
    mut windows: Query<&mut CursorOptions>,
) {
    if let Ok(mut cursor_options) = windows.single_mut() {
        cursor_options.grab_mode = CursorGrabMode::None;
    }
    for entity in &players {
        commands
            .entity(entity)
            .insert(ContextActivity::<PlayerInput>::ACTIVE);
    }
}

fn lock_player_input(
    mut commands: Commands,
    players: Query<(Entity, &Actions<PlayerInput>), (With<Player>, With<PlayerInput>)>,
    mut action_values: Query<&mut ActionValue>,
    mut action_states: Query<&mut ActionState>,
    mut action_events: Query<&mut ActionEvents>,
    mut action_times: Query<&mut ActionTime>,
    mut movement_actions: Query<&mut Action<bevy_ahoy::input::Movement>>,
    mut jump_actions: Query<&mut Action<bevy_ahoy::input::Jump>>,
    mut crouch_actions: Query<&mut Action<bevy_ahoy::input::Crouch>>,
    mut rotate_actions: Query<&mut Action<bevy_ahoy::input::RotateCamera>>,
    mut use_actions: Query<&mut Action<UseAction>>,
    mut windows: Query<&mut CursorOptions>,
) {
    if let Ok(mut cursor_options) = windows.single_mut() {
        cursor_options.grab_mode = CursorGrabMode::None;
    }
    for (entity, actions) in &players {
        // clear any latched values once before disabling the input context
        for action_entity in actions.iter() {
            if let Ok(mut value) = action_values.get_mut(action_entity) {
                *value = ActionValue::zero(value.dim());
            }
            if let Ok(mut state) = action_states.get_mut(action_entity) {
                *state = ActionState::None;
            }
            if let Ok(mut events) = action_events.get_mut(action_entity) {
                *events = ActionEvents::default();
            }
            if let Ok(mut time) = action_times.get_mut(action_entity) {
                *time = ActionTime::default();
            }

            if let Ok(mut action) = movement_actions.get_mut(action_entity) {
                *action = Action::default();
            }
            if let Ok(mut action) = jump_actions.get_mut(action_entity) {
                *action = Action::default();
            }
            if let Ok(mut action) = crouch_actions.get_mut(action_entity) {
                *action = Action::default();
            }
            if let Ok(mut action) = rotate_actions.get_mut(action_entity) {
                *action = Action::default();
            }
            if let Ok(mut action) = use_actions.get_mut(action_entity) {
                *action = Action::default();
            }
        }

        commands
            .entity(entity)
            .insert(ContextActivity::<PlayerInput>::INACTIVE);
    }
}
