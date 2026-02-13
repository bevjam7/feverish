use bevy::prelude::*;
use bevy_seedling::prelude::*;

use crate::settings::GameSettings;

pub(crate) struct AudioPlugin;

#[cfg(feature = "native")]
#[derive(Resource, Debug, Default)]
struct AudioRestartMitigation {
    last_restart_secs: Option<f64>,
    burst_count: u8,
    next_adjustment_after_secs: f64,
}

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, set_up_mixer)
            .add_systems(Update, apply_mixer_settings);

        #[cfg(feature = "native")]
        app.init_resource::<AudioRestartMitigation>()
            .add_observer(adapt_stream_config_on_restart_burst);
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
        sample_effects![(SpatialBasicNode::default(), SpatialScale(Vec3::splat(1.0)))],
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

#[cfg(feature = "native")]
fn adapt_stream_config_on_restart_burst(
    trigger: On<bevy_seedling::context::StreamRestartEvent>,
    time: Res<Time>,
    mut mitigation: ResMut<AudioRestartMitigation>,
    mut stream: ResMut<bevy_seedling::context::AudioStreamConfig>,
) {
    let now = time.elapsed_secs_f64();
    let previous = mitigation.last_restart_secs.replace(now);

    mitigation.burst_count = match previous {
        Some(last) if (now - last) <= 1.0 => mitigation.burst_count.saturating_add(1),
        _ => 1,
    };

    if mitigation.burst_count < 3 || now < mitigation.next_adjustment_after_secs {
        return;
    }

    let current = stream.0.output.desired_block_frames.unwrap_or(1024);
    let next = if current < 2048 {
        2048
    } else if current < 4096 {
        4096
    } else {
        current
    };

    if next == current {
        return;
    }

    stream.0.output.desired_block_frames = Some(next);
    stream.0.output.desired_sample_rate = None;
    stream.0.output.device_id = None;
    stream.0.output.fallback = false;
    mitigation.next_adjustment_after_secs = now + 10.0;

    warn!(
        "audio restart burst detected ({} restarts in <=1s): increasing output buffer from {} to \
         {} frames ({} -> {}).",
        mitigation.burst_count,
        current,
        next,
        trigger.previous_rate.get(),
        trigger.current_rate.get()
    );
}

// mixer channels we tweak from settings

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
