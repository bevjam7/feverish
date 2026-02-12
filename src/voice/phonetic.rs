use rand::RngExt;

/// primary language selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    #[allow(dead_code)]
    Portuguese,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhonemeType {
    Vowel,
    Consonant,
    Pause,
    Breath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsonantClass {
    PlosiveVoiced,
    PlosiveUnvoiced,
    FricativeVoiced,
    FricativeUnvoiced,
    Nasal,
    Liquid,
    Tap,
    Trill,
    Lateral,
    Affricate,
}

#[derive(Debug, Clone)]
pub struct Phoneme {
    pub ty: PhonemeType,
    pub formants: Option<[f32; 3]>,        // vowels mainly
    pub consonant: Option<ConsonantClass>, // consonants mainly
    pub duration: f32,                     // multiplier-ish
    pub stressed: bool,
    #[allow(dead_code)]
    pub source_chars: [char; 2], // small & cheap (up to digraph)
    #[allow(dead_code)]
    pub source_len: u8, // 1 or 2
    #[allow(dead_code)]
    pub source_index: usize,
    pub pitch_mod: f32,
}

impl Phoneme {
    fn pause(source_index: usize, dur: f32) -> Self {
        Self {
            ty: PhonemeType::Pause,
            formants: None,
            consonant: None,
            duration: dur,
            stressed: false,
            source_chars: ['\0', '\0'],
            source_len: 0,
            source_index,
            pitch_mod: 1.0,
        }
    }

    fn breath(source_index: usize) -> Self {
        Self {
            ty: PhonemeType::Breath,
            formants: None,
            consonant: None,
            duration: 0.8,
            stressed: false,
            source_chars: ['\0', '\0'],
            source_len: 0,
            source_index,
            pitch_mod: 1.0,
        }
    }
}

#[derive(Default)]
pub struct PhoneticMapper;

impl PhoneticMapper {
    pub fn text_to_phonemes(&mut self, text: &str, lang: Language) -> Vec<Phoneme> {
        match lang {
            Language::English => text_to_phonemes_english(text),
            Language::Portuguese => text_to_phonemes_portuguese(text),
        }
    }
}

// ---------------------------------------------
// english mapper (primary)
// ---------------------------------------------

fn is_sentence_end(ch: char) -> bool {
    matches!(ch, '.' | '!' | '?')
}

fn is_punct_pause(ch: char) -> bool {
    matches!(ch, ',' | ';' | ':')
}

fn is_vowel_basic(ch: char) -> bool {
    matches!(ch, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
}

/// very small english vowel inventory -> (f1,f2,f3) targets.
/// these are intentionally approximate (uncanny is the goal).
fn english_vowel_formants(token: &str) -> Option<[f32; 3]> {
    let t = token;
    Some(match t {
        // tense
        "ee" | "ea" => [300.0, 2600.0, 3400.0], // i:
        "oo" => [350.0, 800.0, 2500.0],         // u:
        "ay" | "ai" => [650.0, 1800.0, 2800.0], // eɪ-ish
        "oy" | "oi" => [500.0, 1100.0, 2600.0], // ɔɪ-ish
        "ow" | "ou" => [500.0, 900.0, 2500.0],  // oʊ-ish
        "er" => [450.0, 1350.0, 2500.0],        // ɝ-ish

        // lax/short
        "a" => [800.0, 1400.0, 2500.0],       // æ/ɑ blend
        "e" => [500.0, 2000.0, 2600.0],       // ɛ-ish
        "i" | "y" => [380.0, 2100.0, 2900.0], // ɪ-ish
        "o" => [550.0, 1000.0, 2500.0],       // ɒ-ish
        "u" => [600.0, 1200.0, 2400.0],       // ʌ-ish

        _ => return None,
    })
}

/// digraph consonants (english).
fn english_digraph_consonant(token: &str) -> Option<ConsonantClass> {
    Some(match token {
        "th" => ConsonantClass::FricativeUnvoiced,
        "sh" => ConsonantClass::FricativeUnvoiced,
        "ch" => ConsonantClass::Affricate,
        "ph" => ConsonantClass::FricativeUnvoiced,
        "ng" => ConsonantClass::Nasal,
        "wh" => ConsonantClass::FricativeUnvoiced,
        _ => return None,
    })
}

fn english_consonant_class(ch: char) -> Option<ConsonantClass> {
    Some(match ch {
        'b' | 'd' | 'g' => ConsonantClass::PlosiveVoiced,
        'p' | 't' | 'k' | 'c' | 'q' => ConsonantClass::PlosiveUnvoiced,

        'f' | 's' | 'x' | 'h' => ConsonantClass::FricativeUnvoiced,
        'v' | 'z' | 'j' => ConsonantClass::FricativeVoiced,

        'm' | 'n' => ConsonantClass::Nasal,
        'l' => ConsonantClass::Lateral,
        'r' => ConsonantClass::Liquid,
        'w' => ConsonantClass::Liquid,

        _ => return None,
    })
}

/// cheap stress heuristic for english:
/// - if word length <= 4 -> stress first vowel
/// - else stress the first non-final vowel nucleus (keeps endings less punchy)
fn english_stress_vowel_index(vowel_positions: &[usize]) -> usize {
    if vowel_positions.is_empty() {
        return 0;
    }
    if vowel_positions.len() == 1 {
        return 0;
    }
    // choose first vowel for short words, else penultimate vowel
    if vowel_positions.last().copied().unwrap_or(0) <= 3 {
        0
    } else {
        vowel_positions.len().saturating_sub(2)
    }
}

fn text_to_phonemes_english(text: &str) -> Vec<Phoneme> {
    let mut out = Vec::with_capacity(text.len().saturating_mul(2));
    let mut rng = rand::rng();

    // split by whitespace but keep punctuation pauses
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    let mut word_index = 0usize;
    let word_count = {
        // rough word count
        let mut in_word = false;
        let mut c = 0;
        for ch in &chars {
            if ch.is_alphabetic() || *ch == '\'' {
                if !in_word {
                    in_word = true;
                    c += 1;
                }
            } else {
                in_word = false;
            }
        }
        c.max(1)
    };

    while i < chars.len() {
        let ch = chars[i];

        // whitespace -> small pause
        if ch.is_whitespace() {
            out.push(Phoneme::pause(i, 0.35));
            i += 1;
            continue;
        }

        // punctuation -> pauses + optional breath
        if is_punct_pause(ch) {
            out.push(Phoneme::pause(i, 0.75));
            i += 1;
            continue;
        }
        if is_sentence_end(ch) {
            out.push(Phoneme::pause(i, 1.25));
            out.push(Phoneme::breath(i));
            i += 1;
            continue;
        }

        // read a word chunk
        if ch.is_alphabetic() || ch == '\'' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphabetic() || chars[i] == '\'') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let lower = word.to_lowercase();

            // prosody curve across sentence-ish: slight rise in questions is handled by '?'
            // already, here we do a gentle declination across words.
            let word_progress = (word_index as f32) / ((word_count - 1).max(1) as f32);
            word_index += 1;

            // find vowel nuclei positions (token indices in scan below)
            let mut vowel_nuclei = Vec::new();
            {
                let wchars: Vec<char> = lower.chars().collect();
                let mut wi = 0usize;
                let mut last_was_vowel = false;
                while wi < wchars.len() {
                    let v = is_vowel_basic(wchars[wi]);
                    if v && !last_was_vowel {
                        vowel_nuclei.push(wi);
                    }
                    last_was_vowel = v;
                    wi += 1;
                }
            }
            let stressed_nucleus = english_stress_vowel_index(&vowel_nuclei);

            // scan word with digraphs
            let wchars: Vec<char> = lower.chars().collect();
            let mut wi = 0usize;
            let mut nucleus_idx = 0usize;

            while wi < wchars.len() {
                // digraph vowel first
                if wi + 1 < wchars.len() {
                    let dig = [wchars[wi], wchars[wi + 1]];
                    let token: String = dig.iter().collect();

                    if let Some(f) = english_vowel_formants(&token) {
                        let stressed = nucleus_idx == stressed_nucleus;
                        let mut pitch_mod = 1.02 - word_progress * 0.10;
                        if stressed {
                            pitch_mod += 0.06;
                        }
                        pitch_mod += rng.random_range(-0.02..0.02);

                        out.push(Phoneme {
                            ty: PhonemeType::Vowel,
                            formants: Some(f),
                            consonant: None,
                            duration: if stressed { 1.25 } else { 0.95 },
                            stressed,
                            source_chars: dig,
                            source_len: 2,
                            source_index: start + wi,
                            pitch_mod,
                        });

                        nucleus_idx += 1;
                        wi += 2;
                        continue;
                    }

                    if let Some(cc) = english_digraph_consonant(&token) {
                        let stressed = nucleus_idx == stressed_nucleus;
                        let mut pitch_mod = 1.02 - word_progress * 0.10;
                        pitch_mod += rng.random_range(-0.02..0.02);

                        out.push(Phoneme {
                            ty: PhonemeType::Consonant,
                            formants: None,
                            consonant: Some(cc),
                            duration: 0.55,
                            stressed,
                            source_chars: dig,
                            source_len: 2,
                            source_index: start + wi,
                            pitch_mod,
                        });
                        wi += 2;
                        continue;
                    }
                }

                // single char vowel
                let c = wchars[wi];
                if let Some(f) = english_vowel_formants(&c.to_string()) {
                    let stressed = nucleus_idx == stressed_nucleus;
                    let mut pitch_mod = 1.02 - word_progress * 0.10;
                    if stressed {
                        pitch_mod += 0.06;
                    }
                    pitch_mod += rng.random_range(-0.02..0.02);

                    out.push(Phoneme {
                        ty: PhonemeType::Vowel,
                        formants: Some(f),
                        consonant: None,
                        duration: if stressed { 1.15 } else { 0.90 },
                        stressed,
                        source_chars: [c, '\0'],
                        source_len: 1,
                        source_index: start + wi,
                        pitch_mod,
                    });

                    nucleus_idx += 1;
                    wi += 1;
                    continue;
                }

                // consonant rules: soft c/g before e/i/y
                let mut cc = english_consonant_class(c);
                if c == 'c' && wi + 1 < wchars.len() {
                    let n = wchars[wi + 1];
                    if matches!(n, 'e' | 'i' | 'y') {
                        cc = Some(ConsonantClass::FricativeUnvoiced); // /s/
                    }
                }
                if c == 'g' && wi + 1 < wchars.len() {
                    let n = wchars[wi + 1];
                    if matches!(n, 'e' | 'i' | 'y') {
                        cc = Some(ConsonantClass::FricativeVoiced); // /ʒ/-ish
                    }
                }

                if let Some(cc) = cc {
                    let stressed = nucleus_idx == stressed_nucleus;
                    let mut pitch_mod = 1.02 - word_progress * 0.10;
                    pitch_mod += rng.random_range(-0.02..0.02);

                    out.push(Phoneme {
                        ty: PhonemeType::Consonant,
                        formants: None,
                        consonant: Some(cc),
                        duration: 0.5,
                        stressed,
                        source_chars: [c, '\0'],
                        source_len: 1,
                        source_index: start + wi,
                        pitch_mod,
                    });
                } else {
                    // unknown char inside word -> tiny pause
                    out.push(Phoneme::pause(start + wi, 0.12));
                }

                wi += 1;
            }

            // word gap
            out.push(Phoneme::pause(i, 0.25));
            continue;
        }

        // any other char -> ignore-ish
        i += 1;
    }

    out
}

// ---------------------------------------------
// portuguese mapper (kept as option)
// ---------------------------------------------

fn pt_is_vowel(ch: char) -> bool {
    matches!(
        ch,
        'a' | 'á'
            | 'â'
            | 'à'
            | 'e'
            | 'é'
            | 'ê'
            | 'i'
            | 'í'
            | 'o'
            | 'ó'
            | 'ô'
            | 'u'
            | 'ú'
            | 'ã'
            | 'õ'
    )
}

fn pt_vowel_formants(ch: char) -> Option<[f32; 3]> {
    Some(match ch {
        'a' => [800.0, 1200.0, 2500.0],
        'á' => [820.0, 1250.0, 2550.0],
        'â' => [600.0, 1150.0, 2500.0],
        'à' => [800.0, 1200.0, 2500.0],
        'e' => [450.0, 1950.0, 2600.0],
        'é' => [550.0, 1900.0, 2700.0],
        'ê' => [400.0, 2100.0, 2700.0],
        'i' => [300.0, 2700.0, 3300.0],
        'í' => [280.0, 2750.0, 3350.0],
        'o' => [500.0, 900.0, 2500.0],
        'ó' => [550.0, 950.0, 2500.0],
        'ô' => [400.0, 800.0, 2500.0],
        'u' => [350.0, 700.0, 2500.0],
        'ú' => [330.0, 680.0, 2450.0],
        'ã' => [700.0, 1100.0, 2500.0],
        'õ' => [450.0, 820.0, 2400.0],
        _ => return None,
    })
}

fn pt_consonant_class(ch: char) -> Option<ConsonantClass> {
    Some(match ch {
        'b' | 'd' | 'g' => ConsonantClass::PlosiveVoiced,
        'p' | 't' | 'k' | 'c' | 'q' => ConsonantClass::PlosiveUnvoiced,
        'f' | 's' | 'x' | 'ç' | 'h' => ConsonantClass::FricativeUnvoiced,
        'v' | 'z' | 'j' => ConsonantClass::FricativeVoiced,
        'm' | 'n' => ConsonantClass::Nasal,
        'l' | 'w' | 'y' => ConsonantClass::Liquid,
        'r' => ConsonantClass::Tap,
        _ => return None,
    })
}

fn pt_digraph(ch1: char, ch2: char) -> Option<ConsonantClass> {
    Some(match (ch1, ch2) {
        ('n', 'h') => ConsonantClass::Nasal,
        ('l', 'h') => ConsonantClass::Lateral,
        ('c', 'h') => ConsonantClass::FricativeUnvoiced,
        ('r', 'r') => ConsonantClass::Trill,
        ('s', 's') => ConsonantClass::FricativeUnvoiced,
        ('q', 'u') => ConsonantClass::PlosiveUnvoiced,
        ('g', 'u') => ConsonantClass::PlosiveVoiced,
        _ => return None,
    })
}

fn pt_find_stressed_syllable(syllables: usize) -> usize {
    if syllables <= 1 {
        0
    } else {
        syllables.saturating_sub(2)
    }
}

fn text_to_phonemes_portuguese(text: &str) -> Vec<Phoneme> {
    let mut out = Vec::with_capacity(text.len().saturating_mul(2));
    let mut rng = rand::rng();

    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;

    // rough sentence splitting by punctuation; we only need question vs statement
    let is_question = text.trim_end().ends_with('?');

    // rough word count for prosody
    let word_count = {
        let mut in_word = false;
        let mut c = 0;
        for ch in &chars {
            if ch.is_alphabetic() {
                if !in_word {
                    in_word = true;
                    c += 1;
                }
            } else {
                in_word = false;
            }
        }
        c.max(1)
    };
    let mut word_idx = 0usize;

    while i < chars.len() {
        let ch = chars[i];

        if ch.is_whitespace() {
            out.push(Phoneme::pause(i, 0.40));
            i += 1;
            continue;
        }

        if matches!(ch, ',' | ';' | ':') {
            out.push(Phoneme::pause(i, 0.8));
            i += 1;
            continue;
        }

        if matches!(ch, '.' | '!' | '?') {
            out.push(Phoneme::pause(i, if ch == '.' { 1.2 } else { 1.5 }));
            out.push(Phoneme::breath(i));
            i += 1;
            continue;
        }

        if ch.is_alphabetic() {
            let start = i;
            while i < chars.len() && chars[i].is_alphabetic() {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let lower: Vec<char> = word.to_lowercase().chars().collect();

            let word_progress = (word_idx as f32) / ((word_count - 1).max(1) as f32);
            word_idx += 1;

            // syllable count ~= vowel clusters
            let mut syllable_count = 0usize;
            let mut last_v = false;
            for &c in &lower {
                let v = pt_is_vowel(c);
                if v && !last_v {
                    syllable_count += 1;
                }
                last_v = v;
            }
            syllable_count = syllable_count.max(1);
            let stressed_syllable = pt_find_stressed_syllable(syllable_count);

            let mut current_syllable = 0usize;
            let mut last_char_was_vowel = false;

            let mut wi = 0usize;
            while wi < lower.len() {
                let c = lower[wi];
                let global_char_idx = start + wi;

                // digraph consonant
                if wi + 1 < lower.len() {
                    if let Some(cc) = pt_digraph(lower[wi], lower[wi + 1]) {
                        let mut pitch_mod = if is_question {
                            1.0 + word_progress * 0.25
                        } else {
                            1.03 - word_progress * 0.12
                        };
                        pitch_mod += rng.random_range(-0.02..0.02);

                        out.push(Phoneme {
                            ty: PhonemeType::Consonant,
                            formants: None,
                            consonant: Some(cc),
                            duration: 0.6,
                            stressed: current_syllable == stressed_syllable,
                            source_chars: [lower[wi], lower[wi + 1]],
                            source_len: 2,
                            source_index: global_char_idx,
                            pitch_mod,
                        });
                        last_char_was_vowel = false;
                        wi += 2;
                        continue;
                    }
                }

                // vowel
                if let Some(f) = pt_vowel_formants(c) {
                    let stressed = current_syllable == stressed_syllable;

                    let mut pitch_mod = if is_question {
                        1.0 + word_progress * 0.25
                    } else {
                        1.03 - word_progress * 0.12
                    };
                    if stressed {
                        pitch_mod += 0.06;
                    }
                    pitch_mod += rng.random_range(-0.02..0.02);

                    out.push(Phoneme {
                        ty: PhonemeType::Vowel,
                        formants: Some(f),
                        consonant: None,
                        duration: if stressed { 1.2 } else { 0.9 },
                        stressed,
                        source_chars: [c, '\0'],
                        source_len: 1,
                        source_index: global_char_idx,
                        pitch_mod,
                    });

                    if !last_char_was_vowel {
                        current_syllable += 1;
                    }
                    last_char_was_vowel = true;
                    wi += 1;
                    continue;
                }

                // consonant with a few context rules
                if let Some(mut cc) = pt_consonant_class(c) {
                    // 'c' before e/i -> /s/
                    if c == 'c' && wi + 1 < lower.len() {
                        let n = lower[wi + 1];
                        if n == 'e' || n == 'i' {
                            cc = ConsonantClass::FricativeUnvoiced;
                        }
                    }
                    // 'r' at start or after consonant -> trill
                    if c == 'r' && (wi == 0 || !pt_is_vowel(lower[wi.saturating_sub(1)])) {
                        cc = ConsonantClass::Trill;
                    }
                    // 's' between vowels -> /z/
                    if c == 's' && wi > 0 && wi + 1 < lower.len() {
                        if pt_is_vowel(lower[wi - 1]) && pt_is_vowel(lower[wi + 1]) {
                            cc = ConsonantClass::FricativeVoiced;
                        }
                    }

                    let mut pitch_mod = if is_question {
                        1.0 + word_progress * 0.25
                    } else {
                        1.03 - word_progress * 0.12
                    };
                    pitch_mod += rng.random_range(-0.02..0.02);

                    out.push(Phoneme {
                        ty: PhonemeType::Consonant,
                        formants: None,
                        consonant: Some(cc),
                        duration: 0.5,
                        stressed: current_syllable == stressed_syllable,
                        source_chars: [c, '\0'],
                        source_len: 1,
                        source_index: global_char_idx,
                        pitch_mod,
                    });

                    last_char_was_vowel = false;
                    wi += 1;
                    continue;
                }

                // unknown -> tiny pause
                out.push(Phoneme::pause(global_char_idx, 0.15));
                last_char_was_vowel = false;
                wi += 1;
            }

            out.push(Phoneme::pause(i, 0.25));
            continue;
        }

        i += 1;
    }

    out
}
