use bevy::{
    asset::AssetPath,
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};
use bevy_seedling::{
    prelude::HrtfNode,
    sample::{AudioSample, SamplePlayer},
};
use bevy_trenchbroom::prelude::*;

use crate::audio::mixer::WorldSfxPool;

#[point_class(
    group("sound"),
    classname("point"),
    base(Transform),
    iconsprite({ path: "sprites/audio_emitter.png", scale: 0.125 }),
)]
#[derive(Clone)]
#[component(on_add=Self::on_add_hook)]
pub struct SoundPoint {
    volume: f32,
    #[class(default = "audio/sound.ogg", must_set)]
    sample: String,
    repeat: bool,
    play_immediately: bool,
    repeat_count: Option<usize>,
}

impl Default for SoundPoint {
    fn default() -> Self {
        Self {
            volume: 1.0,
            sample: Default::default(),
            repeat: true,
            play_immediately: true,
            repeat_count: None,
        }
    }
}

impl SoundPoint {
    fn on_add_hook(mut world: DeferredWorld, hook: HookContext) {
        if world.is_scene_world() {
            return;
        }
        let point = world.get::<Self>(hook.entity).unwrap();
        let assets = world.resource::<AssetServer>();

        // let samples = world.resource::<Assets<AudioSample>>();

        // let (sample_id, _) = samples
        // .iter()
        // .find(|(x, _)| assets.get_path(*x).unwrap() ==
        // AssetPath::from(point.sample.clone())) .expect(&format!("Could not
        // load sample {}", point.sample)); let sample =
        // assets.get_id_handle(sample_id).unwrap();

        // so i kind of had to edit this becaaaause the last version would crash after
        // not finsing AudioSample in assets
        let sample: Handle<AudioSample> = assets.load(AssetPath::from(point.sample.clone()));

        let volume = point.volume;
        let mut sampler =
            SamplePlayer::new(sample).with_volume(bevy_seedling::prelude::Volume::Linear(volume));
        // TODO: make this work lol
        if point.play_immediately {}
        if point.repeat {
            sampler.repeat_mode = match point.repeat_count {
                Some(count) => bevy_seedling::prelude::RepeatMode::RepeatMultiple {
                    num_times_to_repeat: count as u32,
                },
                None => bevy_seedling::prelude::RepeatMode::RepeatEndlessly,
            };
        }

        world.commands().entity(hook.entity).insert((
            sampler,
            bevy_seedling::sample_effects![HrtfNode::default()],
            WorldSfxPool,
        ));
    }
}
