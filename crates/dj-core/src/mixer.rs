//! Two-channel mixer: trim, 3-band EQ, crossfader curves, master level, peak meters and limiter.
use crate::eq::Eq3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    /// Smooth blend, constant perceived loudness.
    ConstantPower,
    /// Plain linear blend.
    Linear,
    /// "Scratch" curve: each side is full volume until the fader is nearly at the opposite end.
    Cut,
}

/// `pos` in [-1, 1]; -1 = deck A only, +1 = deck B only. Returns (gain_a, gain_b).
pub fn crossfade(pos: f32, curve: Curve) -> (f32, f32) {
    let x = (pos.clamp(-1.0, 1.0) + 1.0) * 0.5;
    match curve {
        Curve::Linear => (1.0 - x, x),
        Curve::ConstantPower => {
            let a = x * std::f32::consts::FRAC_PI_2;
            (a.cos(), a.sin())
        }
        Curve::Cut => {
            const EDGE: f32 = 0.05;
            let ga = if x < 1.0 - EDGE { 1.0 } else { (1.0 - x) / EDGE };
            let gb = if x > EDGE { 1.0 } else { x / EDGE };
            (ga, gb)
        }
    }
}

/// Peak limiter with instant attack and exponential release. `warning` is true if it engaged
/// during the last processed block.
pub struct Limiter {
    pub ceiling: f32,
    gain: f32,
    release: f32,
    pub warning: bool,
}

impl Limiter {
    pub fn new(sample_rate: u32) -> Self {
        Self { ceiling: 0.98, gain: 1.0, release: (-1.0 / (0.1 * sample_rate as f32)).exp(), warning: false }
    }
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        self.warning = false;
        for i in 0..l.len() {
            let peak = l[i].abs().max(r[i].abs());
            let need = if peak > self.ceiling { self.ceiling / peak } else { 1.0 };
            if need < self.gain { self.gain = need; } else { self.gain = 1.0 - (1.0 - self.gain) * self.release; }
            if self.gain < 0.999 { self.warning = true; }
            l[i] *= self.gain;
            r[i] *= self.gain;
        }
    }
}

pub struct Channel {
    pub gain: f32,
    pub eq: Eq3,
    /// Peak level of the last processed block (post-EQ, pre-fader).
    pub peak: f32,
}

pub struct Mixer {
    pub a: Channel,
    pub b: Channel,
    pub crossfader: f32,
    pub curve: Curve,
    pub master: f32,
    pub limiter: Limiter,
    /// Master output peak of the last block (post-limiter).
    pub master_peak: f32,
}

fn peak(l: &[f32], r: &[f32]) -> f32 { l.iter().chain(r).fold(0.0f32, |m, x| m.max(x.abs())) }

impl Mixer {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            a: Channel { gain: 1.0, eq: Eq3::new(sample_rate), peak: 0.0 },
            b: Channel { gain: 1.0, eq: Eq3::new(sample_rate), peak: 0.0 },
            crossfader: 0.0,
            curve: Curve::ConstantPower,
            master: 1.0,
            limiter: Limiter::new(sample_rate),
            master_peak: 0.0,
        }
    }

    /// Mix two deck buffers into `out`.
    pub fn mix(&mut self, a: (&mut [f32], &mut [f32]), b: (&mut [f32], &mut [f32]), out: (&mut [f32], &mut [f32])) {
        self.a.eq.process(a.0, a.1);
        self.b.eq.process(b.0, b.1);
        self.a.peak = peak(a.0, a.1);
        self.b.peak = peak(b.0, b.1);
        let (xa, xb) = crossfade(self.crossfader, self.curve);
        let (ga, gb) = (self.a.gain * xa * self.master, self.b.gain * xb * self.master);
        for i in 0..out.0.len() {
            out.0[i] = a.0[i] * ga + b.0[i] * gb;
            out.1[i] = a.1[i] * ga + b.1[i] * gb;
        }
        self.limiter.process(out.0, out.1);
        self.master_peak = peak(out.0, out.1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints() {
        for c in [Curve::Linear, Curve::ConstantPower, Curve::Cut] {
            let (a, b) = crossfade(-1.0, c);
            assert!((a - 1.0).abs() < 1e-6 && b.abs() < 1e-6);
            let (a, b) = crossfade(1.0, c);
            assert!(a.abs() < 1e-6 && (b - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn constant_power_midpoint() {
        let (a, b) = crossfade(0.0, Curve::ConstantPower);
        assert!((a * a + b * b - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cut_keeps_both_open_in_middle() {
        assert_eq!(crossfade(0.0, Curve::Cut), (1.0, 1.0));
    }

    #[test]
    fn limiter_caps_peaks_and_warns() {
        let mut m = Mixer::new(44100);
        m.crossfader = -1.0;
        let (mut al, mut ar) = (vec![1.5f32; 512], vec![1.5f32; 512]);
        let (mut bl, mut br) = (vec![0.0; 512], vec![0.0; 512]);
        let (mut ol, mut or) = (vec![0.0; 512], vec![0.0; 512]);
        m.mix((&mut al, &mut ar), (&mut bl, &mut br), (&mut ol, &mut or));
        assert!(m.master_peak <= 0.981, "peak {}", m.master_peak);
        assert!(m.limiter.warning);
        assert!(m.a.peak > 1.0);
    }

    #[test]
    fn quiet_signal_untouched() {
        let mut m = Mixer::new(44100);
        m.crossfader = -1.0;
        let (mut al, mut ar) = (vec![0.2f32; 256], vec![0.2f32; 256]);
        let (mut bl, mut br) = (vec![0.0; 256], vec![0.0; 256]);
        let (mut ol, mut or) = (vec![0.0; 256], vec![0.0; 256]);
        m.mix((&mut al, &mut ar), (&mut bl, &mut br), (&mut ol, &mut or));
        assert!(!m.limiter.warning);
        assert!((ol[255] - 0.2).abs() < 0.01);
    }
}
