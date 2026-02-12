//! procedural formant tts for bevy 0.18

pub mod phonetic;
pub mod synth;

use std::num::NonZeroU32;

use bevy::prelude::*;
use bevy_seedling::{
    prelude::VolumeNode,
    sample::{AudioSample, PlaybackSettings, SamplePlayer},
};
pub use synth::{VoiceParams, VoicePreset};

use crate::{
    audio::mixer::WorldSfxPool,
    settings::GameSettings,
    voice::{phonetic::PhoneticMapper, synth::VoiceSynth},
};

pub struct VoicePlugin;

impl Plugin for VoicePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Speak>()
            .add_message::<StopVoice>()
            .init_resource::<VoiceRuntime>()
            .add_systems(Update, (handle_stop_voice_messages, handle_speak_messages));
    }
}

#[derive(Resource, Default)]
struct VoiceRuntime {
    mapper: PhoneticMapper,
}

#[derive(Message, Debug, Clone)]
pub struct Speak {
    pub text: String,
    pub target: Option<Entity>,
    pub params: VoiceParams,
}

#[derive(Message, Debug, Clone, Copy, Default)]
pub struct StopVoice;

#[derive(Component)]
struct VoicePlayback;

impl Speak {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            target: None,
            params: VoiceParams::default_english(),
        }
    }

    pub fn target(mut self, entity: Entity) -> Self {
        self.target = Some(entity);
        self
    }

    #[allow(dead_code)]
    pub fn params(mut self, params: VoiceParams) -> Self {
        self.params = params;
        self
    }

    pub fn voice(mut self, preset: VoicePreset) -> Self {
        self.params = preset.params();
        self
    }

    #[allow(dead_code)]
    pub fn language(mut self, lang: phonetic::Language) -> Self {
        self.params.language = lang;
        self
    }
}

fn handle_speak_messages(
    mut commands: Commands,
    mut runtime: ResMut<VoiceRuntime>,
    mut messages: MessageReader<Speak>,
    mut sample_assets: ResMut<Assets<AudioSample>>,
    settings: Res<GameSettings>,
    active_voice: Query<Entity, With<VoicePlayback>>,
) {
    for ev in messages.read() {
        let text = ev.text.trim();
        if text.is_empty() {
            continue;
        }

        // keep a single active tts stream to prevent stacking/noise pileup
        for entity in &active_voice {
            commands.entity(entity).despawn();
        }

        // map -> synth -> sample asset
        let runtime_params = ev.params.clone();
        let phonemes = runtime
            .mapper
            .text_to_phonemes(text, runtime_params.language);
        let mut synth_params = runtime_params.clone();
        // keep synthesis headroom, final loudnes its applied in the mixer node
        synth_params.volume = 1.0;
        let mut synth = VoiceSynth::new(synth_params);
        let samples = synth.synthesize(&phonemes);
        let sample_rate =
            NonZeroU32::new(synth::SAMPLE_RATE).expect("SAMPLE_RATE must be non-zero");
        let sample = AudioSample::new(vec![samples], sample_rate);

        let handle = sample_assets.add(sample);

        // spawn playback entity
        let mut e = commands.spawn((
            Name::new("voice_tts"),
            VoicePlayback,
            SamplePlayer::new(handle),
            WorldSfxPool,
            VolumeNode::from_linear(
                (runtime_params.volume * settings.voice_volume.clamp(0.0, 1.5)).clamp(0.0, 2.0),
            ),
            PlaybackSettings::default().despawn(),
        ));

        // if target exists, parent the playback so it follow the npc transform
        if let Some(target) = ev.target {
            e.insert(ChildOf(target));
        }
    }
}

fn handle_stop_voice_messages(
    mut commands: Commands,
    mut messages: MessageReader<StopVoice>,
    active_voice: Query<Entity, With<VoicePlayback>>,
) {
    let mut any = false;
    for _ in messages.read() {
        any = true;
    }
    if !any {
        return;
    }
    for entity in &active_voice {
        commands.entity(entity).despawn();
    }
}

pub fn estimate_speech_duration_secs(text: &str, preset: VoicePreset) -> f32 {
    let params = preset.params();
    let mut mapper = PhoneticMapper::default();
    let phonemes = mapper.text_to_phonemes(text, params.language);
    synth::estimate_duration_secs(&phonemes, &params)
}
