//! A playback deck: fractional playhead, tempo / nudge / pitch-play, cues, loops, slip, censor.

#[derive(Clone, Debug)]
pub struct Track {
    pub sample_rate: u32,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

impl Track {
    pub fn frames(&self) -> usize {
        self.left.len().min(self.right.len())
    }
    pub fn mono(&self) -> Vec<f32> {
        self.left.iter().zip(&self.right).map(|(l, r)| 0.5 * (l + r)).collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loop {
    pub start: f64,
    pub end: f64,
}

pub const SLOTS: usize = 8;

pub struct Deck {
    track: Option<Track>,
    output_rate: u32,
    pos: f64,
    tempo: f64,
    nudge: f64,
    pitch_semitones: f64,
    reverse: bool,
    playing: bool,
    cues: [Option<f64>; SLOTS],
    saved_loops: [Option<Loop>; SLOTS],
    active_loop: Option<Loop>,
    /// While slip is active this "shadow" playhead keeps advancing as if nothing had happened.
    slip: Option<f64>,
}

#[inline]
fn hermite(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let c0 = y1;
    let c1 = 0.5 * (y2 - y0);
    let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
    ((c3 * t + c2) * t + c1) * t + c0
}

fn sample_at(buf: &[f32], pos: f64) -> f32 {
    if buf.is_empty() || pos < 0.0 {
        return 0.0;
    }
    let i = pos.floor() as isize;
    let t = (pos - pos.floor()) as f32;
    let g = |k: isize| -> f32 { buf[(i + k).clamp(0, buf.len() as isize - 1) as usize] };
    hermite(g(-1), g(0), g(1), g(2), t)
}

impl Deck {
    pub fn new(output_rate: u32) -> Self {
        Self {
            track: None, output_rate, pos: 0.0, tempo: 1.0, nudge: 0.0, pitch_semitones: 0.0,
            reverse: false, playing: false, cues: [None; SLOTS], saved_loops: [None; SLOTS],
            active_loop: None, slip: None,
        }
    }
    pub fn load(&mut self, track: Track) {
        *self = Self { track: Some(track), ..Self::new(self.output_rate) };
    }
    pub fn eject(&mut self) { *self = Self::new(self.output_rate); }
    pub fn play(&mut self) { if self.track.is_some() { self.playing = true; } }
    pub fn pause(&mut self) { self.playing = false; }
    pub fn is_playing(&self) -> bool { self.playing }
    pub fn position_frames(&self) -> f64 { self.pos }
    pub fn seek_frames(&mut self, f: f64) { self.pos = f.max(0.0); }
    pub fn track(&self) -> Option<&Track> { self.track.as_ref() }
    pub fn tempo(&self) -> f64 { self.tempo }
    /// Playback ratio; 1.0 = original speed (pitch follows speed).
    pub fn set_tempo(&mut self, ratio: f64) { self.tempo = ratio.clamp(0.5, 2.0); }
    /// Momentary tempo offset (pitch bend / nudge), e.g. +0.04 = 4% faster. Set 0.0 to release.
    pub fn set_nudge(&mut self, offset: f64) { self.nudge = offset.clamp(-0.5, 0.5); }

    // ---- cues ----
    pub fn set_cue(&mut self, slot: usize) { if slot < SLOTS { self.cues[slot] = Some(self.pos); } }
    pub fn delete_cue(&mut self, slot: usize) { if slot < SLOTS { self.cues[slot] = None; } }
    pub fn jump_cue(&mut self, slot: usize) {
        if let Some(Some(p)) = self.cues.get(slot).copied() { self.pos = p; }
    }
    pub fn cue(&self, slot: usize) -> Option<f64> { self.cues.get(slot).copied().flatten() }
    /// Pitch play: start from a cue at a semitone offset from the original pitch.
    pub fn play_cue_pitched(&mut self, slot: usize, semitones: f64) {
        self.jump_cue(slot);
        self.pitch_semitones = semitones;
        self.play();
    }
    pub fn clear_pitch_play(&mut self) { self.pitch_semitones = 0.0; }

    // ---- loops ----
    fn frames_per_beat(&self, bpm: f64) -> f64 {
        60.0 / bpm * self.track.as_ref().map_or(44100.0, |t| t.sample_rate as f64)
    }
    /// Start a loop of `beats` beats at the current position.
    pub fn loop_beats(&mut self, bpm: f64, beats: f64) {
        if self.track.is_some() {
            let len = self.frames_per_beat(bpm) * beats;
            self.active_loop = Some(Loop { start: self.pos, end: self.pos + len });
        }
    }
    pub fn exit_loop(&mut self) { self.active_loop = None; }
    pub fn active_loop(&self) -> Option<Loop> { self.active_loop }
    pub fn loop_halve(&mut self) {
        if let Some(l) = &mut self.active_loop { l.end = l.start + (l.end - l.start) / 2.0; }
    }
    pub fn loop_double(&mut self) {
        if let Some(l) = &mut self.active_loop { l.end = l.start + (l.end - l.start) * 2.0; }
    }
    pub fn save_loop(&mut self, slot: usize) {
        if slot < SLOTS { self.saved_loops[slot] = self.active_loop; }
    }
    pub fn saved_loop(&self, slot: usize) -> Option<Loop> { self.saved_loops.get(slot).copied().flatten() }
    /// Re-trigger a saved loop: jump to its start and activate it.
    pub fn recall_loop(&mut self, slot: usize) {
        if let Some(l) = self.saved_loop(slot) { self.pos = l.start; self.active_loop = Some(l); }
    }
    pub fn beat_jump(&mut self, bpm: f64, beats: f64) {
        self.pos = (self.pos + self.frames_per_beat(bpm) * beats).max(0.0);
    }

    // ---- slip / roll / censor ----
    pub fn start_slip(&mut self) { if self.slip.is_none() { self.slip = Some(self.pos); } }
    /// End slip: playback resumes where it would have been had nothing interrupted it.
    pub fn end_slip(&mut self) { if let Some(p) = self.slip.take() { self.pos = p; } }
    pub fn loop_roll_start(&mut self, bpm: f64, beats: f64) {
        self.start_slip();
        self.loop_beats(bpm, beats);
    }
    pub fn loop_roll_end(&mut self) {
        self.exit_loop();
        self.end_slip();
    }
    /// Censor: play in reverse while held; on release continue as if it had not been pressed.
    pub fn censor(&mut self, on: bool) {
        if on { self.start_slip(); self.reverse = true; } else { self.reverse = false; self.end_slip(); }
    }

    /// Render `out_l.len()` frames. Silence when paused / unloaded / finished.
    pub fn render(&mut self, out_l: &mut [f32], out_r: &mut [f32]) {
        let n = out_l.len().min(out_r.len());
        let Some(track) = &self.track else {
            out_l[..n].fill(0.0);
            out_r[..n].fill(0.0);
            return;
        };
        let base = self.tempo * track.sample_rate as f64 / self.output_rate as f64;
        let step = base * (1.0 + self.nudge) * 2f64.powf(self.pitch_semitones / 12.0);
        let total = track.frames() as f64;
        for i in 0..n {
            if !self.playing {
                out_l[i] = 0.0;
                out_r[i] = 0.0;
                continue;
            }
            if let Some(lp) = self.active_loop {
                if !self.reverse && self.pos >= lp.end { self.pos = lp.start + (self.pos - lp.end); }
                if self.reverse && self.pos < lp.start { self.pos = lp.end - (lp.start - self.pos); }
            }
            if let Some(s) = &mut self.slip { *s += base; }
            if self.pos >= total || self.pos < 0.0 {
                self.playing = false;
                out_l[i] = 0.0;
                out_r[i] = 0.0;
                continue;
            }
            out_l[i] = sample_at(&track.left, self.pos);
            out_r[i] = sample_at(&track.right, self.pos);
            self.pos += if self.reverse { -step } else { step };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(n: usize) -> Track {
        let v: Vec<f32> = (0..n).map(|i| i as f32).collect();
        Track { sample_rate: 1000, left: v.clone(), right: v }
    }
    fn run(d: &mut Deck, n: usize) -> Vec<f32> {
        let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
        d.render(&mut l, &mut r);
        l
    }
    fn loaded() -> Deck { let mut d = Deck::new(1000); d.load(ramp(2000)); d }

    #[test]
    fn plays_at_unity() {
        let mut d = loaded();
        d.play();
        let l = run(&mut d, 10);
        for i in 0..10 { assert!((l[i] - i as f32).abs() < 1e-4); }
    }

    #[test]
    fn half_tempo_interpolates() {
        let mut d = loaded();
        d.set_tempo(0.5);
        d.seek_frames(10.0);
        d.play();
        let l = run(&mut d, 4);
        for (i, e) in [10.0, 10.5, 11.0, 11.5].iter().enumerate() { assert!((l[i] - e).abs() < 1e-3); }
    }

    #[test]
    fn nudge_speeds_up() {
        let mut d = loaded();
        d.seek_frames(100.0);
        d.set_nudge(0.5);
        d.play();
        run(&mut d, 10);
        assert!((d.position_frames() - 115.0).abs() < 1e-6);
    }

    #[test]
    fn pitch_play_octave_doubles_rate() {
        let mut d = loaded();
        d.seek_frames(50.0);
        d.set_cue(0);
        d.seek_frames(0.0);
        d.play_cue_pitched(0, 12.0);
        run(&mut d, 10);
        assert!((d.position_frames() - 70.0).abs() < 1e-6);
    }

    #[test]
    fn loop_wraps_and_stops_at_end() {
        let mut d = loaded();
        d.seek_frames(10.0);
        d.loop_beats(60.0, 0.01); // 10 frames
        d.play();
        assert!(run(&mut d, 25).iter().all(|&x| (10.0..20.5).contains(&x)));
        d.exit_loop();
        d.seek_frames(1995.0);
        let l = run(&mut d, 10);
        assert!(!d.is_playing());
        assert_eq!(l[9], 0.0);
    }

    #[test]
    fn halve_double_and_saved_loops() {
        let mut d = loaded();
        d.seek_frames(100.0);
        d.loop_beats(60.0, 0.04); // 40 frames
        d.loop_halve();
        assert_eq!(d.active_loop().unwrap().end, 120.0);
        d.loop_double();
        assert_eq!(d.active_loop().unwrap().end, 140.0);
        d.save_loop(3);
        d.exit_loop();
        d.seek_frames(0.0);
        d.recall_loop(3);
        assert_eq!(d.position_frames(), 100.0);
        assert_eq!(d.active_loop(), d.saved_loop(3));
    }

    #[test]
    fn beat_jump_moves_by_beats() {
        let mut d = loaded();
        d.seek_frames(100.0);
        d.beat_jump(60.0, 0.5); // 0.5 beat at 60 BPM, 1 kHz = 500 frames
        assert_eq!(d.position_frames(), 600.0);
        d.beat_jump(60.0, -0.5);
        assert_eq!(d.position_frames(), 100.0);
    }

    #[test]
    fn loop_roll_returns_to_slip_position() {
        let mut d = loaded();
        d.seek_frames(100.0);
        d.play();
        d.loop_roll_start(60.0, 0.01);
        let l = run(&mut d, 50);
        assert!(l.iter().all(|&x| (100.0..110.5).contains(&x)));
        d.loop_roll_end();
        assert!((d.position_frames() - 150.0).abs() < 1e-6);
        assert!(d.active_loop().is_none());
    }

    #[test]
    fn censor_reverses_then_resumes_in_place() {
        let mut d = loaded();
        d.seek_frames(500.0);
        d.play();
        d.censor(true);
        let l = run(&mut d, 10);
        assert!((l[0] - 500.0).abs() < 1e-3 && (l[9] - 491.0).abs() < 1e-3);
        d.censor(false);
        assert!((d.position_frames() - 510.0).abs() < 1e-6);
    }

    #[test]
    fn cues_roundtrip_and_delete() {
        let mut d = loaded();
        d.seek_frames(42.0);
        d.set_cue(2);
        d.seek_frames(0.0);
        d.jump_cue(2);
        assert_eq!(d.position_frames(), 42.0);
        d.delete_cue(2);
        assert_eq!(d.cue(2), None);
    }

    #[test]
    fn eject_resets_deck() {
        let mut d = loaded();
        d.seek_frames(10.0);
        d.set_cue(0);
        d.play();
        d.eject();
        assert!(d.track().is_none() && !d.is_playing());
        assert_eq!(d.cue(0), None);
        assert_eq!(run(&mut d, 4), vec![0.0; 4]);
    }
}
