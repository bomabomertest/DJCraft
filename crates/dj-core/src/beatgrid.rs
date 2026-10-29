//! Beatgrid (constant tempo), quantize, and sync helpers.
use crate::bpm::estimate_bpm;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeatGrid {
    pub bpm: f64,
    /// Frame of the first beat.
    pub first_beat: f64,
    pub sample_rate: u32,
}

impl BeatGrid {
    pub fn frames_per_beat(&self) -> f64 { 60.0 / self.bpm * self.sample_rate as f64 }
    /// Position in beats relative to the first beat (may be negative before it).
    pub fn beat_position(&self, frame: f64) -> f64 { (frame - self.first_beat) / self.frames_per_beat() }
    /// Fractional position inside the current beat, in [0, 1).
    pub fn phase(&self, frame: f64) -> f64 { let b = self.beat_position(frame); b - b.floor() }
    pub fn nearest_beat_frame(&self, frame: f64) -> f64 {
        self.first_beat + self.beat_position(frame).round() * self.frames_per_beat()
    }
    /// Snap to the nearest beat (quantize).
    pub fn quantize(&self, frame: f64) -> f64 { self.nearest_beat_frame(frame) }
    /// Shift the whole grid by `frames` (manual beatgrid editing).
    pub fn shifted(&self, frames: f64) -> Self { Self { first_beat: self.first_beat + frames, ..*self } }
    pub fn with_bpm(&self, bpm: f64) -> Self { Self { bpm, ..*self } }
}

/// Estimate tempo and place the grid. The coarse autocorrelation tempo is refined by tracking
/// real transients through the track and fitting a line (beat index vs. time).
pub fn analyze(mono: &[f32], sample_rate: u32) -> Option<BeatGrid> {
    let bpm0 = estimate_bpm(mono, sample_rate, 80.0, 160.0)? as f64;
    let fpb0 = 60.0 / bpm0 * sample_rate as f64;
    let (first, fpb) = locate_beats(mono, fpb0)?;
    Some(BeatGrid { bpm: 60.0 * sample_rate as f64 / fpb, first_beat: first, sample_rate })
}

/// Fine onset resolution (samples per block) for transient localisation.
const FH: usize = 128;

/// Positive energy flux of the first-difference (high-passed) signal, one value per `FH` samples.
/// Sustained tones and bass barely register; transients stand out.
fn fine_flux(mono: &[f32]) -> Vec<f32> {
    let nb = mono.len() / FH;
    let e: Vec<f32> = (0..nb)
        .map(|j| (j * FH..(j + 1) * FH).map(|i| { let d = if i > 0 { mono[i] - mono[i - 1] } else { mono[i] }; d * d }).sum())
        .collect();
    (0..nb).map(|j| if j == 0 { e[0] } else { (e[j] - e[j - 1]).max(0.0) }).collect()
}

/// Strongest transient in the sample range [lo, hi), as a sample position, if above `thresh`.
fn peak_in(flux: &[f32], lo: usize, hi: usize, thresh: f32) -> Option<f64> {
    let (a, b) = (lo / FH, (hi / FH + 1).min(flux.len()));
    if a >= b { return None; }
    let (j, v) = (a..b).map(|j| (j, flux[j])).max_by(|x, y| x.1.partial_cmp(&y.1).unwrap())?;
    if v > thresh { Some((j * FH + FH / 4) as f64) } else { None }
}

/// Returns (first beat frame, refined frames-per-beat). Anchors on the strongest transient in the
/// track, follows beats forwards and backwards (re-estimating the period as it goes), then fits a
/// line through all the beats found.
fn locate_beats(mono: &[f32], fpb0: f64) -> Option<(f64, f64)> {
    let flux = fine_flux(mono);
    let (aj, gmax) = flux.iter().copied().enumerate().max_by(|x, y| x.1.partial_cmp(&y.1).unwrap())?;
    if gmax <= 1e-9 { return None; }
    let anchor = (aj * FH + FH / 4) as f64;
    let thresh = 0.1 * gmax;
    let half = (0.2 * fpb0).min(2048.0);
    let mut fpb = fpb0;
    let mut pts: Vec<(f64, f64)> = vec![(0.0, anchor)];
    for dir in [1.0f64, -1.0] {
        let mut last = (0.0f64, anchor);
        for step in 1..=80 {
            let k = dir * step as f64;
            let pred = last.1 + (k - last.0) * fpb;
            if pred + half < 0.0 || pred - half >= mono.len() as f64 { break; }
            let lo = (pred - half).max(0.0) as usize;
            if let Some(t) = peak_in(&flux, lo, (pred + half).max(0.0) as usize, thresh) {
                pts.push((k, t));
                last = (k, t);
                if pts.len() >= 4 { if let Some((_, slope)) = fit(&pts) { fpb = slope; } }
            }
        }
    }
    let (a, slope) = fit(&pts)?;
    Some((a.rem_euclid(slope), slope))
}

/// Least-squares line t = a + k * slope. Needs at least 2 points.
fn fit(pts: &[(f64, f64)]) -> Option<(f64, f64)> {
    if pts.len() < 2 { return None; }
    let n = pts.len() as f64;
    let (mk, mt) = (pts.iter().map(|p| p.0).sum::<f64>() / n, pts.iter().map(|p| p.1).sum::<f64>() / n);
    let (mut cov, mut var) = (0.0, 0.0);
    for (k, t) in pts { cov += (k - mk) * (t - mt); var += (k - mk) * (k - mk); }
    if var < 1e-9 { return None; }
    let slope = cov / var;
    Some((mt - slope * mk, slope))
}

/// Tempo ratio that makes `slave` play at the same BPM as `master` is currently playing.
pub fn tempo_sync_ratio(master_bpm: f64, master_tempo: f64, slave_bpm: f64) -> f64 {
    master_bpm * master_tempo / slave_bpm
}

/// New slave position that puts it in phase with the master (nearest beat alignment).
pub fn beat_sync_position(slave: &BeatGrid, slave_pos: f64, master: &BeatGrid, master_pos: f64) -> f64 {
    let target_phase = master.phase(master_pos);
    let beat = slave.beat_position(slave_pos).floor();
    let fpb = slave.frames_per_beat();
    let base = slave.first_beat + beat * fpb;
    [base - fpb, base, base + fpb]
        .into_iter()
        .map(|b| b + target_phase * fpb)
        .min_by(|a, b| (a - slave_pos).abs().partial_cmp(&(b - slave_pos).abs()).unwrap())
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clicks(bpm: f32, offset_s: f32, secs: f32, sr: u32) -> Vec<f32> {
        let n = (secs * sr as f32) as usize;
        let mut v = vec![0.0f32; n];
        let period = 60.0 / bpm * sr as f32;
        let mut t = offset_s * sr as f32;
        while (t as usize) + 400 < n {
            for k in 0..400 { v[t as usize + k] += (1.0 - k as f32 / 400.0) * (k as f32 * 0.3).sin(); }
            t += period;
        }
        v
    }

    #[test]
    fn finds_tempo_and_first_beat() {
        let sr = 44100;
        let g = analyze(&clicks(120.0, 0.13, 20.0, sr), sr).unwrap();
        assert!((g.bpm - 120.0).abs() < 1.0, "bpm {}", g.bpm);
        let expect = 0.13 * sr as f64;
        let fpb = g.frames_per_beat();
        let mut d = (g.first_beat - expect).rem_euclid(fpb);
        if d > fpb / 2.0 { d -= fpb; }
        assert!(d.abs() < 0.005 * sr as f64, "first beat off by {d} frames");
    }

    #[test]
    fn refines_fractional_tempo() {
        let sr = 44100;
        let g = analyze(&clicks(123.7, 0.21, 40.0, sr), sr).unwrap();
        assert!((g.bpm - 123.7).abs() < 0.1, "bpm {}", g.bpm);
        let expect = 0.21 * sr as f64;
        let fpb = g.frames_per_beat();
        let mut d = (g.first_beat - expect).rem_euclid(fpb);
        if d > fpb / 2.0 { d -= fpb; }
        assert!(d.abs() < 0.004 * sr as f64, "first beat off by {d} frames");
        // The grid must stay locked deep into the track (no drift).
        let late = g.nearest_beat_frame(35.0 * sr as f64);
        let nominal = (0.21 * sr as f64) + ((late - expect) / (60.0 / 123.7 * sr as f64)).round() * (60.0 / 123.7 * sr as f64);
        assert!((late - nominal).abs() < 0.006 * sr as f64, "drift {}", late - nominal);
    }

    #[test]
    fn quantize_and_phase() {
        let g = BeatGrid { bpm: 60.0, first_beat: 100.0, sample_rate: 1000 };
        assert_eq!(g.quantize(1090.0), 1100.0);
        assert_eq!(g.quantize(1400.0), 1100.0);
        assert!((g.phase(1250.0) - 0.15).abs() < 1e-9);
        assert_eq!(g.shifted(5.0).first_beat, 105.0);
    }

    #[test]
    fn sync_helpers() {
        assert!((tempo_sync_ratio(128.0, 1.0, 120.0) - 128.0 / 120.0).abs() < 1e-12);
        let a = BeatGrid { bpm: 60.0, first_beat: 0.0, sample_rate: 1000 };
        let b = BeatGrid { bpm: 60.0, first_beat: 30.0, sample_rate: 1000 };
        let p = beat_sync_position(&b, 5030.0, &a, 7250.0); // master is 0.25 into a beat
        assert!((b.phase(p) - 0.25).abs() < 1e-9);
        assert!((p - 5030.0).abs() <= 500.0);
    }
}
