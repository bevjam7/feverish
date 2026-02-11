use bevy::prelude::*;
use bevy_seedling::prelude::*;

use crate::settings::GameSettings;

pub(crate) struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, set_up_mixer)
            .add_systems(Update, apply_mixer_settings);
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
        sample_effects![(
            SpatialBasicNode::default(),
            HrtfNode::default(),
            SpatialScale(Vec3::splat(1.0))
        )],
    ))
    .connect(mixer::WorldSfxBus);
}

fn apply_mixer_settings(
    settings: Res<GameSettings>,
    mut buses: ParamSet<(
        Query<&mut VolumeNode, With<mixer::MusicBus>>,
        Query<&mut VolumeNode, With<mixer::UiSfxBus>>,
        Query<&mut VolumeNode, With<mixer::WorldSfxBus>>,
    )>,
) {
    let master = settings.master_volume.clamp(0.0, 1.5);
    let music = settings.music_volume.clamp(0.0, 1.5);
    let ui_sfx = settings.ui_sfx_volume.clamp(0.0, 1.5);
    let world_sfx = settings.world_sfx_volume.clamp(0.0, 1.5);

    if let Ok(mut node) = buses.p0().single_mut() {
        node.volume = Volume::Linear(master * music);
    }
    if let Ok(mut node) = buses.p1().single_mut() {
        node.volume = Volume::Linear(master * ui_sfx);
    }
    if let Ok(mut node) = buses.p2().single_mut() {
        node.volume = Volume::Linear(master * world_sfx);
    }
}

// mixer channels that can be controlled in settings.

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
