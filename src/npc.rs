use avian3d::prelude::{
    ColliderConstructor, ColliderConstructorHierarchy, CollisionLayers, RigidBody,
};
use bevy::{
    asset::AssetPath,
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
    scene::SceneInstanceReady,
};
use bevy_trenchbroom::prelude::*;

use crate::{assets::GameAssets, gameplay::PhysLayer};

#[point_class(base(Transform, Visibility), model("models/npc_a/npc_a.gltf"))]
#[derive(Default)]
#[component(on_add=Self::on_add_hook)]
pub(crate) struct Npc {
    #[class(default = "models/npc_a/npc_a.gltf", must_set)]
    model: String,
}

impl Npc {
    fn on_add_hook(mut world: DeferredWorld, hook: HookContext) {
        if world.is_scene_world() {
            return;
        }
        let npc = world.get::<Self>(hook.entity).unwrap();
        let asset_path = AssetPath::from(&npc.model);
        let assets = world.resource::<AssetServer>();
        let scene_asset_path = AssetPath::from(asset_path.to_string() + "#Scene0");
        let scene_handle = assets.get_handle(scene_asset_path).unwrap();
        let gltfs = world.resource::<Assets<Gltf>>();
        let gltf_handle = assets.get_handle(asset_path).unwrap();
        let gltf = gltfs.get(&gltf_handle).unwrap();

        let ref animations = gltf.named_animations;

        // Build the animation graph in a yucky gross way that isn't serialized and is
        // heavily specific :( jam moment!
        let mut graph = AnimationGraph::new();
        let idle_walk_blend = graph.add_blend(0.0, graph.root);
        let idle = graph.add_clip(
            animations.get("idle_a").unwrap().clone(),
            1.0,
            idle_walk_blend,
        );
        let walk = graph.add_clip(
            animations.get("walk").unwrap().clone(),
            0.0,
            idle_walk_blend,
        );

        let mut graphs = world.resource_mut::<Assets<AnimationGraph>>();
        let graph_handle = graphs.add(graph);

        world
            .commands()
            .entity(hook.entity)
            .insert((
                ColliderConstructor::Capsule {
                    radius: 0.5,
                    height: 1.8,
                },
                CollisionLayers::new(
                    [PhysLayer::Npc, PhysLayer::Usable],
                    [PhysLayer::Npc, PhysLayer::Default, PhysLayer::Usable],
                ),
                RigidBody::Kinematic,
                SceneRoot(scene_handle.clone()),
                AnimationGraphHandle(graph_handle.clone()),
                NpcAnimationControls {
                    idle_walk_blend,
                    idle,
                    walk,
                    graph_handle,
                },
            ))
            .observe(idle_on_spawn);
    }
}

fn idle_on_spawn(
    scene_ready: On<SceneInstanceReady>,
    controls: Query<&NpcAnimationControls>,
    children: Query<&Children>,
    mut animators: Query<&mut AnimationPlayer>,
    mut cmd: Commands,
) {
    let controls = controls.get(scene_ready.entity).unwrap();
    for child in children.iter_descendants(scene_ready.entity) {
        if let Ok(mut animator) = animators.get_mut(child) {
            // Tell the animation player to start the animation and keep
            // repeating it.
            animator.play(controls.idle).repeat();
            animator.play(controls.walk).repeat();

            // Add the animation graph. This only needs to be done once to
            // connect the animation player to the mesh.
            cmd.entity(child)
                .insert(AnimationGraphHandle(controls.graph_handle.clone()));
        }
    }
}

#[derive(Component)]
pub(crate) struct NpcAnimationControls {
    pub(crate) idle_walk_blend: AnimationNodeIndex,
    pub(crate) idle: AnimationNodeIndex,
    pub(crate) walk: AnimationNodeIndex,
    pub(crate) graph_handle: Handle<AnimationGraph>,
}
