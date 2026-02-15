use std::{collections::VecDeque, iter, time::Duration};

use avian3d::prelude::{ColliderConstructor, CollisionLayers, RigidBody};
use bevy::{
    asset::AssetPath,
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    platform::collections::HashMap,
    prelude::*,
    scene::SceneInstanceReady,
};
use bevy_trenchbroom::prelude::*;

use crate::{
    gameplay::{ColliderHierarchyChildOf, PhysLayer, props::AnimationControls},
    input::Use,
    ratspinner::{RatCommand, RatHookTriggered, RatStart},
    ui::dialogue::UiDialogueState,
};

#[point_class(base(Transform, Visibility, Target), model("models/npc_a/npc_a.gltf"))]
#[component(on_add=Self::on_add_hook)]
#[require(Navigator)]
pub(crate) struct Npc {
    #[class(default = "models/npc_a/npc_a.gltf", must_set)]
    pub(crate) model: String,
    #[class(default = "idle_a")]
    pub(crate) idle_animation: Option<String>,
    /// Marks the NPC as one that must be eliminated or saved
    pub(crate) suspect: Option<SuspectType>,
    pub(crate) script_id: Option<String>,
    pub(crate) starting_walk_node: Option<String>,
    #[class(ignore)]
    pub(crate) default_script_id: Option<String>,
}

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub(crate) struct Navigator {
    pub(crate) path: VecDeque<Entity>,
    pub(crate) queue: Vec<Entity>,
}

#[point_class(base(Transform, Target, Targetable))]
pub(crate) struct WalkTarget;

#[derive(Reflect, FgdType)]
pub(crate) enum SuspectType {
    Imposter,
    Human,
}

impl Default for Npc {
    fn default() -> Self {
        Self {
            model: Default::default(),
            idle_animation: None,
            suspect: None,
            script_id: None,
            starting_walk_node: None,
            default_script_id: None,
        }
    }
}

impl Npc {
    const WAIT_SECS: u64 = 15;

    fn on_add_hook(mut world: DeferredWorld, hook: HookContext) {
        if world.is_scene_world() {
            return;
        }
        let mut npc = world.get_mut::<Self>(hook.entity).unwrap();
        npc.default_script_id = npc.script_id.clone();
        let npc = world.get::<Self>(hook.entity).unwrap();
        let asset_path = AssetPath::from(&npc.model);
        let assets = world.resource::<AssetServer>();
        let scene_asset_path = AssetPath::from(asset_path.to_string() + "#Scene0");
        let scene_handle = assets.get_handle(scene_asset_path).unwrap();

        const HEIGHT: f32 = 1.8;
        let layers = CollisionLayers::new(
            [PhysLayer::Npc, PhysLayer::Usable],
            [PhysLayer::Npc, PhysLayer::Default, PhysLayer::Usable],
        );
        world
            .commands()
            .entity(hook.entity)
            .insert(SceneRoot(scene_handle.clone()))
            .with_children(|cmd| {
                // Spawn the NPC collider in the center, since the npc models origins are at the
                // feet
                cmd.spawn((
                    ColliderConstructor::Capsule {
                        radius: 0.5,
                        height: HEIGHT,
                    },
                    layers,
                    RigidBody::Kinematic,
                    Transform::from_translation(Vec3::Y * HEIGHT * 0.5),
                    ColliderHierarchyChildOf(cmd.target_entity()),
                ));
            })
            .observe(Self::setup_animations)
            .observe(Self::on_use)
            .observe(Self::on_walk_start)
            .observe(Self::on_arrived_home)
            .observe(Self::on_arrived_destination);
    }

    pub(crate) fn setup_animations(
        scene_ready: On<SceneInstanceReady>,
        mut cmd: Commands,
        npcs: Query<&Npc>,
        assets: Res<AssetServer>,
        mut animators: Query<&mut AnimationPlayer>,
        mut graphs: ResMut<Assets<AnimationGraph>>,
        gltfs: Res<Assets<Gltf>>,
        children: Query<&Children>,
    ) {
        let npc = npcs.get(scene_ready.entity).unwrap();
        let asset_path = AssetPath::from(&npc.model);
        let gltf_handle = assets.get_handle(asset_path).unwrap();
        let gltf = gltfs.get(&gltf_handle).unwrap();

        let ref animations = gltf.named_animations;

        let mut graph = AnimationGraph::new();

        let clips: Vec<&'static str> = animations
            .keys()
            .map(|k| -> &'static str { Box::leak(k.clone()) })
            .collect();
        let register_animation = |clip_name: &'static str| -> (&'static str, AnimationNodeIndex) {
            (
                clip_name,
                graph.add_clip(
                    animations
                        .get(clip_name)
                        .expect(&format!("No animation named {clip_name}"))
                        .clone(),
                    1.0,
                    graph.root,
                ),
            )
        };
        let animations: HashMap<_, _> = clips.into_iter().map(register_animation).collect();
        let graph_handle = graphs.add(graph);

        if let Some(idle_animation_id) = npc.idle_animation.as_ref() {
            let node = animations.get(idle_animation_id.as_str()).unwrap();
            for child in
                iter::once(scene_ready.entity).chain(children.iter_descendants(scene_ready.entity))
            {
                if let Ok(mut animator) = animators.get_mut(child) {
                    let mut transitions = AnimationTransitions::new();
                    transitions
                        .play(&mut animator, *node, Duration::ZERO)
                        .repeat();
                    cmd.entity(child).insert((
                        transitions,
                        AnimationControls {
                            animations: animations.clone(),
                            graph_handle: graph_handle.clone(),
                        },
                        AnimationGraphHandle(graph_handle.clone()),
                    ));
                }
            }
        }
    }

    pub(crate) fn transition_to_animation_one_shot(
        name: In<(Entity, impl AsRef<str>, bool)>,
        controls: Query<&AnimationControls>,
        mut animators: Query<&mut AnimationPlayer>,
        children: Query<&Children>,
        mut transitions: Query<&mut AnimationTransitions>,
    ) {
        let (entity, name, repeat) = name.0;

        for child in iter::once(entity).chain(children.iter_descendants(entity)) {
            if let Ok(mut animator) = animators.get_mut(child) {
                let control = controls.get(child).unwrap();
                let node = control.animations.get(name.as_ref()).unwrap();
                let mut transitions = transitions.get_mut(child).unwrap();
                let active_anim =
                    transitions.play(&mut animator, *node, Duration::from_secs_f32(0.5));
                if repeat {
                    active_anim.repeat();
                }
            }
        }
    }

    fn on_use(trigger: On<Use>, mut cmd: Commands, npcs: Query<&Npc>) {
        let Ok(npc) = npcs.get(trigger.0) else {
            return;
        };
        if let Some(ref script_id) = npc.script_id {
            cmd.write_message(RatCommand::Start(
                RatStart::new(script_id.clone()).target(trigger.0),
            ));
        }
    }

    fn on_walk_start(trigger: On<StartedWalk>, mut cmd: Commands, mut npcs: Query<&mut Self>) {
        let mut npc = npcs.get_mut(trigger.0).unwrap();
        // Disable talking while walking
        npc.script_id = None;
        cmd.run_system_cached_with(
            Npc::transition_to_animation_one_shot,
            (trigger.0, "walk", true),
        );
    }

    fn on_arrived_destination(
        trigger: On<ArrivedAtDestination>,
        mut cmd: Commands,
        mut npcs: Query<&mut Self>,
    ) {
        cmd.run_system_cached_with(
            Npc::transition_to_animation_one_shot,
            (trigger.0, "idle_a", true),
        );
        cmd.entity(trigger.0).insert(WalkbackTimer(Timer::new(
            Duration::from_secs(Npc::WAIT_SECS),
            TimerMode::Once,
        )));
        // Disable talking while walking
        let mut npc = npcs.get_mut(trigger.0).unwrap();
        npc.script_id = npc
            .default_script_id
            .as_ref()
            .map(|x| x.to_string() + ".lured")
            .clone();
    }

    fn on_arrived_home(
        trigger: On<ArrivedHome>,
        mut cmd: Commands,
        mut npcs: Query<(&mut Npc, &mut Transform)>,
        global_transforms: Query<&GlobalTransform>,
        targetables: Query<(Entity, &Targetable), With<WalkTarget>>,
    ) {
        let (mut npc, mut transform) = npcs.get_mut(trigger.0).unwrap();
        cmd.run_system_cached_with(
            Npc::transition_to_animation_one_shot,
            (trigger.0, npc.idle_animation.clone().unwrap(), true),
        );
        // Make the npc face the correct direction
        let walk_node_by_name = |name: &String| {
            targetables
                .iter()
                .find(|(_, Targetable { targetname })| &targetname.0 == name)
                .map(|x| x.0)
        };
        let node_gt = global_transforms
            .get(walk_node_by_name(npc.starting_walk_node.as_ref().unwrap()).unwrap())
            .unwrap();
        transform.translation = node_gt.translation();
        transform.rotation = node_gt.rotation();

        npc.script_id = npc.default_script_id.clone();
    }
}

pub(crate) fn tick_walkback_timers(
    query: Query<(Entity, &mut WalkbackTimer)>,
    mut navigators: Query<&mut Navigator>,
    mut cmd: Commands,
    state: Res<UiDialogueState>,
    time: Res<Time>,
) {
    if state.active {
        return;
    }
    for (entity, mut timer) in query {
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            let mut navigator = navigators.get_mut(entity).unwrap();
            // Begin walk animation
            navigator.queue = navigator.path.iter().copied().collect::<Vec<_>>();
            cmd.entity(entity).trigger(StartedWalk);
        }
    }
}

// this is somewhat cursed
pub(crate) fn npc_navigation(
    mut cmd: Commands,
    npcs: Query<(
        Entity,
        &Npc,
        &mut Navigator,
        &mut Transform,
        &GlobalTransform,
    )>,
    targetables: Query<&Targetable>,
    global_transforms: Query<&GlobalTransform>,
    time: Res<Time>,
) {
    const SPEED: f32 = 1.5;
    const CHECKPOINT_RADIUS: f32 = 0.1;

    for (npc_entity, npc, mut nav, mut transform, global_transform) in npcs {
        if let Some(target) = nav.queue.last().copied() {
            // Move to next node
            let target_global_transform = global_transforms.get(target).unwrap();
            let rotated = global_transform
                .compute_transform()
                .looking_at(target_global_transform.translation(), Vec3::Y);
            // lulz slerp moment
            transform.rotation = transform
                .rotation
                .slerp(rotated.rotation, 3.0 * time.delta_secs());
            let forward = rotated.forward();
            transform.translation += forward * time.delta_secs() * SPEED;

            // Transition to the next node if it exists
            if global_transform
                .translation()
                .distance_squared(target_global_transform.translation())
                <= CHECKPOINT_RADIUS.powi(2)
            {
                nav.queue.pop();
                if nav.queue.is_empty() {
                    match (targetables.get(target)).unwrap().targetname.0.as_str()
                        == npc.starting_walk_node.as_ref().unwrap()
                    {
                        true => {
                            cmd.entity(npc_entity).trigger(ArrivedHome);
                        }
                        false => {
                            cmd.entity(npc_entity).trigger(ArrivedAtDestination);
                        }
                    }
                }
            }
        }
    }
}

pub(crate) fn build_nav_paths(
    navs: Query<(&Npc, &mut Navigator), Added<Navigator>>,
    targetables: Query<(Entity, &Targetable), With<WalkTarget>>,
    targets: Query<&Target, With<WalkTarget>>,
) {
    let walk_node_by_name = |name: &String| {
        targetables
            .iter()
            .find(|(_, Targetable { targetname })| &targetname.0 == name)
            .map(|x| x.0)
    };
    for (npc, mut nav) in navs {
        if let Some(ref start_node) = npc.starting_walk_node {
            let mut current = walk_node_by_name(start_node);

            while let Some(node_entity) = current {
                nav.path.push_back(node_entity);

                let target_name = &targets.get(node_entity).unwrap().target.0;

                current = walk_node_by_name(target_name);
            }
        }
    }
}

pub(crate) fn handle_elimination_triggers(
    mut hooks: MessageReader<RatHookTriggered>,
    mut cmd: Commands,
    mut npcs: Query<(&mut Npc, &mut Navigator), Without<DespawnTimer>>,
    mut lure_timers: Query<&mut WalkbackTimer>,
) {
    for event in hooks.read() {
        match event.hook.as_str() {
            "game.lure" => {
                // Immediately end other lure timers
                // idk why this isnt working lulz
                for mut timer in lure_timers.iter_mut() {
                    timer.0.finish();
                }

                let npc_entity = event.target.unwrap();
                if let Ok((_npc, mut navigator)) = npcs.get_mut(npc_entity) {
                    // Begin walk animation
                    navigator.queue = navigator.path.iter().rev().copied().collect::<Vec<_>>();
                    cmd.entity(npc_entity).trigger(StartedWalk);
                }
            }
            "game.kill" => {
                // TODO: play animations.
                let npc_entity = event.target.unwrap();
                // We don't want already despawning NPCs
                if let Ok((mut npc, _)) = npcs.get_mut(npc_entity) {
                    npc.script_id = None;
                    cmd.run_system_cached_with(
                        Npc::transition_to_animation_one_shot,
                        (npc_entity, "death", false),
                    );
                    cmd.entity(npc_entity).insert(DespawnTimer(Timer::new(
                        Duration::from_secs_f32(1.0),
                        TimerMode::Once,
                    )));
                }
            }
            "game.spare" => {
                let npc_entity = event.target.unwrap();
                if let Ok((_npc, mut navigator)) = npcs.get_mut(npc_entity) {
                    // Begin walk animation
                    navigator.queue = navigator.path.iter().copied().collect::<Vec<_>>();
                    cmd.entity(npc_entity).trigger(StartedWalk);
                }
            }
            _ => (),
        }
    }
}

pub(crate) fn handle_despawn_timers(
    query: Query<(Entity, &mut DespawnTimer)>,
    time: Res<Time>,
    mut cmd: Commands,
) {
    for (entity, mut timer) in query {
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            cmd.entity(entity).try_despawn();
        }
    }
}

#[derive(Component)]
pub(crate) struct DespawnTimer(pub(crate) Timer);

#[derive(EntityEvent)]
pub(crate) struct StartedWalk(pub(crate) Entity);

#[derive(EntityEvent)]
pub(crate) struct ArrivedAtDestination(pub(crate) Entity);

#[derive(EntityEvent)]
pub(crate) struct ArrivedHome(pub(crate) Entity);

#[derive(Component)]
pub(crate) struct WalkbackTimer(pub Timer);
