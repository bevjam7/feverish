use std::{iter, time::Duration};

use avian3d::prelude::{ColliderConstructor, CollisionLayers, RigidBody};
use bevy::{
    asset::AssetPath,
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
    scene::SceneInstanceReady,
};
use bevy_trenchbroom::prelude::*;

use crate::{
    gameplay::{ColliderHierarchyChildOf, PhysLayer, props::AnimationControls},
    input::Use,
    ratspinner::{RatCommand, RatStart},
};

#[point_class(base(Transform, Visibility), model("models/npc_a/npc_a.gltf"))]
#[component(on_add=Self::on_add_hook)]
pub(crate) struct Npc {
    #[class(default = "models/npc_a/npc_a.gltf", must_set)]
    pub(crate) model: String,
    #[class(default = "idle_a")]
    pub(crate) idle_animation: String,
    /// Marks the NPC as one that must be eliminated or saved
    pub(crate) suspect: Option<SuspectType>,
    pub(crate) script_id: Option<String>,
}

#[derive(Reflect, FgdType)]
pub(crate) enum SuspectType {
    Imposter,
    Human,
}

impl Default for Npc {
    fn default() -> Self {
        Self {
            model: Default::default(),
            idle_animation: "idle_a".into(),
            suspect: None,
            script_id: Some("npc.default".into()),
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

        let clips = ["cower", "sit", "idle_a", "idle_lean", "walk"];
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
        let animations = clips
            .into_iter()
            // TODO: Breaks when we add more than 2 animations to root, so filtering to just the
            // idle animation
            // .filter(|x| x == &npc.idle_animation)
            .map(register_animation)
            .collect();
        let graph_handle = {
            let mut graphs = world.resource_mut::<Assets<AnimationGraph>>();
            graphs.add(graph)
        };

        const HEIGHT: f32 = 1.8;
        let layers = CollisionLayers::new(
            [PhysLayer::Npc, PhysLayer::Usable],
            [PhysLayer::Npc, PhysLayer::Default, PhysLayer::Usable],
        );
        // animations
        //     .get(npc.idle_animation.as_str())
        //     .cloned()
        //     .unwrap(),
        world
            .commands()
            .entity(hook.entity)
            .insert((
                SceneRoot(scene_handle.clone()),
                AnimationControls {
                    animations,
                    graph_handle: graph_handle.clone(),
                },
                AnimationGraphHandle(graph_handle),
                layers,
            ))
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
            .observe(Self::on_use)
            .observe(Self::idle_on_spawn);
    }

    fn idle_on_spawn(
        scene_ready: On<SceneInstanceReady>,
        mut cmd: Commands,
        controls: Query<(&AnimationControls, &Npc)>,
        children: Query<&Children>,
        mut animators: Query<&mut AnimationPlayer>,
    ) {
        let (controls, npc) = controls.get(scene_ready.entity).unwrap();
        let node = *controls
            .animations
            .get(npc.idle_animation.as_str())
            .unwrap();
        for child in
            iter::once(scene_ready.entity).chain(children.iter_descendants(scene_ready.entity))
        {
            if let Ok(mut animator) = animators.get_mut(child) {
                let mut transitions = AnimationTransitions::new();
                transitions.play(&mut animator, node, Duration::ZERO);
                cmd.entity(child).insert(transitions);
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
}
