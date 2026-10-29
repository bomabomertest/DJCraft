//! Stem separation: split a track into drums / bass / vocals / other.
//!
//! `StemSeparator` is the seam. `SpectralSeparator` is a dependency-light, non-ML baseline
//! (median-filter harmonic/percussive masks + frequency bands + stereo centre). For
//! production quality, implement the trait over an open neural model (e.g. Demucs via ONNX).
use dj_core::deck::{Deck, Track};
use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::Arc;

pub struct Stems {
    pub drums: Track,
    pub bass: Track,
    pub vocals: Track,
    pub other: Track,
}

pub const STEM_NAMES: [&str; 4] = ["drums", "bass", "vocals", "other"];

pub trait StemSeparator {
    fn separate(&self, track: &Track) -> Result<Stems, String>;
}

const N: usize = 2048;
const HOP: usize = 512;

fn hann() -> Vec<f32> {
    (0..N).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / N as f32).cos()).collect()
}

type Spec = Vec<Vec<Complex<f32>>>; // frames x (N/2+1)

fn stft(x: &[f32], win: &[f32], fft: &Arc<dyn rustfft::Fft<f32>>) -> Spec {
    let padded: Vec<f32> = std::iter::repeat(0.0).take(N).chain(x.iter().copied()).chain(std::iter::repeat(0.0).take(N)).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + N <= padded.len() {
        let mut buf: Vec<Complex<f32>> = (0..N).map(|k| Complex::new(padded[i + k] * win[k], 0.0)).collect();
        fft.process(&mut buf);
        out.push(buf[..=N / 2].to_vec());
        i += HOP;
    }
    out
}

fn istft(spec: &Spec, len: usize, win: &[f32], ifft: &Arc<dyn rustfft::Fft<f32>>) -> Vec<f32> {
    let total = N + len + N;
    let mut acc = vec![0.0f32; total + N];
    let mut norm = vec![0.0f32; total + N];
    for (f, frame) in spec.iter().enumerate() {
        let mut buf = vec![Complex::new(0.0, 0.0); N];
        for k in 0..=N / 2 { buf[k] = frame[k]; }
        for k in 1..N / 2 { buf[N - k] = frame[k].conj(); }
        ifft.process(&mut buf);
        let start = f * HOP;
        for k in 0..N {
            acc[start + k] += buf[k].re / N as f32 * win[k];
            norm[start + k] += win[k] * win[k];
        }
    }
    (0..len).map(|i| { let n = norm[N + i]; if n > 1e-6 { acc[N + i] / n } else { 0.0 } }).collect()
}

fn median(v: &mut [f32]) -> f32 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

pub struct SpectralSeparator {
    /// Median filter half-width (frames over time, bins over frequency).
    pub kernel: usize,
}

impl Default for SpectralSeparator {
    fn default() -> Self { Self { kernel: 8 } }
}

impl StemSeparator for SpectralSeparator {
    fn separate(&self, t: &Track) -> Result<Stems, String> {
        let len = t.frames();
        if len < N { return Err("track too short".into()); }
        let sr = t.sample_rate as f32;
        let win = hann();
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(N);
        let ifft = planner.plan_fft_inverse(N);
        let (sl, sr_) = (stft(&t.left[..len], &win, &fft), stft(&t.right[..len], &win, &fft));
        let frames = sl.len();
        let bins = N / 2 + 1;

        // Mid-channel magnitude drives the harmonic/percussive decision.
        let mag: Vec<Vec<f32>> = (0..frames).map(|f| (0..bins).map(|k| (0.5 * (sl[f][k] + sr_[f][k])).norm()).collect()).collect();
        let kw = self.kernel;
        let mut harm = vec![vec![0.0f32; bins]; frames];
        let mut perc = vec![vec![0.0f32; bins]; frames];
        let mut tmp = Vec::new();
        for k in 0..bins {
            for f in 0..frames {
                tmp.clear();
                tmp.extend((f.saturating_sub(kw)..(f + kw + 1).min(frames)).map(|g| mag[g][k]));
                harm[f][k] = median(&mut tmp);
            }
        }
        for f in 0..frames {
            for k in 0..bins {
                tmp.clear();
                tmp.extend((k.saturating_sub(kw)..(k + kw + 1).min(bins)).map(|j| mag[f][j]));
                perc[f][k] = median(&mut tmp);
            }
        }

        let hz = |k: usize| k as f32 * sr / N as f32;
        let low_w = |k: usize| ((250.0 - hz(k)) / 100.0).clamp(0.0, 1.0); // 1 below 150 Hz, 0 above 250 Hz
        let voc_band = |k: usize| if hz(k) < 8000.0 { 1.0 } else { 0.0 };

        let mut stem_spec: [(Spec, Spec); 3] = std::array::from_fn(|_| (Vec::with_capacity(frames), Vec::with_capacity(frames)));
        for f in 0..frames {
            let mut rows: [(Vec<Complex<f32>>, Vec<Complex<f32>>); 3] =
                std::array::from_fn(|_| (Vec::with_capacity(bins), Vec::with_capacity(bins)));
            for k in 0..bins {
                let (h2, p2) = (harm[f][k].powi(2), perc[f][k].powi(2));
                let mh = h2 / (h2 + p2 + 1e-12);
                let mp = 1.0 - mh;
                let (l, r) = (sl[f][k], sr_[f][k]);
                let centre = 1.0 - ((l - r).norm() / (l.norm() + r.norm() + 1e-9)).clamp(0.0, 1.0);
                let lw = low_w(k);
                let m_drums = mp;
                let m_bass = mh * lw;
                let m_vocals = mh * (1.0 - lw) * voc_band(k) * centre * centre;
                for (i, m) in [m_drums, m_bass, m_vocals].into_iter().enumerate() {
                    rows[i].0.push(l * m);
                    rows[i].1.push(r * m);
                }
            }
            for i in 0..3 {
                let (a, b) = std::mem::take(&mut rows[i]);
                stem_spec[i].0.push(a);
                stem_spec[i].1.push(b);
            }
        }

        let render = |s: &(Spec, Spec)| -> (Vec<f32>, Vec<f32>) { (istft(&s.0, len, &win, &ifft), istft(&s.1, len, &win, &ifft)) };
        let (dl, dr) = render(&stem_spec[0]);
        let (bl, br) = render(&stem_spec[1]);
        let (vl, vr) = render(&stem_spec[2]);
        // "other" is the exact residual, so the four stems always sum back to the input.
        let resid = |orig: &[f32], a: &[f32], b: &[f32], c: &[f32]| -> Vec<f32> {
            (0..len).map(|i| orig[i] - a[i] - b[i] - c[i]).collect()
        };
        let (ol, or) = (resid(&t.left, &dl, &bl, &vl), resid(&t.right, &dr, &br, &vr));
        let mk = |l, r| Track { sample_rate: t.sample_rate, left: l, right: r };
        Ok(Stems { drums: mk(dl, dr), bass: mk(bl, br), vocals: mk(vl, vr), other: mk(ol, or) })
    }
}

/// Four decks locked together, one per stem, with independent gains (mute = 0.0).
pub struct StemDeck {
    decks: [Deck; 4],
    pub gains: [f32; 4],
}

impl StemDeck {
    pub fn new(output_rate: u32, stems: Stems) -> Self {
        let mut decks = [Deck::new(output_rate), Deck::new(output_rate), Deck::new(output_rate), Deck::new(output_rate)];
        for (d, t) in decks.iter_mut().zip([stems.drums, stems.bass, stems.vocals, stems.other]) { d.load(t); }
        Self { decks, gains: [1.0; 4] }
    }
    pub fn play(&mut self) { self.decks.iter_mut().for_each(Deck::play); }
    pub fn pause(&mut self) { self.decks.iter_mut().for_each(Deck::pause); }
    pub fn seek_frames(&mut self, f: f64) { self.decks.iter_mut().for_each(|d| d.seek_frames(f)); }
    pub fn set_tempo(&mut self, r: f64) { self.decks.iter_mut().for_each(|d| d.set_tempo(r)); }
    pub fn position_frames(&self) -> f64 { self.decks[0].position_frames() }
    pub fn set_gain(&mut self, stem: usize, g: f32) { if stem < 4 { self.gains[stem] = g.max(0.0); } }

    pub fn render(&mut self, out_l: &mut [f32], out_r: &mut [f32]) {
        let n = out_l.len().min(out_r.len());
        out_l[..n].fill(0.0);
        out_r[..n].fill(0.0);
        let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
        for (d, g) in self.decks.iter_mut().zip(self.gains) {
            d.render(&mut l, &mut r);
            for i in 0..n { out_l[i] += l[i] * g; out_r[i] += r[i] * g; }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    const SR: u32 = 22050;

    fn synth() -> (Track, Vec<f32>, Vec<f32>) {
        let n = SR as usize * 4;
        let bass: Vec<f32> = (0..n).map(|i| 0.5 * (2.0 * PI * 60.0 * i as f32 / SR as f32).sin()).collect();
        let voice: Vec<f32> = (0..n).map(|i| {
            let t = i as f32 / SR as f32;
            0.3 * (2.0 * PI * 800.0 * t).sin() + 0.15 * (2.0 * PI * 1600.0 * t).sin()
        }).collect();
        let mut seed = 12345u32;
        let mut drums = vec![0.0f32; n];
        let period = SR as usize / 2;
        let mut s = 0;
        while s + 300 < n {
            for k in 0..300 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                drums[s + k] = ((seed >> 8) as f32 / 8388608.0 - 1.0) * 0.8 * (1.0 - k as f32 / 300.0);
            }
            s += period;
        }
        let mix: Vec<f32> = (0..n).map(|i| bass[i] + voice[i] + drums[i]).collect();
        (Track { sample_rate: SR, left: mix.clone(), right: mix }, bass, voice)
    }

    fn corr(a: &[f32], b: &[f32]) -> f32 {
        let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
        for i in 0..a.len() { ab += a[i] * b[i]; aa += a[i] * a[i]; bb += b[i] * b[i]; }
        ab / (aa.sqrt() * bb.sqrt() + 1e-12)
    }

    #[test]
    fn stems_sum_to_input() {
        let (t, _, _) = synth();
        let s = SpectralSeparator::default().separate(&t).unwrap();
        for i in 0..t.frames() {
            let sum = s.drums.left[i] + s.bass.left[i] + s.vocals.left[i] + s.other.left[i];
            assert!((sum - t.left[i]).abs() < 1e-4);
        }
    }

    #[test]
    fn separates_bass_and_voice() {
        let (t, bass, voice) = synth();
        let s = SpectralSeparator::default().separate(&t).unwrap();
        let (a, b) = (SR as usize / 2, t.frames() - SR as usize / 2); // skip edges
        let cb = corr(&s.bass.left[a..b], &bass[a..b]);
        let cv = corr(&s.vocals.left[a..b], &voice[a..b]);
        println!("bass corr {cb}, voice corr {cv}");
        assert!(cb > 0.9, "bass corr {cb}");
        assert!(cv > 0.8, "voice corr {cv}");
        // Bass stem must not carry the voice and vice versa.
        assert!(corr(&s.bass.left[a..b], &voice[a..b]).abs() < 0.2);
        assert!(corr(&s.vocals.left[a..b], &bass[a..b]).abs() < 0.2);
    }

    #[test]
    fn stem_deck_mute_removes_stem() {
        let (t, bass, _) = synth();
        let s = SpectralSeparator::default().separate(&t).unwrap();
        let mut d = StemDeck::new(SR, s);
        d.set_gain(1, 0.0); // mute bass
        d.seek_frames(SR as f64);
        d.play();
        let (mut l, mut r) = (vec![0.0; SR as usize], vec![0.0; SR as usize]);
        d.render(&mut l, &mut r);
        let off = SR as usize;
        assert!(corr(&l, &bass[off..off + l.len()]).abs() < 0.2);
    }
}
