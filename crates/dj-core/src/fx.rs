//! Simple DJ effects: echo, filter sweep, bit crusher. `param` is a 0..1 macro control.

pub trait Effect {
    fn set_param(&mut self, p: f32);
    fn set_wet(&mut self, wet: f32);
    fn process(&mut self, l: &mut [f32], r: &mut [f32]);
}

/// Feedback delay. `delay_frames` is usually one beat (or a fraction) at the master tempo.
pub struct Echo {
    buf: [Vec<f32>; 2],
    idx: usize,
    pub feedback: f32,
    wet: f32,
}

impl Echo {
    pub fn new(delay_frames: usize) -> Self {
        let d = delay_frames.max(1);
        Self { buf: [vec![0.0; d], vec![0.0; d]], idx: 0, feedback: 0.5, wet: 0.5 }
    }
}

impl Effect for Echo {
    fn set_param(&mut self, p: f32) { self.feedback = p.clamp(0.0, 0.95); }
    fn set_wet(&mut self, wet: f32) { self.wet = wet.clamp(0.0, 1.0); }
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let len = self.buf[0].len();
        let n = l.len();
        for (ch, x) in [l, r].into_iter().enumerate() {
            let mut i = self.idx;
            for s in x.iter_mut() {
                let delayed = self.buf[ch][i];
                self.buf[ch][i] = *s + delayed * self.feedback;
                *s = *s * (1.0 - self.wet * 0.5) + delayed * self.wet;
                i = (i + 1) % len;
            }
        }
        self.idx = (self.idx + n) % len;
    }
}

/// Zero-delay-feedback state-variable filter, stable up to Nyquist.
#[derive(Clone, Copy, Default)]
struct Svf { ic1: f32, ic2: f32 }

impl Svf {
    /// Returns (low, high).
    fn run(&mut self, x: f32, g: f32, k: f32) -> (f32, f32) {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, x - k * v1 - v2)
    }
}

/// DJ filter knob: param 0 = deep low-pass, 0.5 = bypass, 1 = high-pass.
pub struct SweepFilter {
    fs: f32,
    st: [Svf; 2],
    p: f32,
    wet: f32,
}

impl SweepFilter {
    pub fn new(sample_rate: u32) -> Self { Self { fs: sample_rate as f32, st: [Svf::default(); 2], p: 0.5, wet: 1.0 } }
}

impl Effect for SweepFilter {
    fn set_param(&mut self, p: f32) { self.p = p.clamp(0.0, 1.0); }
    fn set_wet(&mut self, wet: f32) { self.wet = wet.clamp(0.0, 1.0); }
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        if (self.p - 0.5).abs() < 0.01 { return; }
        let low_pass = self.p < 0.5;
        let amount = ((self.p - 0.5).abs() * 2.0).clamp(0.0, 1.0); // 0..1 away from centre
        // Exponential sweep: LP 20 kHz -> 80 Hz, HP 20 Hz -> 8 kHz
        let fc = if low_pass { 20000.0 * (80.0f32 / 20000.0).powf(amount) } else { 20.0 * (8000.0f32 / 20.0).powf(amount) };
        let g = (std::f32::consts::PI * fc.min(self.fs * 0.45) / self.fs).tan();
        for (ch, x) in [l, r].into_iter().enumerate() {
            for s in x.iter_mut() {
                let (lo, hi) = self.st[ch].run(*s, g, 1.0 / 0.707);
                let y = if low_pass { lo } else { hi };
                *s = *s * (1.0 - self.wet) + y * self.wet;
            }
        }
    }
}

/// Bit-depth and sample-rate reducer. param 0 = clean, 1 = harsh.
pub struct Crusher {
    p: f32,
    wet: f32,
    hold: [f32; 2],
    count: usize,
}

impl Crusher {
    pub fn new() -> Self { Self { p: 0.5, wet: 1.0, hold: [0.0; 2], count: 0 } }
}

impl Default for Crusher { fn default() -> Self { Self::new() } }

impl Effect for Crusher {
    fn set_param(&mut self, p: f32) { self.p = p.clamp(0.0, 1.0); }
    fn set_wet(&mut self, wet: f32) { self.wet = wet.clamp(0.0, 1.0); }
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let bits = 16.0 - self.p * 13.0; // 16 -> 3 bits
        let levels = 2f32.powf(bits - 1.0);
        let down = 1 + (self.p * 15.0) as usize; // hold every 1..16 samples
        for i in 0..l.len() {
            if self.count % down == 0 {
                self.hold = [(l[i] * levels).round() / levels, (r[i] * levels).round() / levels];
            }
            self.count = self.count.wrapping_add(1);
            l[i] = l[i] * (1.0 - self.wet) + self.hold[0] * self.wet;
            r[i] = r[i] * (1.0 - self.wet) + self.hold[1] * self.wet;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn sine(f: f32, n: usize, fs: f32) -> Vec<f32> { (0..n).map(|i| (2.0 * PI * f * i as f32 / fs).sin()).collect() }
    fn rms(x: &[f32]) -> f32 { (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt() }

    #[test]
    fn echo_repeats_an_impulse() {
        let mut e = Echo::new(100);
        e.set_param(0.5);
        e.set_wet(1.0);
        let (mut l, mut r) = (vec![0.0; 300], vec![0.0; 300]);
        l[0] = 1.0;
        r[0] = 1.0;
        e.process(&mut l, &mut r);
        assert!((l[100] - 1.0).abs() < 1e-6, "first echo {}", l[100]);
        assert!((l[200] - 0.5).abs() < 1e-6, "second echo {}", l[200]);
        assert!(l[50].abs() < 1e-6);
    }

    #[test]
    fn echo_is_block_size_independent() {
        let mut a = Echo::new(37);
        let mut b = Echo::new(37);
        let x: Vec<f32> = (0..200).map(|i| ((i * 7) % 13) as f32 / 13.0).collect();
        let (mut al, mut ar) = (x.clone(), x.clone());
        a.process(&mut al, &mut ar);
        let (mut bl, mut br) = (x.clone(), x.clone());
        for (cl, cr) in bl.chunks_mut(23).zip(br.chunks_mut(23)) { b.process(cl, cr); }
        for i in 0..200 { assert!((al[i] - bl[i]).abs() < 1e-6); }
    }

    #[test]
    fn filter_sweeps() {
        let fs = 44100.0;
        let tone = |f| { let x = sine(f, 44100, fs); (x.clone(), x) };
        let mut lp = SweepFilter::new(44100);
        lp.set_param(0.0);
        let (mut l, mut r) = tone(8000.0);
        lp.process(&mut l, &mut r);
        assert!(rms(&l[22050..]) < 0.02);
        let mut lp = SweepFilter::new(44100);
        lp.set_param(0.0);
        let (mut l, mut r) = tone(40.0);
        lp.process(&mut l, &mut r);
        assert!(rms(&l[22050..]) > 0.5);
        let mut hp = SweepFilter::new(44100);
        hp.set_param(1.0);
        let (mut l, mut r) = tone(60.0);
        hp.process(&mut l, &mut r);
        assert!(rms(&l[22050..]) < 0.02);
        let mut off = SweepFilter::new(44100);
        let (mut l, mut r) = tone(1000.0);
        off.process(&mut l, &mut r);
        assert!((rms(&l) - 0.7071).abs() < 0.01);
    }

    #[test]
    fn crusher_reduces_levels() {
        let mut c = Crusher::new();
        c.set_param(1.0);
        let mut l = sine(440.0, 4410, 44100.0);
        let mut r = l.clone();
        c.process(&mut l, &mut r);
        let mut uniq: Vec<i32> = l.iter().map(|x| (x * 1e5) as i32).collect();
        uniq.sort();
        uniq.dedup();
        assert!(uniq.len() <= 10, "{} distinct levels", uniq.len());
    }
}
