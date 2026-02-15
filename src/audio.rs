use bevy::prelude::*;
use bevy_seedling::prelude::*;

use crate::{Phase, assets::GameAssets, settings::GameSettings};

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
            .add_systems(
                Update,
                (
                    apply_mixer_settings.run_if(resource_changed::<GameSettings>),
                    run_fade_in_out,
                ),
            )
            .add_systems(OnEnter(Phase::Main), start_playing_bgm);

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
        Query<&mut VolumeNode, With<MainBus>>,
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
        node.volume = Volume::Linear(master);
    }
    if let Ok(mut node) = buses.p1().single_mut() {
        node.volume = Volume::Linear(music);
    }
    if let Ok(mut node) = buses.p2().single_mut() {
        node.volume = Volume::Linear(ui_sfx);
    }
    if let Ok(mut node) = buses.p3().single_mut() {
        node.volume = Volume::Linear(world_sfx);
    }
}

fn start_playing_bgm(
    mut cmd: Commands,
    game_assets: Res<GameAssets>,
    music_volume: Single<&mut VolumeNode, With<mixer::MusicBus>>,
) {
    let mut sampler = SamplePlayer::new(game_assets.music_a.clone());
    sampler.repeat_mode = RepeatMode::RepeatEndlessly;
    // TODO: Volume busses not working so we set a very small volume on bgm.
    cmd.spawn((
        sampler.with_volume(Volume::Linear(0.15)),
        // MusicPool,
        // VolumeNode::from_linear(0.1), // FadeInOut::new(0.0, 0.2, 5.0)
    ));
    dbg!(music_volume.volume);
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

#[derive(Component)]
#[require(VolumeNode)]
pub(crate) struct FadeInOut {
    timer: Timer,
    from: f32,
    to: f32,
}

impl FadeInOut {
    fn new(from: f32, to: f32, seconds: f32) -> Self {
        Self {
            timer: Timer::from_seconds(seconds, TimerMode::Once),
            from,
            to,
        }
    }
}

fn run_fade_in_out(
    fades: Query<(Entity, &mut VolumeNode, &mut FadeInOut)>,
    time: Res<Time>,
    mut cmd: Commands,
) {
    for (entity, mut volume, mut fade) in fades {
        volume.set_linear(fade.from.lerp(fade.to, fade.timer.fraction()));
        if fade.timer.is_finished() {
            cmd.entity(entity).remove::<FadeInOut>();
        }
        fade.timer.tick(time.delta());
    }
}
