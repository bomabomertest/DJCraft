//! Three-band DJ EQ built from RBJ biquads (low shelf, peaking mid, high shelf).
use std::f32::consts::PI;

#[derive(Clone, Copy, Default)]
struct Biquad { b0: f32, b1: f32, b2: f32, a1: f32, a2: f32, z1: f32, z2: f32 }

impl Biquad {
    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
    fn set(&mut self, b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) {
        self.b0 = b0 / a0; self.b1 = b1 / a0; self.b2 = b2 / a0;
        self.a1 = a1 / a0; self.a2 = a2 / a0;
    }
    fn low_shelf(&mut self, fs: f32, f0: f32, db: f32) {
        let a = 10f32.powf(db / 40.0);
        let w = 2.0 * PI * f0 / fs;
        let (c, s) = (w.cos(), w.sin());
        let alpha = s / 2.0 * 2f32.sqrt(); // shelf slope S = 1
        let sq = 2.0 * a.sqrt() * alpha;
        self.set(
            a * ((a + 1.0) - (a - 1.0) * c + sq),
            2.0 * a * ((a - 1.0) - (a + 1.0) * c),
            a * ((a + 1.0) - (a - 1.0) * c - sq),
            (a + 1.0) + (a - 1.0) * c + sq,
            -2.0 * ((a - 1.0) + (a + 1.0) * c),
            (a + 1.0) + (a - 1.0) * c - sq,
        );
    }
    fn high_shelf(&mut self, fs: f32, f0: f32, db: f32) {
        let a = 10f32.powf(db / 40.0);
        let w = 2.0 * PI * f0 / fs;
        let (c, s) = (w.cos(), w.sin());
        let alpha = s / 2.0 * 2f32.sqrt();
        let sq = 2.0 * a.sqrt() * alpha;
        self.set(
            a * ((a + 1.0) + (a - 1.0) * c + sq),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * c),
            a * ((a + 1.0) + (a - 1.0) * c - sq),
            (a + 1.0) - (a - 1.0) * c + sq,
            2.0 * ((a - 1.0) - (a + 1.0) * c),
            (a + 1.0) - (a - 1.0) * c - sq,
        );
    }
    fn peaking(&mut self, fs: f32, f0: f32, q: f32, db: f32) {
        let a = 10f32.powf(db / 40.0);
        let w = 2.0 * PI * f0 / fs;
        let (c, s) = (w.cos(), w.sin());
        let alpha = s / (2.0 * q);
        self.set(1.0 + alpha * a, -2.0 * c, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * c, 1.0 - alpha / a);
    }
}

/// Per-channel 3-band EQ. Gains in dB; `KILL_DB` approximates a full kill.
pub const KILL_DB: f32 = -40.0;

pub struct Eq3 {
    fs: f32,
    low: [Biquad; 2],
    mid: [Biquad; 2],
    high: [Biquad; 2],
}

impl Eq3 {
    pub fn new(sample_rate: u32) -> Self {
        let mut e = Self { fs: sample_rate as f32, low: Default::default(), mid: Default::default(), high: Default::default() };
        e.set_gains(0.0, 0.0, 0.0);
        e
    }
    pub fn set_gains(&mut self, low_db: f32, mid_db: f32, high_db: f32) {
        for ch in 0..2 {
            self.low[ch].low_shelf(self.fs, 200.0, low_db);
            self.mid[ch].peaking(self.fs, 1000.0, 0.7, mid_db);
            self.high[ch].high_shelf(self.fs, 4000.0, high_db);
        }
    }
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        for (ch, buf) in [l, r].into_iter().enumerate() {
            for x in buf.iter_mut() {
                *x = self.high[ch].process(self.mid[ch].process(self.low[ch].process(*x)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms_after(eq: &mut Eq3, freq: f32, fs: u32) -> f32 {
        let n = fs as usize;
        let mut l: Vec<f32> = (0..n).map(|i| (2.0 * PI * freq * i as f32 / fs as f32).sin()).collect();
        let mut r = l.clone();
        eq.process(&mut l, &mut r);
        let tail = &l[n / 2..];
        (tail.iter().map(|x| x * x).sum::<f32>() / tail.len() as f32).sqrt()
    }

    #[test]
    fn flat_is_transparent() {
        let mut e = Eq3::new(44100);
        let g = rms_after(&mut e, 1000.0, 44100) / (0.5f32).sqrt();
        assert!((g - 1.0).abs() < 0.02, "{g}");
    }

    #[test]
    fn low_kill_attenuates_bass_not_treble() {
        let mut e = Eq3::new(44100);
        e.set_gains(KILL_DB, 0.0, 0.0);
        let bass = rms_after(&mut e, 60.0, 44100) / (0.5f32).sqrt();
        let mut e2 = Eq3::new(44100);
        e2.set_gains(KILL_DB, 0.0, 0.0);
        let treb = rms_after(&mut e2, 8000.0, 44100) / (0.5f32).sqrt();
        assert!(bass < 0.05, "bass {bass}");
        assert!(treb > 0.9, "treble {treb}");
    }
}
