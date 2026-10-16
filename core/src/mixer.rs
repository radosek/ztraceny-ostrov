//! Deterministic software mixer driven by game time (22050 Hz mono). The game owns it and
//! renders exactly as many samples as virtual time advanced, so "is the voice still playing"
//! has the same answer on every run.

use std::sync::Arc;

pub const RATE: u32 = 22050;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Channel {
    Music = 0,
    Fx = 1,
    Speech = 2,
    Ambient = 3,
}

#[derive(Default)]
struct Voice {
    samples: Option<Arc<[i16]>>,
    pos: usize,
    looped: bool,
    paused: bool,
}

pub struct Mixer {
    voices: [Voice; 4],
    /// Per channel 0..=100 (Main.cfg bytes 12..14), music and FX ducked by `duck`.
    pub volume: [u8; 4],
    pub duck: bool,
    rendered: u64,
}

impl Default for Mixer {
    fn default() -> Self {
        Self { voices: Default::default(), volume: [100; 4], duck: false, rendered: 0 }
    }
}

impl Mixer {
    pub fn play(&mut self, ch: Channel, samples: Arc<[i16]>, looped: bool) {
        self.voices[ch as usize] = Voice { samples: Some(samples), pos: 0, looped, paused: false };
    }

    pub fn stop(&mut self, ch: Channel) {
        self.voices[ch as usize] = Voice::default();
    }

    pub fn stop_all(&mut self) {
        for ch in [Channel::Music, Channel::Fx, Channel::Speech, Channel::Ambient] {
            self.stop(ch);
        }
    }

    pub fn set_paused(&mut self, ch: Channel, paused: bool) {
        self.voices[ch as usize].paused = paused;
    }

    pub fn playing(&self, ch: Channel) -> bool {
        let v = &self.voices[ch as usize];
        v.samples.as_ref().is_some_and(|s| v.pos < s.len())
    }

    /// Mix output up to absolute game time `ms`, appending to `out`.
    pub fn render_until(&mut self, ms: u64, out: &mut Vec<i16>) {
        let target = ms * RATE as u64 / 1000;
        let n = target.saturating_sub(self.rendered) as usize;
        self.rendered = target.max(self.rendered);
        let start = out.len();
        out.resize(start + n, 0);
        let mut acc = vec![0i32; n];
        for (c, v) in self.voices.iter_mut().enumerate() {
            let Some(s) = v.samples.clone() else { continue };
            if v.paused {
                continue;
            }
            let mut gain = self.volume[c] as i32;
            if self.duck && matches!(c, 0 | 1 | 3) {
                gain = (gain as f64 * 0.3).round() as i32;
            }
            for a in acc.iter_mut() {
                if v.pos >= s.len() {
                    if v.looped && !s.is_empty() {
                        v.pos = 0;
                    } else {
                        break;
                    }
                }
                *a += s[v.pos] as i32 * gain / 100;
                v.pos += 1;
            }
        }
        for (o, a) in out[start..].iter_mut().zip(acc) {
            *o = a.clamp(-32768, 32767) as i16;
        }
    }
}
