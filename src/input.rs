use avian3d::prelude::{CollisionLayers, RayHits};
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::{
    gameplay::{PhysLayer, Player},
    ratspinner::RatDialogueState,
    ui::{DialogueUiRoot, UiDialogueCommand},
};

pub(crate) struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_input_context::<PlayerInput>()
            .add_observer(apply_use)
            .add_systems(Update, sync_player_input_lock);
    }
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
pub struct UseRaycaster;

#[derive(Component)]
struct DialogueInputLocked;

fn apply_use(
    _action: On<Start<UseAction>>,
    mut cmd: Commands,
    caster: Single<Entity, (With<UseRaycaster>, With<Player>)>,
    hits: Query<&RayHits>,
    layers: Query<&CollisionLayers>,
    dialogue_ui: Query<(), With<DialogueUiRoot>>,
) {
    if !dialogue_ui.is_empty() {
        cmd.write_message(UiDialogueCommand::Advance);
        return;
    }

    let hits = hits.get(caster.entity()).unwrap();
    if let Some(hit) = hits.first() {
        // check if the layer is usable. we also collide with walls to prevent using
        // things through the environment
        let layer = layers.get(hit.entity).unwrap();
        if layer.memberships.has_all(PhysLayer::Usable) {
            cmd.entity(hit.entity).trigger(Use);
        }
    }
}

fn sync_player_input_lock(
    dialogue: Res<RatDialogueState>,
    mut commands: Commands,
    enabled: Query<
        (Entity, &Actions<PlayerInput>),
        (
            With<Player>,
            With<PlayerInput>,
            Without<DialogueInputLocked>,
        ),
    >,
    locked: Query<Entity, (With<Player>, With<PlayerInput>, With<DialogueInputLocked>)>,
    mut action_values: Query<&mut ActionValue>,
    mut action_states: Query<&mut ActionState>,
    mut action_events: Query<&mut ActionEvents>,
    mut action_times: Query<&mut ActionTime>,
    mut movement_actions: Query<&mut Action<bevy_ahoy::input::Movement>>,
    mut jump_actions: Query<&mut Action<bevy_ahoy::input::Jump>>,
    mut crouch_actions: Query<&mut Action<bevy_ahoy::input::Crouch>>,
    mut rotate_actions: Query<&mut Action<bevy_ahoy::input::RotateCamera>>,
    mut use_actions: Query<&mut Action<UseAction>>,
    mut lock_logged: Local<bool>,
) {
    if dialogue.active {
        for (entity, actions) in &enabled {
            // clear latched action values from the current frame bfore disabling context
            // updates
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
                .insert(ContextActivity::<PlayerInput>::INACTIVE)
                .insert(DialogueInputLocked);
        }
        if !*lock_logged {
            info!("dialogue active: locking player input");
            *lock_logged = true;
        }
    } else {
        for entity in &locked {
            commands
                .entity(entity)
                .insert(ContextActivity::<PlayerInput>::ACTIVE)
                .remove::<DialogueInputLocked>();
        }
        if *lock_logged {
            info!("dialogue closed: restoring player input");
            *lock_logged = false;
        }
    }
}
