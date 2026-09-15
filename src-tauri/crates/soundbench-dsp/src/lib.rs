//! Measurements for an audio interface bench.
//!
//! Everything in this crate is a pure function from sample buffers to numbers.
//! The device side (`soundbench-audio`) plays what [`signal`] generates, records
//! what comes back, and hands both buffers here. That split is what makes the
//! measurements testable: every analysis has a test that runs it through a
//! simulated loopback with a known delay, gain, filter and distortion, and
//! checks the numbers come out.
//!
//! Conventions:
//!
//! - Samples are `f32` in the range −1..1. **0 dBFS is a full-scale sine**
//!   (peak 1.0, RMS 1/√2), the convention interfaces and AES17 use, so a tone
//!   generated at −12 dBFS and looped through a unity path reads −12 dBFS.
//! - Frequencies are Hz, times are seconds, delays are in samples of the
//!   stream's rate — fractional where the estimator can resolve it.
//! - Spectra are reported on a log-spaced frequency grid ([`response::LogGrid`])
//!   so a plot on a log axis gets the same density everywhere.

pub mod fft;
pub mod latency;
pub mod noise;
pub mod response;
pub mod signal;
pub mod stability;
pub mod tone;
pub mod transfer;
pub mod window;

/// Power ratio to decibels, floored so silence is a number rather than −∞.
pub fn db_power(ratio: f64) -> f64 {
    if ratio <= 0.0 {
        -200.0
    } else {
        10.0 * ratio.log10().max(-20.0)
    }
}

/// Amplitude ratio to decibels, floored so silence is a number rather than −∞.
pub fn db_amp(ratio: f64) -> f64 {
    if ratio <= 0.0 {
        -200.0
    } else {
        20.0 * ratio.log10().max(-10.0)
    }
}

/// dBFS of an RMS value under the full-scale-sine convention.
pub fn dbfs_rms(rms: f64) -> f64 {
    db_amp(rms * std::f64::consts::SQRT_2)
}

/// RMS of a buffer.
pub fn rms(x: &[f32]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    let sum: f64 = x.iter().map(|&v| (v as f64) * (v as f64)).sum();
    (sum / x.len() as f64).sqrt()
}

/// Largest absolute sample.
pub fn peak(x: &[f32]) -> f64 {
    x.iter().fold(0.0f64, |m, &v| m.max((v as f64).abs()))
}

/// Mean of a buffer — the DC offset of a captured signal.
pub fn mean(x: &[f32]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    x.iter().map(|&v| v as f64).sum::<f64>() / x.len() as f64
}

#[cfg(test)]
pub(crate) mod testutil {
    //! A simulated loopback: the thing every analysis test runs through.

    /// Delay by an integer number of samples, scale, add a first-order
    /// low-pass, add soft clipping (odd harmonics) and a touch of asymmetry
    /// (even harmonics), then add white noise. Every parameter is optional so
    /// each test can turn on only the impairment it is measuring.
    #[derive(Clone, Debug)]
    pub struct Loopback {
        pub delay: usize,
        pub gain: f64,
        /// Cut-off of a one-pole low-pass in Hz, or 0 for none.
        pub lowpass_hz: f64,
        /// Cubic distortion coefficient: y = x − k·x³ (0 for none).
        pub cubic: f64,
        /// Quadratic distortion coefficient: y = x + k·x² (0 for none).
        pub quadratic: f64,
        /// White noise RMS (0 for none).
        pub noise_rms: f64,
        pub invert: bool,
    }

    impl Default for Loopback {
        fn default() -> Self {
            Loopback {
                delay: 0,
                gain: 1.0,
                lowpass_hz: 0.0,
                cubic: 0.0,
                quadratic: 0.0,
                noise_rms: 0.0,
                invert: false,
            }
        }
    }

    impl Loopback {
        pub fn run(&self, x: &[f32], sr: f64, out_len: usize) -> Vec<f32> {
            let mut y = vec![0.0f32; out_len];
            let mut rng = crate::signal::Rng::new(0x5eed);
            let alpha = if self.lowpass_hz > 0.0 {
                let dt = 1.0 / sr;
                let rc = 1.0 / (2.0 * std::f64::consts::PI * self.lowpass_hz);
                dt / (rc + dt)
            } else {
                1.0
            };
            let mut state = 0.0f64;
            for (n, slot) in y.iter_mut().enumerate() {
                let src = if n >= self.delay { x.get(n - self.delay).copied().unwrap_or(0.0) as f64 } else { 0.0 };
                let mut v = src * self.gain;
                if self.invert {
                    v = -v;
                }
                v = v - self.cubic * v * v * v + self.quadratic * v * v;
                state += alpha * (v - state);
                let mut out = state;
                if self.noise_rms > 0.0 {
                    out += rng.gaussian() * self.noise_rms;
                }
                *slot = out as f32;
            }
            y
        }
    }
}
