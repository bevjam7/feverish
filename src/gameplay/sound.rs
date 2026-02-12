use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};
use bevy_seedling::{
    prelude::HrtfNode,
    sample::{AudioSample, SamplePlayer},
};
use bevy_trenchbroom::prelude::*;

use crate::{AssetServerExt, audio::mixer::WorldSfxPool};

#[point_class(
    group("sound"),
    classname("point"),
    base(Transform),
    iconsprite({ path: "sprites/audio_emitter.png", scale: 0.125 }),
)]
#[derive(Clone)]
#[component(on_add=Self::on_add_hook)]
pub struct SoundPoint {
    pub(crate) volume: f32,
    #[class(default = "audio/sound.ogg", must_set)]
    pub(crate) sample: String,
    pub(crate) repeat: bool,
    pub(crate) play_immediately: bool,
    pub(crate) repeat_count: Option<usize>,
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
        let sample = assets
            .get_path_handle::<AudioSample>(point.sample.clone())
            .unwrap();

        let volume = point.volume;
        let mut sampler =
            SamplePlayer::new(sample).with_volume(bevy_seedling::prelude::Volume::Linear(volume));

        if point.repeat {
            sampler.repeat_mode = match point.repeat_count {
                Some(count) => bevy_seedling::prelude::RepeatMode::RepeatMultiple {
                    num_times_to_repeat: count as u32,
                },
                None => bevy_seedling::prelude::RepeatMode::RepeatEndlessly,
            };
        }

        let playback_settings = match point.play_immediately {
            false => PlaybackSettings::default().paused(),
            true => PlaybackSettings::default(),
        };

        world.commands().entity(hook.entity).insert((
            sampler,
            playback_settings,
            bevy_seedling::sample_effects![HrtfNode::default()],
            WorldSfxPool,
        ));
    }
}
