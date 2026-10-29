//! Audio analysis: musical key detection and waveform overviews with frequency colouring.
use dj_core::deck::Track;
use rustfft::{num_complex::Complex, FftPlanner};

const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    /// 0 = C ... 11 = B
    pub tonic: u8,
    pub minor: bool,
    /// Correlation with the winning profile (higher = more confident).
    pub confidence: f32,
}

impl Key {
    pub fn name(&self) -> String { format!("{} {}", NAMES[self.tonic as usize], if self.minor { "minor" } else { "major" }) }
    /// Camelot-wheel code (e.g. "8A" = A minor, "8B" = C major), the usual DJ key notation.
    pub fn camelot(&self) -> String {
        // Position of each tonic on the wheel, major keys (B) from C; minor (A) is the relative minor.
        const MAJOR: [u8; 12] = [8, 3, 10, 5, 12, 7, 2, 9, 4, 11, 6, 1];
        if self.minor { format!("{}A", MAJOR[((self.tonic + 3) % 12) as usize]) } else { format!("{}B", MAJOR[self.tonic as usize]) }
    }
}

// Krumhansl-Kessler key profiles.
const MAJOR_PROFILE: [f32; 12] = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const MINOR_PROFILE: [f32; 12] = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];

fn corr(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    let (ma, mb) = (a.iter().sum::<f32>() / 12.0, b.iter().sum::<f32>() / 12.0);
    let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
    for i in 0..12 { let (x, y) = (a[i] - ma, b[i] - mb); ab += x * y; aa += x * x; bb += y * y; }
    ab / (aa.sqrt() * bb.sqrt() + 1e-12)
}

/// Average chroma (12 pitch-class energies) over the whole signal.
pub fn chroma(mono: &[f32], sample_rate: u32) -> [f32; 12] {
    const N: usize = 8192;
    const HOP: usize = 4096;
    let mut out = [0.0f32; 12];
    if mono.len() < N { return out; }
    let fft = FftPlanner::<f32>::new().plan_fft_forward(N);
    let win: Vec<f32> = (0..N).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / N as f32).cos()).collect();
    let mut i = 0;
    while i + N <= mono.len() {
        let mut buf: Vec<Complex<f32>> = (0..N).map(|k| Complex::new(mono[i + k] * win[k], 0.0)).collect();
        fft.process(&mut buf);
        for k in 1..N / 2 {
            let f = k as f32 * sample_rate as f32 / N as f32;
            if !(55.0..2000.0).contains(&f) { continue; }
            let midi = 69.0 + 12.0 * (f / 440.0).log2();
            let pc = (midi.round() as i32).rem_euclid(12) as usize;
            out[pc] += buf[k].norm().sqrt(); // compressed magnitude so harmonics don't dominate
        }
        i += HOP;
    }
    out
}

/// Detect the musical key. Returns None for silence / too-short audio.
pub fn detect_key(mono: &[f32], sample_rate: u32) -> Option<Key> {
    let c = chroma(mono, sample_rate);
    if c.iter().all(|&x| x < 1e-6) { return None; }
    let mut best: Option<Key> = None;
    for tonic in 0..12u8 {
        for minor in [false, true] {
            let profile = if minor { &MINOR_PROFILE } else { &MAJOR_PROFILE };
            let mut rotated = [0.0f32; 12];
            for i in 0..12 { rotated[(i + tonic as usize) % 12] = profile[i]; }
            let score = corr(&c, &rotated);
            if best.map_or(true, |b| score > b.confidence) { best = Some(Key { tonic, minor, confidence: score }); }
        }
    }
    best
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WaveBucket {
    pub min: f32,
    pub max: f32,
    /// RMS energy per band, for colouring (bass / mids / highs).
    pub low: f32,
    pub mid: f32,
    pub high: f32,
}

/// Downsample a track to `buckets` summary columns with min/max and 3-band colour energy.
pub fn overview(track: &Track, buckets: usize) -> Vec<WaveBucket> {
    let n = track.frames();
    if n == 0 || buckets == 0 { return Vec::new(); }
    let fs = track.sample_rate as f32;
    let a1 = 1.0 - (-2.0 * std::f32::consts::PI * 200.0 / fs).exp();
    let a2 = 1.0 - (-2.0 * std::f32::consts::PI * 3000.0 / fs).exp();
    let (mut lp1, mut lp2) = (0.0f32, 0.0f32);
    let mut out = Vec::with_capacity(buckets);
    for b in 0..buckets {
        let (s, e) = (b * n / buckets, ((b + 1) * n / buckets).max(b * n / buckets + 1).min(n));
        let (mut mn, mut mx, mut el, mut em, mut eh) = (f32::MAX, f32::MIN, 0.0f32, 0.0f32, 0.0f32);
        for i in s..e {
            let x = 0.5 * (track.left[i] + track.right[i]);
            lp1 += a1 * (x - lp1);
            lp2 += a2 * (x - lp2);
            let (low, mid, high) = (lp1, lp2 - lp1, x - lp2);
            mn = mn.min(x);
            mx = mx.max(x);
            el += low * low;
            em += mid * mid;
            eh += high * high;
        }
        let c = (e - s) as f32;
        out.push(WaveBucket { min: mn, max: mx, low: (el / c).sqrt(), mid: (em / c).sqrt(), high: (eh / c).sqrt() });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    const SR: u32 = 22050;

    fn note(midi: f32) -> f32 { 440.0 * 2f32.powf((midi - 69.0) / 12.0) }
    fn chord(midis: &[f32], secs: f32) -> Vec<f32> {
        let n = (SR as f32 * secs) as usize;
        (0..n).map(|i| {
            let t = i as f32 / SR as f32;
            midis.iter().map(|&m| (2.0 * PI * note(m) * t).sin() + 0.4 * (2.0 * PI * 2.0 * note(m) * t).sin()).sum::<f32>() * 0.2
        }).collect()
    }
    // Quick progression: I - IV - V - I in the key (root midi given), root-heavy.
    fn progression(root: f32, minor: bool) -> Vec<f32> {
        let third = if minor { 3.0 } else { 4.0 };
        let mut v = Vec::new();
        for deg in [0.0, 5.0, 7.0, 0.0] {
            let r = root + deg;
            let t = if deg == 0.0 { third } else if minor { 3.0 } else { 4.0 };
            v.extend(chord(&[r - 12.0, r, r + t, r + 7.0], 2.0));
        }
        v
    }

    #[test]
    fn detects_c_major_and_a_minor() {
        let k = detect_key(&progression(60.0, false), SR).unwrap();
        assert_eq!(k.name(), "C major", "got {} ({})", k.name(), k.confidence);
        let k = detect_key(&progression(57.0, true), SR).unwrap();
        assert_eq!(k.name(), "A minor", "got {} ({})", k.name(), k.confidence);
    }

    #[test]
    fn camelot_codes() {
        assert_eq!(Key { tonic: 0, minor: false, confidence: 1.0 }.camelot(), "8B");
        assert_eq!(Key { tonic: 9, minor: true, confidence: 1.0 }.camelot(), "8A");
        assert_eq!(Key { tonic: 2, minor: true, confidence: 1.0 }.camelot(), "7A"); // D minor
    }

    #[test]
    fn silence_has_no_key() {
        assert!(detect_key(&vec![0.0; SR as usize * 3], SR).is_none());
    }

    #[test]
    fn overview_colours_follow_frequency() {
        let n = SR as usize * 2;
        let mk = |f: f32| -> Track {
            let v: Vec<f32> = (0..n).map(|i| (2.0 * PI * f * i as f32 / SR as f32).sin()).collect();
            Track { sample_rate: SR, left: v.clone(), right: v }
        };
        let bass = overview(&mk(60.0), 50);
        let hi = overview(&mk(8000.0), 50);
        assert_eq!(bass.len(), 50);
        let b = bass[25];
        let h = hi[25];
        assert!(b.low > b.mid && b.low > b.high, "{b:?}");
        assert!(h.high > h.low && h.high > h.mid, "{h:?}");
        assert!(b.max > 0.9 && b.min < -0.9);
    }
}
