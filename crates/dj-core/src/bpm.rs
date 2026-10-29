//! Tempo estimation: onset-strength envelope + autocorrelation.

pub const HOP: usize = 512;
pub const WIN: usize = 1024;

/// Spectral-flux-free, energy-based onset envelope (half-wave rectified energy difference).
pub fn onset_envelope(mono: &[f32]) -> Vec<f32> {
    let mut energy = Vec::new();
    let mut i = 0;
    while i + WIN <= mono.len() {
        energy.push(mono[i..i + WIN].iter().map(|x| x * x).sum::<f32>().sqrt());
        i += HOP;
    }
    let mut env: Vec<f32> = energy.windows(2).map(|w| (w[1] - w[0]).max(0.0)).collect();
    let mean = env.iter().sum::<f32>() / env.len().max(1) as f32;
    for e in env.iter_mut() { *e = (*e - mean).max(0.0); }
    env
}

/// Estimate BPM in the range [min_bpm, max_bpm). Returns None for audio that is too short or silent.
pub fn estimate_bpm(mono: &[f32], sample_rate: u32, min_bpm: f32, max_bpm: f32) -> Option<f32> {
    let env = onset_envelope(mono);
    if env.len() < 64 || env.iter().all(|&x| x == 0.0) { return None; }
    let fr = sample_rate as f32 / HOP as f32; // envelope frames per second
    let lag_min = (60.0 * fr / max_bpm).floor() as usize;
    let lag_max = (60.0 * fr / min_bpm).ceil() as usize;
    if lag_max + 2 >= env.len() { return None; }

    let ac = |lag: usize| -> f32 {
        env[..env.len() - lag].iter().zip(&env[lag..]).map(|(a, b)| a * b).sum::<f32>()
    };
    let (mut best, mut best_v) = (lag_min.max(1), f32::MIN);
    for lag in lag_min.max(1)..=lag_max {
        // Reinforce by the first harmonic so a true beat period beats its sub-multiples.
        let v = ac(lag) + 0.5 * if lag * 2 < env.len() { ac(lag * 2) } else { 0.0 };
        if v > best_v { best_v = v; best = lag; }
    }
    // Parabolic interpolation around the peak for sub-frame precision.
    let (y0, y1, y2) = (ac(best - 1), ac(best), ac(best + 1));
    let denom = y0 - 2.0 * y1 + y2;
    let delta = if denom.abs() > 1e-9 { 0.5 * (y0 - y2) / denom } else { 0.0 };
    let lag = best as f32 + delta.clamp(-1.0, 1.0);
    Some(60.0 * fr / lag)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click_track(bpm: f32, secs: f32, sr: u32) -> Vec<f32> {
        let n = (secs * sr as f32) as usize;
        let mut v = vec![0.0f32; n];
        let period = 60.0 / bpm * sr as f32;
        let mut t = 0.0f32;
        while (t as usize) + 400 < n {
            for k in 0..400 {
                let env = 1.0 - k as f32 / 400.0;
                v[t as usize + k] += env * (k as f32 * 0.3).sin();
            }
            t += period;
        }
        v
    }

    #[test]
    fn detects_common_tempos() {
        for bpm in [90.0, 120.0, 128.0, 140.0] {
            let est = estimate_bpm(&click_track(bpm, 30.0, 44100), 44100, 80.0, 160.0).unwrap();
            assert!((est - bpm).abs() < 1.0, "expected {bpm}, got {est}");
        }
    }

    #[test]
    fn silence_is_none() {
        assert!(estimate_bpm(&vec![0.0; 44100 * 20], 44100, 80.0, 160.0).is_none());
    }
}
