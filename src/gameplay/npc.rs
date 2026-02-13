use std::{iter, time::Duration};

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
    ratspinner::{RatCommand, RatStart},
};

#[point_class(base(Transform, Visibility), model("models/npc_a/npc_a.gltf"))]
#[component(on_add=Self::on_add_hook)]
pub(crate) struct Npc {
    #[class(default = "models/npc_a/npc_a.gltf", must_set)]
    pub(crate) model: String,
    #[class(default = "idle_a")]
    pub(crate) idle_animation: Option<String>,
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
            idle_animation: None,
            suspect: None,
            script_id: None,
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
            .observe(Self::on_use);
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

        // Build the animation graph in a yucky gross way that isn't serialized and is
        // heavily specific :( jam moment!
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
                    transitions.play(&mut animator, *node, Duration::ZERO);
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
