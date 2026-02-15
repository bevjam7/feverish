use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};
use bevy_seedling::sample::SamplePlayer;
use bevy_trenchbroom::prelude::*;

use crate::{
    Phase, Usable,
    audio::mixer::WorldSfxPool,
    gameplay::{
        DoorPortal, EliminationCount, EmitHook, HookCounter,
        npc::{Npc, SuspectType},
    },
    input::Use,
    ratspinner::RatHookTriggered,
    ui::UiEndingCommandsExt,
};

/// Base door class from which other types of doors inherit
#[base_class(base(EmitHook, Targetable))]
#[derive(Component, Clone, Reflect)]
#[require(Usable)]
pub struct DoorBase {
    pub(crate) open: bool,
    pub(crate) locked: bool,
    pub(crate) sound_locked: String,
    pub(crate) sound_open: String,
    pub(crate) sound_close: String,
}

impl Default for DoorBase {
    fn default() -> Self {
        Self {
            open: false,
            locked: false,
            sound_locked: "audio/door_locked.ogg".into(),
            sound_open: "audio/door_open.ogg".into(),
            sound_close: "audio/door_close.ogg".into(),
        }
    }
}

#[base_class(base(DoorBase))]
#[derive(Component, Clone)]
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
        assets: Res<AssetServer>,
        mut door: Query<(
            Entity,
            &mut DoorBase,
            &DoorRotatingBase,
            &EmitHook,
            &mut HookCounter,
        )>,
        mut cmd: Commands,
        mut signal_messages: MessageWriter<RatHookTriggered>,
    ) {
        let (door_entity, mut door, door_rotating, maybe_signal, mut signal_counter) =
            door.get_mut(event.event_target()).unwrap();

        // Send signals
        if signal_counter.0 == 0 || maybe_signal.hook_repeat {
            if let Some(ref signal) = maybe_signal.hook {
                signal_messages.write(RatHookTriggered {
                    hook: signal.clone(),
                    script_id: Default::default(),
                    node_id: Default::default(),
                    option_id: None,
                    target: None,
                });
            }
            signal_counter.0 += 1;
        }

        let sample_to_play: Option<_>;
        match door.open {
            // If the door is already open, it can be closed without further checks
            true => {
                door.open = false;
                cmd.entity(door_entity).insert(DoorAnimationTimer::new(
                    door_rotating.animation_seconds,
                    door.open,
                ));
                sample_to_play = Some(door.sound_close.clone());
            }
            // If the door is closed, check if it is locked.
            false => match door.locked {
                // Do nothing if the door is locked
                true => {
                    sample_to_play = Some(door.sound_locked.clone());
                }
                // Open the door if unlocked
                false => {
                    door.open = true;
                    cmd.entity(door_entity).insert(DoorAnimationTimer::new(
                        door_rotating.animation_seconds,
                        door.open,
                    ));
                    sample_to_play = Some(door.sound_open.clone());
                }
            },
        }

        if let Some(sample) = sample_to_play {
            cmd.entity(door_entity).with_child((
                SamplePlayer::new(assets.get_handle(sample).unwrap())
                    .with_volume(bevy_seedling::prelude::Volume::Linear(0.5)),
                // bevy_seedling::sample_effects![HrtfNode::default()],
                WorldSfxPool,
            ));
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

/// Not exactly a door, but we'll have the endgame screen play for win states
/// when the player leaves.
#[solid_class(group("func"), classname("door_end"), base(Transform, DoorPortal))]
#[derive(Component, Clone, Copy, Default)]
#[component(on_add=Self::on_add_hook)]
#[require(Usable)]
pub(crate) struct EndDoor;

impl EndDoor {
    fn on_add_hook(mut world: DeferredWorld, hook: HookContext) {
        if world.is_scene_world() {
            return;
        }
        world.commands().entity(hook.entity).observe(Self::on_use);
    }

    fn on_use(_on: On<Use>, npcs: Query<&Npc>, phase: Res<State<Phase>>, mut cmd: Commands) {
        if let Phase::Win = phase.get() {
            // Check to see if the human npc was killed
            let spared = npcs
                .iter()
                .any(|x| matches!(x.suspect, Some(SuspectType::Human)));

            dbg!("Human spared: {}", spared);
            match spared {
                true => {
                    cmd.show_ending("win_spared");
                }
                false => {
                    cmd.show_ending("win_killed");
                }
            };
        }
    }
}
