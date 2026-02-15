use avian3d::prelude::*;
use bevy::{
    asset::AssetPath,
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    platform::collections::HashMap,
    prelude::*,
    scene::SceneInstanceReady,
};
use bevy_seedling::sample::SamplePlayer;
use bevy_trenchbroom::prelude::*;

use crate::{
    Usable,
    gameplay::{PhysLayer, link_hierarchal_colliders, npc::Npc},
    input::Use,
};

#[base_class(base(Transform, Visibility), model({path: model}))]
#[derive(Default, Clone)]
#[component(on_add=Self::on_add_hook)]
pub(crate) struct Model {
    pub(crate) model: String,
    pub(crate) animation: Option<String>,
}

impl Model {
    fn on_add_hook(mut world: DeferredWorld, hook: HookContext) {
        if world.is_scene_world() {
            return;
        }
        let model = world.get::<Self>(hook.entity).unwrap().clone();
        let asset_path = AssetPath::from(&model.model);
        let assets = world.resource::<AssetServer>();

        let asset_string = asset_path.to_string();
        let scene_path = if asset_string.contains("#Scene") {
            asset_string
        } else {
            asset_string + "#Scene0"
        };

        let scene_handle = match assets.get_handle(scene_path) {
            Some(handle) => handle,
            None => {
                return;
            }
        };

        let gltfs = world.resource::<Assets<Gltf>>();
        let gltf_handle = match assets.get_handle(asset_path) {
            Some(handle) => handle,
            None => {
                return;
            }
        };

        let Some(gltf) = gltfs.get(&gltf_handle) else {
            return;
        };

        if let Some(animation_name) = model.animation {
            let ref animations = gltf.named_animations;

            // Build the animation graph in a yucky gross way that isn't serialized and is
            // heavily specific :( jam moment!
            let mut graph = AnimationGraph::new();

            let clips = ["idle_a", "idle_lean", "walk", "cower"];
            let register_animation =
                |clip_name: &'static str| -> (&'static str, AnimationNodeIndex) {
                    (
                        clip_name,
                        graph.add_clip(
                            animations
                                .get(clip_name)
                                .expect(&format!("No animation named {clip_name}"))
                                .clone(),
                            match clip_name == &animation_name {
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
            }
            .clone();

            world
                .commands()
                .entity(hook.entity)
                .insert(AnimationControls {
                    animations,
                    graph_handle,
                })
                .observe(Self::start_animating);
        }

        world
            .commands()
            .entity(hook.entity)
            .insert((SceneRoot(scene_handle.clone()),));
    }

    fn start_animating(
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
}

#[point_class(base(Transform, Visibility, Model), model({path: model}))]
#[derive(Default)]
#[component(on_add=Self::on_add_hook)]
pub(crate) struct Prop {
    pub(crate) dynamic: bool,
}

impl Prop {
    pub fn new(dynamic: bool) -> Self {
        Self { dynamic }
    }

    fn on_add_hook(mut world: DeferredWorld, hook: HookContext) {
        if world.is_scene_world() {
            return;
        }
        let is_usable = world.get::<Usable>(hook.entity).is_some();
        let prop = world.get::<Self>(hook.entity).unwrap();
        let rb = match prop.dynamic {
            true => RigidBody::Dynamic,
            false => RigidBody::Static,
        };

        let mut memberships = PhysLayer::Prop.to_bits();
        if is_usable {
            memberships |= PhysLayer::Usable.to_bits();
        }

        let layers = CollisionLayers::new(memberships, PhysLayer::all_bits());
        world
            .commands()
            .entity(hook.entity)
            .insert((
                ColliderConstructorHierarchy::new(ColliderConstructor::ConvexDecompositionFromMesh)
                    .with_default_layers(layers),
                rb,
                layers,
            ))
            .observe(link_hierarchal_colliders);
    }
}

#[point_class(base(Transform, Visibility, Prop, Npc), model({path: model}))]
#[derive(Default)]
#[require(Usable)]
#[component(on_add=Self::on_add_hook)]
pub(crate) struct Phone;

impl Phone {
    fn on_add_hook(mut world: DeferredWorld, hook: HookContext) {
        if world.is_scene_world() {
            return;
        }
        world.commands().entity(hook.entity).observe(Self::on_use);
    }

    fn on_use(
        trigger: On<Use>,
        mut cmd: Commands,
        children: Query<&Children>,
        audio: Query<(), With<SamplePlayer>>,
    ) {
        for child in children.get(trigger.0).unwrap().iter() {
            if audio.contains(child) {
                cmd.entity(child).despawn();
            }
        }
    }
}

// #[base_class(base(Transform, Visibility), model({ path:
// "sprites/audio_emitter.png", scale: 0.125 }))] #[derive(Default)]
// #[component(on_add=Self::on_add_hook)]
// pub(crate) struct Prop {
//     #[class(default = "models/model/model.gltf", must_set)]
//     model: String,
//     dynamic: bool,
// }

#[derive(Component)]
pub(crate) struct AnimationControls {
    pub(crate) animations: HashMap<&'static str, AnimationNodeIndex>,
    pub(crate) graph_handle: Handle<AnimationGraph>,
}
