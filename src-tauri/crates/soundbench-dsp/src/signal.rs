//! Test signals.
//!
//! Every generator is deterministic — the noises are seeded — so a test that
//! plays a signal through a simulated loopback gets the same numbers every
//! run, and a measurement's report can name the exact stimulus.
//!
//! Levels are given in dBFS under the full-scale-sine convention, so a
//! `-12 dBFS` sine has peak 0.25. Noise is set by RMS: `-12 dBFS` pink noise
//! has the same RMS as that sine, and peaks well above it (pink noise has a
//! crest factor around 12 dB), which the level helper accounts for by
//! refusing anything that would clip.

use std::f64::consts::PI;

/// Peak amplitude of a sine at the given dBFS.
pub fn amplitude(dbfs: f64) -> f64 {
    10f64.powf(dbfs / 20.0)
}

/// Apply a raised-cosine fade of `fade` samples to both ends, in place.
pub fn fade_ends(x: &mut [f32], fade: usize) {
    let fade = fade.min(x.len() / 2);
    for i in 0..fade {
        let g = (0.5 - 0.5 * (PI * i as f64 / fade as f64).cos()) as f32;
        x[i] *= g;
        let j = x.len() - 1 - i;
        x[j] *= g;
    }
}

/// A sine of `n` samples at `freq` Hz, peak `amp`, with `fade` samples of
/// raised-cosine at each end so the transitions do not click.
pub fn sine(sr: f64, freq: f64, amp: f64, n: usize, fade: usize) -> Vec<f32> {
    let w = 2.0 * PI * freq / sr;
    let mut x: Vec<f32> = (0..n).map(|i| (amp * (w * i as f64).sin()) as f32).collect();
    fade_ends(&mut x, fade);
    x
}

/// `n` samples of silence.
pub fn silence(n: usize) -> Vec<f32> {
    vec![0.0; n]
}

/// xorshift64* — small, fast, and good enough for test noise. Not crypto.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in −1..1.
    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }

    /// Standard normal, by Box–Muller.
    pub fn gaussian(&mut self) -> f64 {
        let u1 = ((self.next_u64() >> 11) as f64 + 1.0) / (1u64 << 53) as f64;
        let u2 = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
    }
}

/// Gaussian white noise at the given RMS.
pub fn white_noise(n: usize, rms: f64, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    (0..n).map(|_| (rng.gaussian() * rms) as f32).collect()
}

/// Pink noise (−3 dB/octave) at the given RMS, by Paul Kellet's refined
/// seven-pole filter on white noise, then normalised to the requested RMS.
pub fn pink_noise(n: usize, rms: f64, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let mut b = [0.0f64; 7];
    let mut out: Vec<f64> = Vec::with_capacity(n);
    for _ in 0..n {
        let w = rng.gaussian();
        b[0] = 0.99886 * b[0] + w * 0.0555179;
        b[1] = 0.99332 * b[1] + w * 0.0750759;
        b[2] = 0.96900 * b[2] + w * 0.1538520;
        b[3] = 0.86650 * b[3] + w * 0.3104856;
        b[4] = 0.55000 * b[4] + w * 0.5329522;
        b[5] = -0.7616 * b[5] - w * 0.0168980;
        let pink = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + w * 0.5362;
        b[6] = w * 0.115926;
        out.push(pink);
    }
    let cur = (out.iter().map(|v| v * v).sum::<f64>() / n.max(1) as f64).sqrt();
    let g = if cur > 0.0 { rms / cur } else { 0.0 };
    out.iter().map(|v| (v * g) as f32).collect()
}

/// Exponential (logarithmic) sine sweep, after Farina.
///
/// `x(t) = A·sin(2π·f1·L·(e^(t/L) − 1))`, `L = T / ln(f2/f1)`. The
/// instantaneous frequency rises from `f1` to `f2` over `duration_s`, spending
/// equal time in every octave — which is what puts the harmonic distortion
/// products at fixed, separable offsets in the deconvolved response.
#[derive(Clone, Debug, PartialEq)]
pub struct SweepSpec {
    pub sr: f64,
    pub f1: f64,
    pub f2: f64,
    pub duration_s: f64,
    /// Peak amplitude.
    pub amp: f64,
    /// Raised-cosine fade at each end, in seconds.
    pub fade_s: f64,
}

impl SweepSpec {
    /// The sweep rate constant `L` in seconds.
    pub fn l(&self) -> f64 {
        self.duration_s / (self.f2 / self.f1).ln()
    }

    pub fn len(&self) -> usize {
        (self.duration_s * self.sr).round() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The sweep as played.
    pub fn samples(&self) -> Vec<f32> {
        let l = self.l();
        let k = 2.0 * PI * self.f1 * l;
        let n = self.len();
        let mut x: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f64 / self.sr;
                (self.amp * (k * ((t / l).exp() - 1.0)).sin()) as f32
            })
            .collect();
        fade_ends(&mut x, (self.fade_s * self.sr).round() as usize);
        x
    }

    /// The inverse filter: the sweep time-reversed, with an envelope decaying
    /// by 6 dB per octave so that `sweep ⊛ inverse` has a flat spectrum. Not
    /// normalised — callers divide by the peak of the reference response
    /// (`sweep ⊛ inverse`), which also cancels the fade's small effect.
    pub fn inverse_filter(&self) -> Vec<f64> {
        let l = self.l();
        let x = self.samples();
        let n = x.len();
        (0..n)
            .map(|i| {
                let t = i as f64 / self.sr;
                x[n - 1 - i] as f64 * (-t / l).exp()
            })
            .collect()
    }

    /// Where the `k`-th harmonic's impulse response lands, in samples before
    /// the linear response: `L·ln(k)`.
    pub fn harmonic_offset(&self, k: u32) -> f64 {
        self.l() * (k as f64).ln() * self.sr
    }
}

/// A short linear chirp with a Hann envelope, for latency measurement: a
/// waveform whose autocorrelation is a single sharp peak, so its arrival can
/// be located to a fraction of a sample by cross-correlation even through a
/// noisy or band-limited path. 200 Hz to 40 % of the sample rate, so it stays
/// well inside any interface's passband.
pub fn latency_burst(sr: f64, duration_s: f64, amp: f64) -> Vec<f32> {
    let n = (duration_s * sr).round() as usize;
    let f0 = 200.0;
    let f1 = 0.4 * sr;
    let rate = (f1 - f0) / duration_s;
    (0..n)
        .map(|i| {
            let t = i as f64 / sr;
            let phase = 2.0 * PI * (f0 * t + 0.5 * rate * t * t);
            let env = 0.5 - 0.5 * (2.0 * PI * i as f64 / n as f64).cos();
            (amp * env * phase.sin()) as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dbfs_rms, rms};

    #[test]
    fn sine_level_is_what_was_asked_for() {
        let x = sine(48000.0, 1000.0, amplitude(-12.0), 48000, 0);
        assert!((dbfs_rms(rms(&x)) + 12.0).abs() < 0.01);
    }

    #[test]
    fn noises_hit_their_rms() {
        let w = white_noise(100_000, 0.1, 1);
        assert!((rms(&w) - 0.1).abs() < 0.002);
        let p = pink_noise(100_000, 0.1, 1);
        assert!((rms(&p) - 0.1).abs() < 1e-6);
    }

    #[test]
    fn pink_noise_is_pink() {
        // Compare band power an octave apart: pink noise loses 3 dB per octave,
        // so equal-width bands at 1 k and 2 k differ by about 3 dB.
        let sr = 48000.0;
        let x = pink_noise(1 << 18, 0.1, 7);
        let xd: Vec<f64> = x.iter().map(|&v| v as f64).collect();
        let bins = crate::fft::forward(&xd, 1 << 18);
        let hz = sr / (1 << 18) as f64;
        let band = |lo: f64, hi: f64| -> f64 {
            let a = (lo / hz) as usize;
            let b = (hi / hz) as usize;
            bins[a..b].iter().map(|c| c.norm_sqr()).sum::<f64>()
        };
        let low = band(900.0, 1100.0);
        let high = band(1900.0, 2100.0);
        let ratio_db = 10.0 * (low / high).log10();
        assert!((ratio_db - 3.0).abs() < 0.8, "ratio {ratio_db} dB");
    }

    #[test]
    fn sweep_starts_and_ends_at_the_right_frequencies() {
        let spec = SweepSpec { sr: 48000.0, f1: 20.0, f2: 20000.0, duration_s: 2.0, amp: 0.5, fade_s: 0.0 };
        // Instantaneous frequency f(t) = f1·e^(t/L): check the phase argument
        // derivative at the ends against the design.
        let l = spec.l();
        let f_at = |t: f64| spec.f1 * (t / l).exp();
        assert!((f_at(0.0) - 20.0).abs() < 1e-9);
        assert!((f_at(2.0) - 20000.0).abs() < 1e-6);
        assert_eq!(spec.samples().len(), 96000);
        // Harmonic 2 lands L·ln2 seconds early.
        assert!((spec.harmonic_offset(2) / 48000.0 - l * 2f64.ln()).abs() < 1e-9);
    }

    #[test]
    fn sweep_deconvolves_to_a_single_peak() {
        let spec = SweepSpec { sr: 48000.0, f1: 20.0, f2: 20000.0, duration_s: 1.0, amp: 0.5, fade_s: 0.005 };
        let x: Vec<f64> = spec.samples().iter().map(|&v| v as f64).collect();
        let inv = spec.inverse_filter();
        let ir = crate::fft::convolve(&x, &inv);
        let (k, pk) = ir.iter().enumerate().fold((0, f64::MIN), |m, (i, &v)| if v > m.1 { (i, v) } else { m });
        // The peak sits at the sweep length, and everything 2 ms away is far below it.
        assert!((k as i64 - x.len() as i64).abs() <= 1, "peak at {k}");
        let far = ir[k + 96..k + 4800].iter().fold(0.0f64, |m, &v| m.max(v.abs()));
        assert!(far / pk < 0.02, "sidelobe ratio {}", far / pk);
    }

    #[test]
    fn burst_is_bounded_and_windowed() {
        let b = latency_burst(48000.0, 0.04, 0.5);
        assert_eq!(b.len(), 1920);
        assert!(crate::peak(&b) <= 0.5);
        assert!(b[0].abs() < 1e-6);
    }
}
