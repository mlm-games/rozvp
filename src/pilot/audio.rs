//! Pilot audio: `repame-audio` engine plus a synth cue bank.
//! The bevy build ships empty `assets/audio/` and only plumbs volumes, so
//! parity means engine init + channel volumes + cue hooks, not content.
//! Cues are tiny synthesized WAVs (sine blips with decay envelopes) baked
//! at boot; real packs (Kenney) drop in later via `load_cue`.

use repame_audio::{Audio, AudioChannel, CueDef, Variation};

/// UI/game event cues. Names double as bank keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    UiClick,
    Plant,
    Sun,
    Pea,
    Explosion,
    Chomp,
    Win,
    Lose,
}

impl Cue {
    pub fn key(self) -> &'static str {
        match self {
            Cue::UiClick => "ui_click",
            Cue::Plant => "plant",
            Cue::Sun => "sun",
            Cue::Pea => "pea",
            Cue::Explosion => "explosion",
            Cue::Chomp => "chomp",
            Cue::Win => "win",
            Cue::Lose => "lose",
        }
    }
}

pub struct PilotAudio {
    audio: Audio,
}

impl PilotAudio {
    /// Boot the engine. Never fails: without a device this is a silent
    /// no-op backend (`try_init` soft-fails by design).
    pub fn new() -> Self {
        let audio = Audio::try_init().unwrap_or_else(|_| Audio::noop());
        let mut this = Self { audio };
        this.build_bank();
        // Rig `footstep`/`bite` events drive matching cues globally.
        this.audio.map_rig_event_global("footstep", "pea");
        this.audio.map_rig_event_global("bite", "chomp");
        this.audio.map_rig_event("zombie.ren", "footstep", "pea");
        this.audio.map_rig_event("zombie.ren", "bite", "chomp");
        this
    }

    pub fn is_live(&self) -> bool {
        self.audio.is_live()
    }

    pub fn cue(&mut self, cue: Cue) {
        self.audio.play(cue.key());
    }

    pub fn apply_volumes(&mut self, master: f32, sfx: f32, music: f32) {
        self.audio
            .set_channel(AudioChannel::Master, master.max(0.0));
        self.audio.set_channel(AudioChannel::Sfx, sfx.max(0.0));
        self.audio.set_channel(AudioChannel::Music, music.max(0.0));
    }

    /// 0..1 combat intensity (wave progress / huge wave): stems + duck.
    pub fn set_intensity(&mut self, v: f32) {
        self.audio.set_intensity(v.clamp(0.0, 1.0));
    }

    pub fn update(&mut self, dt_secs: f32) {
        self.audio.update(dt_secs);
    }

    fn build_bank(&mut self) {
        // (freq_hz, ms, bus): distinct blips per cue, all synth.
        let defs: &[(Cue, f32, u64, AudioChannel)] = &[
            (Cue::UiClick, 880.0, 60, AudioChannel::Ui),
            (Cue::Plant, 330.0, 140, AudioChannel::Sfx),
            (Cue::Sun, 1320.0, 120, AudioChannel::Sfx),
            (Cue::Pea, 520.0, 70, AudioChannel::Sfx),
            (Cue::Explosion, 110.0, 400, AudioChannel::Sfx),
            (Cue::Chomp, 220.0, 160, AudioChannel::Sfx),
            (Cue::Win, 660.0, 500, AudioChannel::Music),
            (Cue::Lose, 165.0, 600, AudioChannel::Music),
        ];
        for (cue, freq, ms, bus) in defs {
            let wav = synth_blip(*freq, *ms);
            let def = CueDef {
                bus: *bus,
                gain: 0.5,
                cooldown_ms: 30,
                max_voices: 4,
                pitch_wobble: 0.05,
                variation: Variation::RoundRobin,
            };
            let _ = self.audio.load_cue(cue.key(), def, &[wav.as_slice()]);
        }
    }
}

impl Default for PilotAudio {
    fn default() -> Self {
        Self::new()
    }
}

/// Mono 16-bit PCM WAV: sine at `freq_hz` with exponential decay.
/// 22050 Hz keeps cues small (a 600 ms lose sting is ~26 KB).
fn synth_blip(freq_hz: f32, ms: u64) -> Vec<u8> {
    const RATE: u32 = 22_050;
    let n = (RATE as u64 * ms / 1000).max(1) as usize;
    let mut pcm = Vec::with_capacity(n * 2);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let env = (-4.0 * i as f32 / n as f32).exp();
        let s = (t * freq_hz * std::f32::consts::TAU).sin() * env;
        pcm.extend_from_slice(&((s * 12000.0) as i16).to_le_bytes());
    }
    let data_len = pcm.len() as u32;
    let mut wav = Vec::with_capacity(44 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(&pcm);
    wav
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synth_wav_has_valid_header() {
        let wav = synth_blip(440.0, 100);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
        // 100 ms at 22050 Hz, 2 bytes/sample.
        assert_eq!(wav.len(), 44 + 2205 * 2);
    }

    #[test]
    fn cue_keys_are_stable() {
        assert_eq!(Cue::UiClick.key(), "ui_click");
        assert_eq!(Cue::Explosion.key(), "explosion");
    }

    #[test]
    fn bank_builds_without_device() {
        // try_init soft-fails headless; cues must not panic either way.
        let mut pa = PilotAudio::new();
        pa.apply_volumes(0.9, 0.8, 0.7);
        pa.cue(Cue::Plant);
        pa.set_intensity(0.5);
        pa.update(0.016);
    }
}
