use std::{f32::consts::PI, str::FromStr};

use rand::Rng;

use super::phonetic::{ConsonantClass, Language, Phoneme, PhonemeType};

pub const SAMPLE_RATE: u32 = 44_100;

#[derive(Debug, Clone)]
pub struct VoiceParams {
    pub language: Language,

    pub pitch_hz: f32,
    pub speed: f32,
    pub breathiness: f32,
    pub creepiness: f32,
    pub whisper_mix: f32,
    pub distortion: f32,
    pub reverb_mix: f32,
    pub volume: f32,
}

impl VoiceParams {
    pub fn default_english() -> Self {
        Self {
            language: Language::English,
            pitch_hz: 110.0,
            speed: 1.0,
            breathiness: 0.25,
            creepiness: 0.45,
            whisper_mix: 0.12,
            distortion: 0.10,
            reverb_mix: 0.28,
            volume: 0.75,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum VoicePreset {
    HostileEntity,
    LostChild,
    CorruptedTransmission,
    NeutralNpc,
}

impl VoicePreset {
    pub fn params(self) -> VoiceParams {
        match self {
            VoicePreset::HostileEntity => VoiceParams {
                language: Language::English,
                pitch_hz: 78.0,
                speed: 1.05,
                breathiness: 0.18,
                creepiness: 0.78,
                whisper_mix: 0.16,
                distortion: 0.20,
                reverb_mix: 0.34,
                volume: 0.80,
            },
            VoicePreset::LostChild => VoiceParams {
                language: Language::English,
                pitch_hz: 170.0,
                speed: 1.10,
                breathiness: 0.35,
                creepiness: 0.35,
                whisper_mix: 0.10,
                distortion: 0.06,
                reverb_mix: 0.22,
                volume: 0.70,
            },
            VoicePreset::CorruptedTransmission => VoiceParams {
                language: Language::English,
                pitch_hz: 125.0,
                speed: 1.35,
                breathiness: 0.20,
                creepiness: 0.68,
                whisper_mix: 0.14,
                distortion: 0.14,
                reverb_mix: 0.16,
                volume: 0.70,
            },
            VoicePreset::NeutralNpc => VoiceParams::default_english(),
        }
    }
}

impl FromStr for VoicePreset {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "hostile_entity" | "hostile" => Ok(VoicePreset::HostileEntity),
            "lost_child" | "child" => Ok(VoicePreset::LostChild),
            "corrupted_transmission" | "glitch" => Ok(VoicePreset::CorruptedTransmission),
            "neutral_npc" | "neutral" | "npc" => Ok(VoicePreset::NeutralNpc),
            _ => Ok(VoicePreset::NeutralNpc),
        }
    }
}

// --------- synthesis core ---------

pub struct VoiceSynth {
    params: VoiceParams,
    rng: rand::rngs::ThreadRng,

    // state for continuity
    glottal_phase: f32,
    prev_formants: Option<[f32; 3]>,
    pitch_drift_hz: f32,

    // simple resonators for F1/F2/F3
    f1: Biquad,
    f2: Biquad,
    f3: Biquad,

    // reverb buffer
    rvb: Comb,

    // filtered noise state for smoother aspiration/frication
    noise_fast: f32,
    noise_slow: f32,
}

impl VoiceSynth {
    pub fn new(params: VoiceParams) -> Self {
        Self {
            params,
            rng: rand::rng(),
            glottal_phase: 0.0,
            prev_formants: None,
            pitch_drift_hz: 0.0,
            f1: Biquad::default(),
            f2: Biquad::default(),
            f3: Biquad::default(),
            rvb: Comb::new((SAMPLE_RATE as f32 * 0.085) as usize, 0.72),
            noise_fast: 0.0,
            noise_slow: 0.0,
        }
    }

    fn clear_filters(&mut self) {
        self.f1.reset_state();
        self.f2.reset_state();
        self.f3.reset_state();
    }

    pub fn synthesize(&mut self, phonemes: &[Phoneme]) -> Vec<f32> {
        let mut samples: Vec<f32> = Vec::new();
        const JOIN_FADE_SAMPLES: usize = 192;

        for (idx, p) in phonemes.iter().enumerate() {
            match p.ty {
                PhonemeType::Pause | PhonemeType::Breath => {
                    self.clear_filters();
                    let duration_mod = if matches!(p.ty, PhonemeType::Breath) {
                        0.5
                    } else {
                        1.0
                    };
                    let dur_s = (0.080 * p.duration * duration_mod) / self.params.speed.max(0.05);
                    let mut seg = Vec::new();
                    push_silence(&mut seg, dur_s);
                    append_with_crossfade(&mut samples, &seg, JOIN_FADE_SAMPLES);
                }
                PhonemeType::Vowel => {
                    let dur_s =
                        (if p.stressed { 0.095 } else { 0.070 }) * p.duration / self.params.speed;
                    let next = lookahead_formants(phonemes, idx);
                    let mut seg = self.synth_vowel(p, dur_s, next);
                    apply_segment_edges(&mut seg, 36);
                    append_with_crossfade(&mut samples, &seg, JOIN_FADE_SAMPLES);
                    self.prev_formants = p.formants;
                }
                PhonemeType::Consonant => {
                    self.clear_filters();
                    let dur_s = consonant_duration(
                        p.consonant.unwrap_or(ConsonantClass::FricativeUnvoiced),
                        p.duration,
                    ) / self.params.speed.max(0.05);
                    let next = lookahead_formants(phonemes, idx);
                    let mut seg = self.synth_consonant(p, dur_s, next);
                    apply_segment_edges(&mut seg, 24);
                    append_with_crossfade(&mut samples, &seg, JOIN_FADE_SAMPLES);
                }
            }
        }

        // add a tail so effects do not end abruptly.
        let tail_s = 0.10 + self.params.reverb_mix.clamp(0.0, 1.0) * 0.14;
        push_silence(&mut samples, tail_s);

        // effects: distortion + reverb mix
        if self.params.distortion > 0.02 {
            apply_distortion(&mut samples, self.params.distortion);
        }
        if self.params.reverb_mix > 0.001 {
            apply_reverb(&mut samples, self.params.reverb_mix, &mut self.rvb);
        }

        apply_lowpass(&mut samples, 6800.0);
        apply_highpass(&mut samples, 38.0);
        apply_transient_guard(&mut samples, 0.075);
        remove_dc_offset(&mut samples);
        soft_noise_gate(&mut samples, 0.0015);
        apply_fade_edges(&mut samples, 160, 2400);
        normalize(&mut samples, self.params.volume);
        apply_soft_limiter(&mut samples, 1.05);
        clamp_peak(&mut samples, 0.98);
        samples
    }

    fn synth_vowel(&mut self, p: &Phoneme, dur_s: f32, next: Option<[f32; 3]>) -> Vec<f32> {
        let n = (dur_s * SAMPLE_RATE as f32).max(1.0) as usize;
        let mut out = Vec::with_capacity(n);

        let target = p.formants.unwrap_or([500.0, 1500.0, 2500.0]);

        // coarticulation: blend prev->target, slightly tug toward next
        let prev = self.prev_formants.unwrap_or(target);
        let next = next.unwrap_or(target);

        let creep = self.params.creepiness.clamp(0.0, 1.0);
        let creep_shift = 1.0 + self.rng.random_range(-0.5..0.5) * 0.12 * creep;

        // set resonators (bandpass-ish)
        // Q rises with creepiness (sharper, weirder)
        let q1 = 10.0 + creep * 6.0;
        let q2 = 12.0 + creep * 7.0;
        let q3 = 9.0 + creep * 5.0;

        // we smoothly retune over first quarter
        let ramp_n = (n as f32 * 0.25) as usize;

        for i in 0..n {
            let t = i as f32 / (n as f32);

            let f = [
                lerp(prev[0], target[0], (t * 1.4).min(1.0)) * 0.75
                    + lerp(target[0], next[0], (t * 0.9).min(1.0)) * 0.25,
                lerp(prev[1], target[1], (t * 1.4).min(1.0)) * 0.75
                    + lerp(target[1], next[1], (t * 0.9).min(1.0)) * 0.25,
                lerp(prev[2], target[2], (t * 1.4).min(1.0)) * 0.75
                    + lerp(target[2], next[2], (t * 0.9).min(1.0)) * 0.25,
            ];

            // apply slightly different shift on lower formants for uncanny
            let f1 = f[0] * (creep_shift * (1.0 + 0.04 * creep));
            let f2 = f[1] * (creep_shift * (1.0 - 0.03 * creep));
            let f3 = f[2] * (1.0 + self.rng.random_range(-0.01..0.01) * creep);

            if i < ramp_n {
                let a = i as f32 / ramp_n.max(1) as f32;
                self.f1.set_bandpass(lerp(prev[0], f1, a), q1);
                self.f2.set_bandpass(lerp(prev[1], f2, a), q2);
                self.f3.set_bandpass(lerp(prev[2], f3, a), q3);
            } else {
                self.f1.set_bandpass(f1, q1);
                self.f2.set_bandpass(f2, q2);
                self.f3.set_bandpass(f3, q3);
            }

            // pitch with prosody, vibrato, jitter
            let base_pitch = self.params.pitch_hz * p.pitch_mod;
            let vib_rate = 4.0 + 6.0 * creep;
            let vib_depth = 0.006 + 0.015 * creep;
            let vib = (2.0 * PI * vib_rate * (i as f32 / SAMPLE_RATE as f32)).sin();

            // smooth micro-instability avoids static tone while preventing harsh buzz.
            let target_drift = self.rng.random_range(-1.0..1.0) * (0.25 + 0.95 * creep);
            self.pitch_drift_hz = lerp(self.pitch_drift_hz, target_drift, 0.015);
            let shimmer =
                (2.0 * PI * 11.5 * (i as f32 / SAMPLE_RATE as f32)).sin() * (0.08 + 0.20 * creep);
            let f0 = base_pitch * (1.0 + vib * vib_depth) + self.pitch_drift_hz + shimmer;

            let src = self.rosenberg_glottal(f0.max(40.0));

            // aspiration + whisper layers
            let breath = self.colored_noise(0.24, 0.06) * self.params.breathiness * 0.10;
            let whisper = self.colored_noise(0.30, 0.10)
                * self.params.whisper_mix
                * (0.05 + 0.06 * creep);

            // formant filtering (source-filter)
            let voiced = src + breath;
            let y1 = self.f1.process(voiced) * 0.90;
            let y2 = self.f2.process(voiced) * 0.55;
            let y3 = self.f3.process(voiced) * 0.28;

            // whisper mainly rides F2/F1 a bit
            let wy = self.f2.process(whisper) * 0.55 + self.f1.process(whisper) * 0.25;

            let env = adsr(t, 0.08, 0.0, 1.0, 0.15);
            out.push((y1 + y2 + y3 + wy) * env);
        }

        out
    }

    fn synth_consonant(&mut self, p: &Phoneme, dur_s: f32, next: Option<[f32; 3]>) -> Vec<f32> {
        let n = (dur_s * SAMPLE_RATE as f32).max(1.0) as usize;
        let mut out = Vec::with_capacity(n);
        let class = p.consonant.unwrap_or(ConsonantClass::FricativeUnvoiced);
        let noise_amp = 0.3f32;

        match class {
            ConsonantClass::PlosiveVoiced
            | ConsonantClass::PlosiveUnvoiced
            | ConsonantClass::Affricate => {
                // gap then burst
                let gap_n = (n as f32 * 0.35).max(1.0) as usize;
                for _ in 0..gap_n {
                    out.push(0.0);
                }
                for i in gap_n..n {
                    let t = (i - gap_n) as f32 / (n - gap_n).max(1) as f32;
                    let decay = (-(t * 12.0)).exp();
                    let noise = self.colored_noise(0.40, 0.16) * noise_amp;
                    let voiced = if matches!(class, ConsonantClass::PlosiveVoiced) {
                        self.rosenberg_glottal((self.params.pitch_hz * p.pitch_mod).max(50.0)) * 0.4
                    } else {
                        0.0
                    };
                    out.push((noise + voiced) * decay * 0.5);
                }
            }

            ConsonantClass::FricativeVoiced | ConsonantClass::FricativeUnvoiced => {
                // filtered-ish noise with optional voicing
                let voiced_on = matches!(class, ConsonantClass::FricativeVoiced);
                let base_pitch = (self.params.pitch_hz * p.pitch_mod).max(60.0);

                let form = next
                    .or(self.prev_formants)
                    .unwrap_or([600.0, 1800.0, 2600.0]);
                self.f1.set_bandpass(form[0], 4.0);
                self.f2.set_bandpass(form[1], 3.0);
                self.f3.set_bandpass(form[2], 2.5);

                for i in 0..n {
                    let t = i as f32 / n as f32;
                    let env = adsr(t, 0.15, 0.0, 1.0, 0.20);

                    let noise = self.colored_noise(0.35, 0.12) * noise_amp;
                    let hiss =
                        self.f2.process(noise) * 0.6 + self.f3.process(noise) * 0.3 + noise * 0.1;

                    let voiced = if voiced_on {
                        self.rosenberg_glottal(base_pitch) * 0.25
                    } else {
                        0.0
                    };

                    out.push((hiss + voiced) * env * 0.5);
                }
            }

            ConsonantClass::Nasal => {
                // muffled voiced resonance + notch-ish via low Q
                let base_pitch = (self.params.pitch_hz * p.pitch_mod).max(55.0);
                let form = next
                    .or(self.prev_formants)
                    .unwrap_or([300.0, 1200.0, 2400.0]);
                self.f1.set_bandpass(form[0].min(400.0), 12.0);
                self.f2.set_bandpass(form[1] * 0.70, 9.0);
                self.f3.set_bandpass(form[2] * 0.55, 7.0);

                for i in 0..n {
                    let t = i as f32 / n as f32;
                    let env = adsr(t, 0.12, 0.0, 1.0, 0.18);
                    let src = self.rosenberg_glottal(base_pitch) * 0.55;
                    let buzz = self.f1.process(src) * 0.7 + self.f2.process(src) * 0.35;
                    let air = self.colored_noise(0.22, 0.08) * (0.04 * noise_amp);
                    out.push((buzz + air) * env * 0.32);
                }
            }

            ConsonantClass::Liquid | ConsonantClass::Lateral => {
                // voiced + gentle sweep in F2
                let base_pitch = (self.params.pitch_hz * p.pitch_mod).max(55.0);
                let form = next
                    .or(self.prev_formants)
                    .unwrap_or([450.0, 1500.0, 2500.0]);

                for i in 0..n {
                    let t = i as f32 / n as f32;
                    let env = adsr(t, 0.10, 0.0, 1.0, 0.20);

                    let f2 = lerp(form[1] * 1.25, form[1] * 0.90, t);
                    self.f1.set_bandpass(form[0], 8.0);
                    self.f2.set_bandpass(f2, 7.0);
                    self.f3.set_bandpass(form[2], 6.0);

                    let src = self.rosenberg_glottal(base_pitch);
                    let y = self.f1.process(src) * 0.55
                        + self.f2.process(src) * 0.40
                        + self.f3.process(src) * 0.15;
                    let grit = self.colored_noise(0.28, 0.10) * (0.03 * noise_amp);
                    out.push((y + grit) * env * 0.36);
                }
            }

            ConsonantClass::Tap => {
                // very short blip
                let base_pitch = (self.params.pitch_hz * p.pitch_mod).max(65.0);
                let form = next
                    .or(self.prev_formants)
                    .unwrap_or([500.0, 1700.0, 2600.0]);
                self.f1.set_bandpass(form[0], 10.0);
                self.f2.set_bandpass(form[1], 8.0);
                self.f3.set_bandpass(form[2], 7.0);

                for i in 0..n {
                    let t = i as f32 / n as f32;
                    let env = (-(t * 14.0)).exp(); // fast decay
                    let src = self.rosenberg_glottal(base_pitch) * 0.7;
                    let y = self.f1.process(src) * 0.6 + self.f2.process(src) * 0.25;
                    let click = self.colored_noise(0.52, 0.22) * (0.08 * noise_amp);
                    out.push((y + click) * env * 0.28);
                }
            }

            ConsonantClass::Trill => {
                // amplitude mod around 25hz
                let base_pitch = (self.params.pitch_hz * p.pitch_mod).max(55.0);
                let form = next
                    .or(self.prev_formants)
                    .unwrap_or([450.0, 1400.0, 2400.0]);
                self.f1.set_bandpass(form[0], 8.0);
                self.f2.set_bandpass(form[1], 7.0);
                self.f3.set_bandpass(form[2], 6.0);

                for i in 0..n {
                    let t = i as f32 / n as f32;
                    let env = adsr(t, 0.10, 0.0, 1.0, 0.18);

                    let trill =
                        (2.0 * PI * 25.0 * (i as f32 / SAMPLE_RATE as f32)).sin() * 0.45 + 0.55;
                    let src = self.rosenberg_glottal(base_pitch);
                    let y = self.f1.process(src) * 0.55 + self.f2.process(src) * 0.35;
                    let air = self.colored_noise(0.24, 0.08) * (0.04 * noise_amp);
                    out.push((y + air) * env * trill * 0.34);
                }
            }
        }

        out
    }

    /// rosenberg-ish glottal pulse: cheap but effective for "voicey" source.
    fn rosenberg_glottal(&mut self, f0: f32) -> f32 {
        let sr = SAMPLE_RATE as f32;
        let phase_inc = (f0 / sr).min(0.45);
        self.glottal_phase = (self.glottal_phase + phase_inc) % 1.0;

        let t = self.glottal_phase;

        // open 0..0.4, close 0.4..0.8, rest 0.8..1.0
        let y = if t < 0.4 {
            // rise
            let x = t / 0.4;
            (x * x) * (3.0 - 2.0 * x) // smoothstep
        } else if t < 0.8 {
            // fall
            let x = (t - 0.4) / 0.4;
            1.0 - (x * x) * (3.0 - 2.0 * x)
        } else {
            0.0
        };

        // add a tiny odd harmonic vibe
        let buzz = (2.0 * PI * (t)).sin() * 0.08;
        (y + buzz) * 0.9
    }

    fn colored_noise(&mut self, fast: f32, slow: f32) -> f32 {
        let white = self.rng.random_range(-1.0..1.0);
        self.noise_fast = lerp(self.noise_fast, white, fast.clamp(0.001, 1.0));
        self.noise_slow = lerp(self.noise_slow, white, slow.clamp(0.001, 1.0));
        (self.noise_fast * 0.72 + self.noise_slow * 0.28).clamp(-1.0, 1.0)
    }
}

fn lookahead_formants(phonemes: &[Phoneme], idx: usize) -> Option<[f32; 3]> {
    for j in (idx + 1)..phonemes.len().min(idx + 4) {
        if phonemes[j].ty == PhonemeType::Vowel {
            if let Some(f) = phonemes[j].formants {
                return Some(f);
            }
        }
    }
    None
}

fn consonant_duration(class: ConsonantClass, mult: f32) -> f32 {
    let base = match class {
        ConsonantClass::PlosiveVoiced => 0.020,
        ConsonantClass::PlosiveUnvoiced => 0.022,
        ConsonantClass::Affricate => 0.050,
        ConsonantClass::FricativeVoiced => 0.060,
        ConsonantClass::FricativeUnvoiced => 0.065,
        ConsonantClass::Nasal => 0.055,
        ConsonantClass::Liquid => 0.045,
        ConsonantClass::Lateral => 0.050,
        ConsonantClass::Tap => 0.025,
        ConsonantClass::Trill => 0.070,
    };
    base * mult
}

fn push_silence(dst: &mut Vec<f32>, seconds: f32) {
    let n = (seconds * SAMPLE_RATE as f32).max(1.0) as usize;
    dst.extend(std::iter::repeat_n(0.0, n));
}

pub fn estimate_duration_secs(phonemes: &[Phoneme], params: &VoiceParams) -> f32 {
    let speed = params.speed.max(0.05);
    let mut seconds = 0.0f32;

    for p in phonemes {
        match p.ty {
            PhonemeType::Pause => {
                seconds += (0.080 * p.duration) / speed;
            }
            PhonemeType::Breath => {
                // mean of random term in synth path.
                seconds += 0.14 / speed;
            }
            PhonemeType::Vowel => {
                seconds += (if p.stressed { 0.095 } else { 0.070 }) * p.duration / speed;
            }
            PhonemeType::Consonant => {
                seconds += consonant_duration(
                    p.consonant.unwrap_or(ConsonantClass::FricativeUnvoiced),
                    p.duration,
                ) / speed;
            }
        }
    }

    seconds + (0.10 + params.reverb_mix.clamp(0.0, 1.0) * 0.14)
}

fn append_with_crossfade(dst: &mut Vec<f32>, seg: &[f32], fade_samples: usize) {
    if seg.is_empty() {
        return;
    }
    if dst.is_empty() || fade_samples == 0 {
        dst.extend_from_slice(seg);
        return;
    }

    let overlap = fade_samples.min(dst.len()).min(seg.len());
    let start = dst.len() - overlap;
    for i in 0..overlap {
        let a = i as f32 / overlap.max(1) as f32;
        let fade_out = (1.0 - a).sqrt();
        let fade_in = a.sqrt();
        dst[start + i] = dst[start + i] * fade_out + seg[i] * fade_in;
    }
    dst.extend_from_slice(&seg[overlap..]);
}

fn apply_segment_edges(samples: &mut [f32], edge_samples: usize) {
    if samples.is_empty() || edge_samples == 0 {
        return;
    }

    let edge = edge_samples.min(samples.len() / 2);
    for i in 0..edge {
        let t = i as f32 / edge.max(1) as f32;
        let env = t * t * (3.0 - 2.0 * t);
        samples[i] *= env;
    }

    let start = samples.len() - edge;
    for i in 0..edge {
        let t = (edge - i) as f32 / edge.max(1) as f32;
        let env = t * t * (3.0 - 2.0 * t);
        samples[start + i] *= env;
    }
}

fn normalize(samples: &mut [f32], volume: f32) {
    let mut peak = 0.00001f32;
    for &s in samples.iter() {
        peak = peak.max(s.abs());
    }
    let gain = (volume / peak).min(1.0);
    for s in samples.iter_mut() {
        *s *= gain;
    }
}

fn apply_soft_limiter(samples: &mut [f32], drive: f32) {
    let k = drive.max(1.0);
    for s in samples.iter_mut() {
        *s = (*s * k).tanh() / k.tanh();
    }
}

fn clamp_peak(samples: &mut [f32], max_abs: f32) {
    let lim = max_abs.clamp(0.0, 1.0);
    for s in samples.iter_mut() {
        *s = s.clamp(-lim, lim);
    }
}

fn apply_distortion(samples: &mut [f32], amount: f32) {
    let drive = (1.0 + amount * 12.0).min(30.0);
    for s in samples.iter_mut() {
        let x = (*s * drive).clamp(-3.0, 3.0);
        *s = x.tanh();
    }
}

fn apply_reverb(samples: &mut [f32], mix: f32, comb: &mut Comb) {
    let wet = mix.clamp(0.0, 0.95);
    let dry = 1.0 - wet;
    for s in samples.iter_mut() {
        let r = comb.process(*s);
        *s = *s * dry + r * wet;
    }
}

fn apply_lowpass(samples: &mut [f32], cutoff_hz: f32) {
    let cutoff = cutoff_hz.clamp(400.0, (SAMPLE_RATE as f32) * 0.45);
    let rc = 1.0 / (2.0 * PI * cutoff);
    let dt = 1.0 / SAMPLE_RATE as f32;
    let alpha = dt / (rc + dt);
    let mut y = 0.0f32;
    for s in samples.iter_mut() {
        y += alpha * (*s - y);
        *s = y;
    }
}

fn apply_highpass(samples: &mut [f32], cutoff_hz: f32) {
    let cutoff = cutoff_hz.clamp(10.0, (SAMPLE_RATE as f32) * 0.45);
    let rc = 1.0 / (2.0 * PI * cutoff);
    let dt = 1.0 / SAMPLE_RATE as f32;
    let alpha = rc / (rc + dt);
    let mut y = 0.0f32;
    let mut x1 = 0.0f32;
    for s in samples.iter_mut() {
        let x = *s;
        y = alpha * (y + x - x1);
        x1 = x;
        *s = y;
    }
}

fn apply_transient_guard(samples: &mut [f32], max_delta: f32) {
    let delta_limit = max_delta.clamp(0.005, 1.0);
    let mut prev = 0.0f32;
    for sample in samples.iter_mut() {
        let delta = (*sample - prev).clamp(-delta_limit, delta_limit);
        prev += delta;
        *sample = prev;
    }
}

fn remove_dc_offset(samples: &mut [f32]) {
    let mut x1 = 0.0f32;
    let mut y1 = 0.0f32;
    let r = 0.995f32;
    for s in samples.iter_mut() {
        let y = *s - x1 + r * y1;
        x1 = *s;
        y1 = y;
        *s = y;
    }
}

fn soft_noise_gate(samples: &mut [f32], threshold: f32) {
    let t = threshold.max(1e-6);
    for s in samples.iter_mut() {
        let a = s.abs();
        if a < t {
            *s *= a / t;
        }
    }
}

fn apply_fade_edges(samples: &mut [f32], fade_in_samples: usize, fade_out_samples: usize) {
    if samples.is_empty() {
        return;
    }

    let fade_in_n = fade_in_samples.min(samples.len());
    for (i, s) in samples.iter_mut().take(fade_in_n).enumerate() {
        let x = i as f32 / fade_in_n.max(1) as f32;
        let env = x * x * (3.0 - 2.0 * x);
        *s *= env;
    }

    let fade_out_n = fade_out_samples.min(samples.len());
    let start = samples.len() - fade_out_n;
    for i in 0..fade_out_n {
        let x = (fade_out_n - i) as f32 / fade_out_n.max(1) as f32;
        let env = x * x * (3.0 - 2.0 * x);
        samples[start + i] *= env;
    }
}

/// ADSR-ish envelope with 0 sustain level (we use it like AR).
fn adsr(t: f32, attack: f32, _decay: f32, sustain: f32, release: f32) -> f32 {
    if t < attack {
        let x = (t / attack.max(1e-6)).clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    } else if t > 1.0 - release {
        let x = ((1.0 - t) / release.max(1e-6)).clamp(0.0, 1.0);
        sustain * (x * x * (3.0 - 2.0 * x))
    } else {
        sustain
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

// --------- filters ---------

#[derive(Clone, Copy, Debug)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Default for Biquad {
    fn default() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }
}

impl Biquad {
    fn reset_state(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    fn set_bandpass(&mut self, freq_hz: f32, q: f32) {
        let f = (freq_hz.max(40.0).min((SAMPLE_RATE as f32) * 0.45)) / (SAMPLE_RATE as f32);
        let w0 = 2.0 * PI * f;
        let alpha = (w0.sin()) / (2.0 * q.max(0.001));

        // constant skirt gain, peak gain = Q
        let b0 = alpha;
        let b1 = 0.0;
        let b2 = -alpha;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * w0.cos();
        let a2 = 1.0 - alpha;

        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    fn process(&mut self, x: f32) -> f32 {
        // transposed direct form ii
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

// --------- tiny comb reverb ---------

struct Comb {
    buf: Vec<f32>,
    idx: usize,
    feedback: f32,
}

impl Comb {
    fn new(size: usize, feedback: f32) -> Self {
        Self {
            buf: vec![0.0; size.max(1)],
            idx: 0,
            feedback: feedback.clamp(0.0, 0.98),
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.buf[self.idx];
        self.buf[self.idx] = x + y * self.feedback;
        self.idx += 1;
        if self.idx >= self.buf.len() {
            self.idx = 0;
        }
        y
    }
}
