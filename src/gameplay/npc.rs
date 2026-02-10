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
    gameplay::{PhysLayer, props::AnimationControls},
    input::Use,
    ratspinner::{RatCommand, RatStart},
};

#[point_class(base(Transform, Visibility), model("models/npc_a/npc_a.gltf"))]
#[component(on_add=Self::on_add_hook)]
pub(crate) struct Npc {
    #[class(default = "models/npc_a/npc_a.gltf", must_set)]
    model: String,
    #[class(default = "idle_a")]
    idle_animation: String,
}

impl Default for Npc {
    fn default() -> Self {
        Self {
            model: Default::default(),
            idle_animation: "idle_a".into(),
        }
    }
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

        let clips = ["idle_a", "idle_lean", "walk", "cower"];
        let register_animation = |clip_name: &'static str| -> (&'static str, AnimationNodeIndex) {
            (
                clip_name,
                graph.add_clip(
                    animations
                        .get(clip_name)
                        .expect(&format!("No animation named {clip_name}"))
                        .clone(),
                    match clip_name == &npc.idle_animation {
                        true => 1.0,
                        false => 0.0,
                    },
                    graph.root,
                ),
            )
        };
        let animations = clips.into_iter().map(register_animation).collect();
        let graph_handle = {
            let mut graphs = world.resource_mut::<Assets<AnimationGraph>>();
            graphs.add(graph)
        };
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
                AnimationControls {
                    animations,
                    graph_handle,
                },
                NpcDialogueProfile::default(),
            ))
            .observe(idle_on_spawn)
            .observe(npc_on_use);
    }
}

fn idle_on_spawn(
    scene_ready: On<SceneInstanceReady>,
    controls: Query<&AnimationControls>,
    children: Query<&Children>,
    mut animators: Query<&mut AnimationPlayer>,
    mut cmd: Commands,
) {
    let controls = controls.get(scene_ready.entity).unwrap();
    for child in children.iter_descendants(scene_ready.entity) {
        if let Ok(mut animator) = animators.get_mut(child) {
            // Tell the animation player to start the animation and keep
            // repeating it.
            for animation in controls.animations.iter() {
                animator.play(animation.1.clone()).repeat();
            }

            // Add the animation graph. This only needs to be done once to
            // connect the animation player to the mesh.
            cmd.entity(child)
                .insert(AnimationGraphHandle(controls.graph_handle.clone()));
        }
    }
}

#[derive(Component, Clone)]
struct NpcDialogueProfile {
    script_id: String,
}

impl Default for NpcDialogueProfile {
    fn default() -> Self {
        Self {
            script_id: "npc.default".to_string(),
        }
    }
}

fn npc_on_use(trigger: On<Use>, mut commands: Commands, npcs: Query<&NpcDialogueProfile>) {
    let Ok(dialogue) = npcs.get(trigger.0) else {
        return;
    };
    commands.write_message(RatCommand::Start(
        RatStart::new(dialogue.script_id.clone()).target(trigger.0),
    ));
}
