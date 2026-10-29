//! 8-slot sampler with one-shot / hold / loop play modes and per-slot volume.
use crate::deck::Track;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlayMode {
    /// Plays to the end regardless of release.
    OneShot,
    /// Plays while the pad is held.
    Hold,
    /// Loops until stopped.
    Loop,
}

struct Slot {
    track: Track,
    pos: f64,
    playing: bool,
    mode: PlayMode,
    volume: f32,
}

pub struct Sampler {
    slots: [Option<Slot>; 8],
    output_rate: u32,
    pub master_volume: f32,
}

impl Sampler {
    pub fn new(output_rate: u32) -> Self {
        Self { slots: Default::default(), output_rate, master_volume: 1.0 }
    }
    pub fn load(&mut self, slot: usize, track: Track) {
        if slot < 8 { self.slots[slot] = Some(Slot { track, pos: 0.0, playing: false, mode: PlayMode::OneShot, volume: 1.0 }); }
    }
    pub fn unload(&mut self, slot: usize) { if slot < 8 { self.slots[slot] = None; } }
    pub fn is_loaded(&self, slot: usize) -> bool { self.slots.get(slot).map_or(false, Option::is_some) }
    pub fn set_mode(&mut self, slot: usize, mode: PlayMode) { if let Some(Some(s)) = self.slots.get_mut(slot) { s.mode = mode; } }
    pub fn set_volume(&mut self, slot: usize, v: f32) { if let Some(Some(s)) = self.slots.get_mut(slot) { s.volume = v.max(0.0); } }
    pub fn trigger(&mut self, slot: usize) { if let Some(Some(s)) = self.slots.get_mut(slot) { s.pos = 0.0; s.playing = true; } }
    pub fn release(&mut self, slot: usize) {
        if let Some(Some(s)) = self.slots.get_mut(slot) { if s.mode == PlayMode::Hold { s.playing = false; } }
    }
    pub fn stop(&mut self, slot: usize) { if let Some(Some(s)) = self.slots.get_mut(slot) { s.playing = false; } }
    pub fn is_playing(&self, slot: usize) -> bool { matches!(self.slots.get(slot), Some(Some(s)) if s.playing) }

    /// Add all playing slots into the output buffers.
    pub fn render_add(&mut self, out_l: &mut [f32], out_r: &mut [f32]) {
        let n = out_l.len().min(out_r.len());
        for s in self.slots.iter_mut().flatten() {
            if !s.playing { continue; }
            let step = s.track.sample_rate as f64 / self.output_rate as f64;
            let total = s.track.frames() as f64;
            let gain = s.volume * self.master_volume;
            for i in 0..n {
                if s.pos >= total {
                    if s.mode == PlayMode::Loop { s.pos -= total; } else { s.playing = false; break; }
                }
                let (i0, t) = (s.pos.floor() as usize, (s.pos - s.pos.floor()) as f32);
                let i1 = (i0 + 1).min(s.track.frames() - 1);
                out_l[i] += (s.track.left[i0] * (1.0 - t) + s.track.left[i1] * t) * gain;
                out_r[i] += (s.track.right[i0] * (1.0 - t) + s.track.right[i1] * t) * gain;
                s.pos += step;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dc(n: usize) -> Track { Track { sample_rate: 1000, left: vec![1.0; n], right: vec![1.0; n] } }
    fn render(s: &mut Sampler, n: usize) -> Vec<f32> {
        let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
        s.render_add(&mut l, &mut r);
        l
    }

    #[test]
    fn one_shot_plays_once_then_stops() {
        let mut s = Sampler::new(1000);
        s.load(0, dc(10));
        s.trigger(0);
        let l = render(&mut s, 20);
        assert_eq!(l.iter().filter(|&&x| x > 0.5).count(), 10);
        assert!(!s.is_playing(0));
    }

    #[test]
    fn hold_stops_on_release_loop_wraps() {
        let mut s = Sampler::new(1000);
        s.load(1, dc(100));
        s.set_mode(1, PlayMode::Hold);
        s.trigger(1);
        s.release(1);
        assert!(!s.is_playing(1));
        s.load(2, dc(10));
        s.set_mode(2, PlayMode::Loop);
        s.trigger(2);
        assert!(render(&mut s, 35).iter().all(|&x| x > 0.5));
        assert!(s.is_playing(2));
        s.stop(2);
        assert!(!s.is_playing(2));
    }

    #[test]
    fn volumes_and_mixing() {
        let mut s = Sampler::new(1000);
        s.load(0, dc(10));
        s.load(1, dc(10));
        s.set_volume(1, 0.5);
        s.trigger(0);
        s.trigger(1);
        assert!((render(&mut s, 5)[0] - 1.5).abs() < 1e-6);
    }
}
