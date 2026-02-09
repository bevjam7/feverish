use bevy::prelude::*;
use bevy_seedling::prelude::*;

pub(crate) struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, set_up_mixer);
    }
}

fn set_up_mixer(mut cmd: Commands) {
    cmd.spawn((VolumeNode::default(), mixer::MusicBus));
    cmd.spawn((VolumeNode::default(), mixer::UiSfxBus));
    cmd.spawn((VolumeNode::default(), mixer::WorldSfxBus));

    cmd.spawn(SamplerPool(mixer::MusicPool))
        .connect(mixer::MusicBus);
    cmd.spawn(SamplerPool(mixer::UiSfxPool))
        .connect(mixer::UiSfxBus);
    cmd.spawn((
        SamplerPool(mixer::WorldSfxPool),
        sample_effects![(SpatialBasicNode::default(), SpatialScale(Vec3::splat(2.0)))],
    ))
    .connect(mixer::WorldSfxBus);
}

// Various mixer channels for settings tuning

pub(crate) mod mixer {
    use bevy_seedling::prelude::*;

    #[derive(NodeLabel, PartialEq, Eq, Debug, Hash, Clone)]
    pub(crate) struct UiSfxBus;
    #[derive(PoolLabel, PartialEq, Eq, Debug, Hash, Clone)]
    pub(crate) struct UiSfxPool;

    #[derive(NodeLabel, PartialEq, Eq, Debug, Hash, Clone)]
    pub(crate) struct WorldSfxBus;
    #[derive(PoolLabel, PartialEq, Eq, Debug, Hash, Clone)]
    pub(crate) struct WorldSfxPool;

    #[derive(NodeLabel, PartialEq, Eq, Debug, Hash, Clone)]
    pub(crate) struct MusicBus;
    #[derive(PoolLabel, PartialEq, Eq, Debug, Hash, Clone)]
    pub(crate) struct MusicPool;
}
