mod door;
mod focus_fx;
mod inventory;
mod npc;
mod props;
mod sound;

use std::collections::HashMap;

use avian3d::prelude::*;
use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};
use bevy_ahoy::prelude::*;
use bevy_seedling::spatial::SpatialListener3D;
use bevy_trenchbroom::prelude::*;

use crate::{
    AppSystems, Usable,
    input::{Use, UseRaycaster},
    map::{LevelToPrepare, PendingLevelTransition},
    psx::{PsxCamera, PsxConfig},
};

pub(crate) struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DoorScenePreloads>();
        app.add_systems(
            Update,
            (
                preload_door_target_levels,
                handle_added_spawn_point_camera,
                (sound::detect_footstep_surface, sound::handle_footsteps).chain(),
                door::rotate_doors,
                focus_fx::handle_focus_effect,
            )
                .in_set(AppSystems::Update),
        );
    }
}

#[derive(Resource, Default)]
struct DoorScenePreloads(HashMap<String, Handle<Scene>>);

/// Marks an entity as owned by the player. Note that this does *not* refer to a
/// specific entity, but should instead be combined with other queries.
#[derive(Component, Default)]
pub(crate) struct Player;

#[derive(Component)]
#[require(Player)]
pub(crate) struct PlayerRoot;

#[point_class(group("player"), classname("spawn"), size(-16 -16 -32, 16 16 32), base(Transform))]
#[derive(Clone, Copy)]
#[component(on_add=Self::on_add_hook)]
pub struct SpawnPoint;

impl SpawnPoint {
    fn on_add_hook(mut world: DeferredWorld, hook: HookContext) {
        if world.is_scene_world() {
            return;
        }
        world
            .commands()
            .entity(hook.entity)
            .insert(SpatialListener3D::default());
    }
}

/// Transition between two doors across different levels
#[solid_class(group("func"), classname("door_portal"), base(Transform, Target))]
#[derive(Clone, Default)]
#[component(on_add=Self::on_add_hook)]
#[require(Usable)]
pub struct DoorPortal {
    /// If none, attempt to find a door portal target within the same level and
    /// move there
    level: Option<String>,
}

impl DoorPortal {
    fn on_add_hook(mut world: DeferredWorld, hook: HookContext) {
        if world.is_scene_world() {
            return;
        }
        world.commands().entity(hook.entity).observe(Self::on_use);
    }

    fn on_use(
        trigger: On<Use>,
        targets: Query<&Target>,
        doors: Query<&Self>,
        mut pending_transition: ResMut<PendingLevelTransition>,
        mut preloads: ResMut<DoorScenePreloads>,
        assets: Res<AssetServer>,
    ) {
        let door_portal = doors.get(trigger.0).unwrap();
        let target_level = door_portal.level.clone().expect(
            "Transitioning between two doors in the same map is not yet supported. A target level \
             must be set.",
        );
        let target_name = targets
            .get(trigger.0)
            .expect("Door portal must have a set target.")
            .target
            .0
            .clone();

        let handle = preloads
            .0
            .entry(target_level.clone())
            .or_insert_with(|| assets.load(format!("maps/{target_level}.map#Scene")))
            .clone();

        // Queue transition and let map plugin switch states once dependencies are
        // ready.
        pending_transition.level = Some(handle);
        pending_transition.portal_target = Some(target_name);
    }
}

fn preload_door_target_levels(
    mut preloads: ResMut<DoorScenePreloads>,
    doors: Query<&DoorPortal, Added<DoorPortal>>,
    assets: Res<AssetServer>,
) {
    for door in &doors {
        let Some(level) = &door.level else {
            continue;
        };
        preloads
            .0
            .entry(level.clone())
            .or_insert_with(|| assets.load(format!("maps/{level}.map#Scene")));
    }
}

#[point_class(group("door"), classname("portal_target"), size(-16 -16 -32, 16 16 32), base(Transform, Targetable))]
#[derive(Clone, Copy, Default)]
pub struct DoorPortalTarget;

fn handle_added_spawn_point_camera(
    mut cmd: Commands,
    added: Query<(Entity, &GlobalTransform), Added<SpawnPoint>>,
    level_to_prepare: Res<LevelToPrepare>,
    door_targets: Query<(&Targetable, &GlobalTransform), With<DoorPortalTarget>>,
    assets: Res<AssetServer>,
) {
    const MAX_INTERACTION_DISTANCE: f32 = 3.0;

    if added.count() > 1 {
        error!("Multiple spawn points detected.");
    }
    if let Some((entity, added)) = added.iter().next() {
        let target_transform = {
            match (
                level_to_prepare.level.as_ref(),
                level_to_prepare.portal_target.as_ref(),
            ) {
                (Some(_level), Some(portal_target)) => {
                    // Move the player to the desired portal door exit
                    let (_, target_transform) = door_targets
                        .iter()
                        .find(|(name, _)| &name.targetname.0 == portal_target)
                        .expect(&format!("Door target `{portal_target}` not found"));
                    target_transform.compute_transform()
                }
                _ => added.compute_transform(),
            }
        };

        // Character collider and player root
        let player_root = cmd
            .entity(entity)
            .insert((
                crate::input::controller_bundle(),
                target_transform,
                Player,
                PlayerRoot,
                sound::default_footstep_player(assets.as_ref()),
            ))
            .id();

        // Camera for our character collider
        let camera_entity = cmd
            .spawn((
                crate::camera::player_camera_bundle(),
                PsxCamera,
                PsxConfig::default(),
                CharacterControllerCameraOf::new(player_root),
                Player,
            ))
            .id();

        // Raycaster for our use functionality
        cmd.entity(camera_entity).with_child((
            Player,
            UseRaycaster,
            RayCaster::new(Vec3::ZERO, Dir3::NEG_Z)
                .with_max_distance(MAX_INTERACTION_DISTANCE)
                .with_query_filter(SpatialQueryFilter {
                    mask: [PhysLayer::Default, PhysLayer::Usable].into(),
                    ..Default::default()
                })
                .with_max_hits(1),
        ));
    }
}

#[derive(PhysicsLayer, Default)]
pub enum PhysLayer {
    #[default]
    Default,
    Usable,
    Npc,
    Prop,
}

#[derive(Component)]
pub(crate) struct ColliderHierarchyChildOf(pub(crate) Entity);

fn link_hierarchal_colliders(
    trigger: On<ColliderConstructorHierarchyReady>,
    children: Query<&Children>,
    colliders: Query<&Collider>,
    mut cmd: Commands,
) {
    for child in children.iter_descendants(trigger.entity) {
        if colliders.contains(child) {
            cmd.entity(child)
                .insert(ColliderHierarchyChildOf(trigger.entity));
        }
    }
}
