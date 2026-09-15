//! Transfer function and coherence from a noise stimulus.
//!
//! The sweep ([`response`](crate::response)) is the precise measurement; this
//! is the cross-check that uses a different stimulus and a different
//! estimator. Pink (or white) noise is played, the recording is aligned to
//! it by phase-transform cross-correlation, and Welch-averaged cross-spectra give the H1
//! estimate `H = Sxy / Sxx` and the coherence `γ² = |Sxy|² / (Sxx·Syy)`.
//! Coherence is the part this adds: it is 1 where the output is a linear
//! function of the input and falls wherever noise or distortion dominates —
//! so it shows at a glance which part of the response plot to believe.

use serde::Serialize;

use crate::fft::{self, C64};
use crate::window::{hann, power_gain};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferAnalysis {
    pub found: bool,
    pub delay_samples: f64,
    pub freqs: Vec<f64>,
    pub magnitude_db: Vec<f64>,
    pub phase_deg: Vec<f64>,
    pub coherence: Vec<f64>,
    pub fft_size: usize,
    pub blocks: usize,
}

/// `played` is the stimulus as written to the output from frame `played_at`;
/// `captured` is the recording on the same frame clock.
pub fn analyse(played: &[f32], captured: &[f32], played_at: usize, max_delay: usize, sr: f64, per_octave: usize) -> TransferAnalysis {
    let empty = TransferAnalysis {
        found: false,
        delay_samples: 0.0,
        freqs: vec![],
        magnitude_db: vec![],
        phase_deg: vec![],
        coherence: vec![],
        fft_size: 0,
        blocks: 0,
    };
    // Align: correlate the first half-second of the stimulus against the
    // recording around where it should have arrived.
    let probe = (0.5 * sr) as usize;
    if played.len() < probe || played_at + probe + max_delay > captured.len() {
        return empty;
    }
    let seg: Vec<f64> = captured[played_at..played_at + probe + max_delay].iter().map(|&v| v as f64).collect();
    let tpl: Vec<f64> = played[..probe].iter().map(|&v| v as f64).collect();
    let c = fft::xcorr_phat(&seg, &tpl);
    let usable = seg.len() - probe;
    let (k, pk) = c[..usable].iter().enumerate().fold((0usize, 0.0f64), |m, (i, &v)| if v.abs() > m.1 { (i, v.abs()) } else { m });
    let rms = (c[..usable].iter().map(|v| v * v).sum::<f64>() / usable as f64).sqrt();
    // PHAT whitening makes the correlation floor noise-like, so a real
    // alignment stands well above six times its RMS.
    if rms <= 0.0 || pk / rms < 6.0 {
        return empty;
    }
    let frac = if k > 0 && k + 1 < usable { crate::latency::parabolic_offset(c[k - 1].abs(), c[k].abs(), c[k + 1].abs()) } else { 0.0 };
    let delay = k;

    // Welch cross-spectra, aligned so x[n] pairs with y[n + delay].
    let n = 8192usize;
    let hop = n / 2;
    let avail = played.len().min(captured.len().saturating_sub(played_at + delay));
    if avail < 2 * n {
        return empty;
    }
    let w = hann(n);
    let pg = power_gain(&w);
    let mut sxx = vec![0.0f64; n / 2 + 1];
    let mut syy = vec![0.0f64; n / 2 + 1];
    let mut sxy = vec![C64::new(0.0, 0.0); n / 2 + 1];
    let mut blocks = 0usize;
    let mut start = 0usize;
    while start + n <= avail {
        let x: Vec<f64> = played[start..start + n].iter().zip(&w).map(|(&v, w)| v as f64 * w).collect();
        let y: Vec<f64> = captured[played_at + delay + start..played_at + delay + start + n].iter().zip(&w).map(|(&v, w)| v as f64 * w).collect();
        let fx = fft::forward(&x, n);
        let fy = fft::forward(&y, n);
        for i in 0..=n / 2 {
            sxx[i] += fx[i].norm_sqr();
            syy[i] += fy[i].norm_sqr();
            sxy[i] += fy[i] * fx[i].conj();
        }
        blocks += 1;
        start += hop;
    }
    let _ = pg;
    let bin_hz = sr / n as f64;
    let grid = crate::response::log_grid((2.0 * bin_hz).max(20.0), 0.48 * sr, per_octave);
    let mut magnitude_db = Vec::with_capacity(grid.len());
    let mut phase = Vec::with_capacity(grid.len());
    let mut coherence = Vec::with_capacity(grid.len());
    for &f in &grid {
        let i = (f / bin_hz).round() as usize;
        let i = i.min(n / 2);
        let h = if sxx[i] > 0.0 { sxy[i] / sxx[i] } else { C64::new(0.0, 0.0) };
        // Put back the fractional sample the integer alignment left.
        let rot = C64::from_polar(1.0, 2.0 * std::f64::consts::PI * f * frac / sr);
        let h = h * rot;
        magnitude_db.push(crate::db_amp(h.norm()));
        phase.push(h.arg());
        let g = if sxx[i] > 0.0 && syy[i] > 0.0 { sxy[i].norm_sqr() / (sxx[i] * syy[i]) } else { 0.0 };
        coherence.push(g.min(1.0));
    }
    TransferAnalysis {
        found: true,
        delay_samples: delay as f64 + frac,
        freqs: grid,
        magnitude_db,
        phase_deg: crate::response::unwrap_degrees(&phase),
        coherence,
        fft_size: n,
        blocks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::pink_noise;
    use crate::testutil::Loopback;

    #[test]
    fn pink_noise_through_a_wire_is_flat_and_coherent() {
        let sr = 48000.0;
        let x = pink_noise(48000 * 6, 0.1, 4);
        let played_at = 4800;
        let mut out = vec![0.0f32; played_at];
        out.extend_from_slice(&x);
        out.extend(std::iter::repeat(0.0f32).take(24000));
        let lb = Loopback { delay: 640, gain: 0.5, noise_rms: 1e-3, ..Default::default() };
        let cap = lb.run(&out, sr, out.len());
        let a = analyse(&x, &cap, played_at, 12000, sr, 24);
        assert!(a.found);
        assert!((a.delay_samples - 640.0).abs() < 0.2, "delay {}", a.delay_samples);
        for (i, f) in a.freqs.iter().enumerate() {
            if *f > 50.0 && *f < 15000.0 {
                assert!((a.magnitude_db[i] + 6.02).abs() < 0.5, "{f} Hz: {}", a.magnitude_db[i]);
                assert!(a.coherence[i] > 0.97, "{f} Hz: coherence {}", a.coherence[i]);
                assert!(a.phase_deg[i].abs() < 5.0, "{f} Hz: phase {}", a.phase_deg[i]);
            }
        }
    }

    #[test]
    fn coherence_drops_where_noise_dominates() {
        let sr = 48000.0;
        let x = pink_noise(48000 * 6, 0.1, 4);
        let lb = Loopback { delay: 10, lowpass_hz: 1500.0, noise_rms: 1e-2, ..Default::default() };
        let cap = lb.run(&x, sr, x.len());
        let a = analyse(&x, &cap, 0, 1000, sr, 12);
        assert!(a.found);
        let lo = a.freqs.iter().position(|&f| f > 100.0).unwrap();
        let hi = a.freqs.iter().position(|&f| f > 15000.0).unwrap();
        assert!(a.coherence[lo] > 0.95, "low coherence {}", a.coherence[lo]);
        assert!(a.coherence[hi] < 0.5, "high coherence {}", a.coherence[hi]);
    }
}
