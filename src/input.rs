use avian3d::prelude::{CollisionLayers, RayHits};
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

use crate::gameplay::{PhysLayer, Player};

pub(crate) struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_input_context::<PlayerInput>()
            .add_observer(apply_use);
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

fn apply_use(
    _action: On<Start<UseAction>>,
    mut cmd: Commands,
    caster: Single<Entity, (With<UseRaycaster>, With<Player>)>,
    hits: Query<&RayHits>,
    layers: Query<&CollisionLayers>,
) {
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
