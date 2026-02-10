use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};
use bevy_trenchbroom::prelude::*;

use crate::{Usable, input::Use};

/// Base door class from which other types of doors inherit
#[base_class]
#[derive(Component, Clone, Copy, Default, Reflect)]
#[require(Usable)]
pub struct DoorBase {
    open: bool,
    locked: bool,
}

#[base_class(base(DoorBase))]
#[derive(Component, Clone, Copy)]
#[component(immutable, on_add=Self::on_add_hook)]
pub struct DoorRotatingBase {
    open_degrees: f32,
    animation_seconds: f32,
    axis: Vec3,
}

#[solid_class(
    group("func"),
    classname("door_rotating"),
    base(DoorRotatingBase, Transform, Targetable)
)]
#[derive(Component, Clone, Copy, Default)]
pub struct DoorRotatingSolid {
    open_degrees: f32,
    animation_seconds: f32,
}

impl Default for DoorRotatingBase {
    fn default() -> Self {
        Self {
            open_degrees: Self::DEFAULT_OPEN_DEGREES,
            animation_seconds: Self::ANIMATION_SECONDS,
            axis: Self::AXIS,
        }
    }
}

impl DoorRotatingBase {
    const ANIMATION_SECONDS: f32 = 0.8;
    const AXIS: Vec3 = Vec3::Y;
    const DEFAULT_OPEN_DEGREES: f32 = 100.0;

    fn on_add_hook(mut world: DeferredWorld, hook: HookContext) {
        if world.is_scene_world() {
            return;
        }

        let door_base = world.get::<DoorBase>(hook.entity).unwrap();
        let open_angle = world
            .get::<DoorRotatingBase>(hook.entity)
            .unwrap()
            .open_degrees
            .to_radians();

        let mut transform = world.get::<Transform>(hook.entity).unwrap().clone();
        if door_base.open {
            transform.rotate_local_y(open_angle);
        }

        // Build observers
        world
            .commands()
            .entity(hook.entity)
            .insert(transform)
            .observe(Self::on_use_door);
    }

    fn on_use_door(
        event: On<Use>,
        mut door: Query<(Entity, &mut DoorBase, &DoorRotatingBase)>,
        mut cmd: Commands,
    ) {
        let (door_entity, mut door, door_rotating) = door.get_mut(event.event_target()).unwrap();
        match door.open {
            // If the door is already open, it can be closed without further checks
            true => {
                door.open = false;
                cmd.entity(door_entity).insert(DoorAnimationTimer::new(
                    door_rotating.animation_seconds,
                    door.open,
                ));
            }
            // If the door is closed, check if it is locked.
            false => match door.locked {
                // Do nothing if the door is locked
                true => (),
                // Open the door if unlocked
                false => {
                    door.open = true;
                    cmd.entity(door_entity).insert(DoorAnimationTimer::new(
                        door_rotating.animation_seconds,
                        door.open,
                    ));
                }
            },
        }
    }
}

#[derive(Component)]
pub(crate) struct DoorAnimationTimer {
    timer: Timer,
    opening: bool,
}

impl DoorAnimationTimer {
    pub(crate) fn new(seconds: f32, open: bool) -> Self {
        Self {
            timer: Timer::from_seconds(seconds, TimerMode::Once),
            opening: open,
        }
    }
}

pub(super) fn rotate_doors(
    mut cmd: Commands,
    doors: Query<
        (
            Entity,
            &mut DoorAnimationTimer,
            &DoorRotatingBase,
            &mut Transform,
        ),
        With<DoorRotatingBase>,
    >,
    time: Res<Time>,
) {
    for (door_entity, mut animation, door, mut door_transform) in doors {
        animation.timer.tick(time.delta());

        let end = match animation.opening {
            true => door.open_degrees,
            false => 0.0,
        };

        door_transform.rotation = door_transform.rotation.slerp(
            Quat::from_axis_angle(door.axis, end.to_radians()),
            animation.timer.fraction(),
        );

        if animation.timer.is_finished() {
            cmd.entity(door_entity).remove::<DoorAnimationTimer>();
        }
    }
}
