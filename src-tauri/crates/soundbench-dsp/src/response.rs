//! Frequency response, phase and harmonic distortion from a log sweep.
//!
//! Farina's method. The recording of an exponential sweep is convolved with
//! the sweep's inverse filter, which turns the linear part of the path into
//! an impulse response and — because an exponential sweep spends equal time
//! per octave — turns each order of harmonic distortion into its own smaller
//! impulse response, arriving a fixed time *before* the linear one:
//! `L·ln(k)` seconds for the `k`-th harmonic. Windowing each one out and
//! transforming it gives the linear response and the distortion of each
//! order as a function of frequency, from a single ten-second stimulus.
//!
//! The reference for every ratio is the sweep convolved with its own inverse
//! filter: a perfect wire scores 0 dB flat, and the sweep's own band-edge
//! shape and the fades cancel out of the measurement rather than appearing
//! in it.

use serde::Serialize;

use crate::fft::{self, C64};
use crate::signal::SweepSpec;
use crate::window::tukey_asym;

/// Log-spaced frequencies, `per_octave` points per octave from `lo` to `hi`.
pub fn log_grid(lo: f64, hi: f64, per_octave: usize) -> Vec<f64> {
    let octaves = (hi / lo).log2();
    let n = (octaves * per_octave as f64).ceil() as usize + 1;
    (0..n)
        .map(|i| lo * 2f64.powf(i as f64 / per_octave as f64))
        .filter(|&f| f <= hi * 1.0000001)
        .collect()
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepAnalysis {
    pub found: bool,
    /// Round-trip delay to the linear impulse response, in samples (fractional).
    pub delay_samples: f64,
    /// Broadband gain of the path: the impulse response peak relative to the
    /// reference, in dB.
    pub peak_gain_db: f64,
    pub freqs: Vec<f64>,
    /// Linear response magnitude in dB relative to a perfect wire.
    pub magnitude_db: Vec<f64>,
    /// Excess phase in degrees, unwrapped, with the round-trip delay removed.
    pub phase_deg: Vec<f64>,
    /// `harmonics_db[k-2][i]`: level of the `k`-th harmonic product relative
    /// to the fundamental, for a fundamental at `freqs[i]`. `NaN` where the
    /// harmonic would fall above the sweep's top frequency.
    pub harmonics_db: Vec<Vec<f64>>,
    pub thd_percent: Vec<f64>,
    pub thd_db: Vec<f64>,
    /// A short stretch of the normalised impulse response around its peak,
    /// for display; `ir_pre` samples of it come before the peak.
    pub ir: Vec<f32>,
    pub ir_pre: usize,
    /// Highest harmonic order analysed.
    pub max_harmonic: u32,
}

pub struct SweepOptions {
    /// Longest round-trip delay to search for, in samples.
    pub max_delay: usize,
    /// Points per octave on the output grid.
    pub per_octave: usize,
    /// Highest harmonic order to extract.
    pub max_harmonic: u32,
}

impl Default for SweepOptions {
    fn default() -> Self {
        SweepOptions { max_delay: 48000, per_octave: 48, max_harmonic: 5 }
    }
}

fn argmax_abs(x: &[f64]) -> (usize, f64) {
    x.iter().enumerate().fold((0, 0.0), |m, (i, &v)| if v.abs() > m.1 { (i, v.abs()) } else { m })
}

/// Sample a spectrum (bins spaced `bin_hz` apart) at `f` by linear
/// interpolation of the complex value.
fn sample_bin(spec: &[C64], bin_hz: f64, f: f64) -> C64 {
    let pos = f / bin_hz;
    let i = pos.floor() as usize;
    if i + 1 >= spec.len() {
        return spec[spec.len() - 1];
    }
    let t = pos - i as f64;
    spec[i] * (1.0 - t) + spec[i + 1] * t
}

/// Analyse the recording of a sweep played at frame `played_at`.
pub fn analyse(spec: &SweepSpec, captured: &[f32], played_at: usize, opts: &SweepOptions) -> SweepAnalysis {
    let sr = spec.sr;
    let sweep: Vec<f64> = spec.samples().iter().map(|&v| v as f64).collect();
    let inv = spec.inverse_filter();
    let n = sweep.len();

    // Reference: the sweep through a perfect wire.
    let ref_ir = fft::convolve(&sweep, &inv);
    let (p_ref, v_ref) = argmax_abs(&ref_ir);

    // The recording, from where the sweep started to as far as the delay
    // search and a tail for the linear response can reach.
    let tail = (0.5 * sr) as usize;
    let end = (played_at + n + opts.max_delay + tail).min(captured.len());
    let empty = SweepAnalysis {
        found: false,
        delay_samples: 0.0,
        peak_gain_db: -200.0,
        freqs: vec![],
        magnitude_db: vec![],
        phase_deg: vec![],
        harmonics_db: vec![],
        thd_percent: vec![],
        thd_db: vec![],
        ir: vec![],
        ir_pre: 0,
        max_harmonic: opts.max_harmonic,
    };
    if played_at >= captured.len() || end <= played_at + n {
        return empty;
    }
    let seg: Vec<f64> = captured[played_at..end].iter().map(|&v| v as f64).collect();
    let mut ir = fft::convolve(&seg, &inv);
    let scale = 1.0 / v_ref;
    ir.iter_mut().for_each(|v| *v *= scale);

    // Find the linear response: the strongest peak between zero delay and
    // the maximum, judged against the level of everything else in that span.
    let search_end = (p_ref + opts.max_delay).min(ir.len() - 1);
    let (k_rel, pk) = argmax_abs(&ir[p_ref..=search_end]);
    let k = p_ref + k_rel;
    let rms = (ir[p_ref..=search_end].iter().map(|v| v * v).sum::<f64>() / (search_end - p_ref + 1) as f64).sqrt();
    if rms <= 0.0 || pk / rms < 20.0 {
        return empty;
    }
    let frac = if k > 0 && k + 1 < ir.len() {
        crate::latency::parabolic_offset(ir[k - 1].abs(), ir[k].abs(), ir[k + 1].abs())
    } else {
        0.0
    };
    let delay = (k as f64 - p_ref as f64) + frac;
    let peak_gain_db = crate::db_amp(pk);

    // Windows. Harmonics sit before the peak, so the pre-window is bounded by
    // half the distance to the second harmonic; the post-window only has to
    // stay inside the array.
    let h2 = spec.harmonic_offset(2);
    let pre = ((0.05 * sr) as usize).min((h2 / 2.0) as usize).min(k).min(p_ref);
    let post = ((0.3 * sr) as usize).min(ir.len() - 1 - k).min(ref_ir.len() - 1 - p_ref);
    let lin_len = pre + post;
    let taper = ((0.002 * sr) as usize).max(8);
    let win = tukey_asym(lin_len, pre.min(taper * 4), taper);

    let mut lin_seg: Vec<f64> = ir[k - pre..k + post].to_vec();
    let mut ref_seg: Vec<f64> = ref_ir[p_ref - pre..p_ref + post].iter().map(|v| v * scale).collect();
    lin_seg.iter_mut().zip(&win).for_each(|(v, w)| *v *= w);
    ref_seg.iter_mut().zip(&win).for_each(|(v, w)| *v *= w);

    // Harmonic segments, each centred L·ln(k) before the linear peak.
    let mut harm_segs: Vec<Option<(usize, Vec<f64>)>> = Vec::new();
    for h in 2..=opts.max_harmonic {
        let centre = k as f64 - spec.harmonic_offset(h);
        let gap_prev = spec.harmonic_offset(h) - spec.harmonic_offset(h - 1);
        let gap_next = spec.harmonic_offset(h + 1) - spec.harmonic_offset(h);
        let half = (gap_prev.min(gap_next) / 2.0) as usize;
        let c = centre.round() as i64;
        if c - (half as i64) < 0 || half < 16 {
            harm_segs.push(None);
            continue;
        }
        let start = (c - half as i64) as usize;
        let len = 2 * half;
        if start + len > ir.len() {
            harm_segs.push(None);
            continue;
        }
        let w = tukey_asym(len, half / 2, half / 2);
        let s: Vec<f64> = ir[start..start + len].iter().zip(&w).map(|(v, w)| v * w).collect();
        harm_segs.push(Some((half, s)));
    }

    // One FFT length for everything so bins line up.
    let longest = harm_segs.iter().flatten().map(|(_, s)| s.len()).max().unwrap_or(0).max(lin_len);
    let nfft = fft::next_pow2(longest) * 2;
    let bin_hz = sr / nfft as f64;
    let lin_f = fft::forward(&lin_seg, nfft);
    let ref_f = fft::forward(&ref_seg, nfft);
    let harm_f: Vec<Option<Vec<C64>>> = harm_segs
        .iter()
        .map(|s| s.as_ref().map(|(_, s)| fft::forward(s, nfft)))
        .collect();

    let f_lo = spec.f1.max(10.0);
    let f_hi = spec.f2.min(0.48 * sr);
    let freqs = log_grid(f_lo, f_hi, opts.per_octave);

    let mut magnitude_db = Vec::with_capacity(freqs.len());
    let mut phase_raw = Vec::with_capacity(freqs.len());
    let mut lin_mag = Vec::with_capacity(freqs.len());
    for &f in &freqs {
        let r = sample_bin(&ref_f, bin_hz, f);
        let l = sample_bin(&lin_f, bin_hz, f);
        let h = if r.norm() > 1e-12 { l / r } else { C64::new(0.0, 0.0) };
        // Remove the fractional-sample delay the integer alignment left.
        let rot = C64::from_polar(1.0, 2.0 * std::f64::consts::PI * f * frac / sr);
        let h = h * rot;
        lin_mag.push(h.norm());
        magnitude_db.push(crate::db_amp(h.norm()));
        phase_raw.push(h.arg());
    }
    let phase_deg = unwrap_degrees(&phase_raw);

    let mut harmonics_db: Vec<Vec<f64>> = Vec::new();
    let mut thd_pow = vec![0.0f64; freqs.len()];
    for (hi, hf) in harm_f.iter().enumerate() {
        let order = (hi + 2) as f64;
        let mut row = Vec::with_capacity(freqs.len());
        for (i, &f) in freqs.iter().enumerate() {
            let fk = f * order;
            // Below twice the start frequency the harmonic products land
            // where the sweep's own fade-in still colours the response, so
            // the first octave is not reported.
            if fk > f_hi || f < 2.0 * spec.f1 || hf.is_none() {
                row.push(f64::NAN);
                continue;
            }
            let r = sample_bin(&ref_f, bin_hz, fk).norm();
            let d = sample_bin(hf.as_ref().unwrap(), bin_hz, fk).norm();
            let rel = if r > 1e-12 && lin_mag[i] > 1e-12 { (d / r) / lin_mag[i] } else { 0.0 };
            thd_pow[i] += rel * rel;
            row.push(crate::db_amp(rel));
        }
        harmonics_db.push(row);
    }
    let thd: Vec<f64> = thd_pow.iter().zip(&freqs).map(|(p, &f)| if f < 2.0 * spec.f1 { f64::NAN } else { p.sqrt() }).collect();
    let thd_percent = thd.iter().map(|t| t * 100.0).collect();
    let thd_db = thd.iter().map(|&t| if t.is_nan() { f64::NAN } else { crate::db_amp(t) }).collect();

    // A display stretch: 2 ms before the peak, 10 ms after.
    let ir_pre = ((0.002 * sr) as usize).min(k);
    let ir_post = ((0.010 * sr) as usize).min(ir.len() - k);
    let ir_disp: Vec<f32> = ir[k - ir_pre..k + ir_post].iter().map(|&v| v as f32).collect();

    SweepAnalysis {
        found: true,
        delay_samples: delay,
        peak_gain_db,
        freqs,
        magnitude_db,
        phase_deg,
        harmonics_db,
        thd_percent,
        thd_db,
        ir: ir_disp,
        ir_pre,
        max_harmonic: opts.max_harmonic,
    }
}

/// Unwrap a phase sequence (radians) and convert to degrees.
pub fn unwrap_degrees(phase: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(phase.len());
    let mut offset = 0.0;
    let mut prev = 0.0;
    for (i, &p) in phase.iter().enumerate() {
        if i > 0 {
            let d = p - prev;
            if d > std::f64::consts::PI {
                offset -= 2.0 * std::f64::consts::PI;
            } else if d < -std::f64::consts::PI {
                offset += 2.0 * std::f64::consts::PI;
            }
        }
        prev = p;
        out.push((p + offset).to_degrees());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::Loopback;

    fn spec() -> SweepSpec {
        SweepSpec { sr: 48000.0, f1: 20.0, f2: 20000.0, duration_s: 4.0, amp: 0.25, fade_s: 0.005 }
    }

    fn run(lb: &Loopback, spec: &SweepSpec) -> SweepAnalysis {
        let x = spec.samples();
        let played_at = 2400;
        let mut out = vec![0.0f32; played_at];
        out.extend_from_slice(&x);
        out.extend(std::iter::repeat(0.0f32).take(48000));
        let cap = lb.run(&out, spec.sr, out.len());
        analyse(spec, &cap, played_at, &SweepOptions { max_delay: 24000, ..Default::default() })
    }

    fn at(a: &SweepAnalysis, f: f64) -> usize {
        a.freqs.iter().enumerate().min_by(|x, y| (x.1 - f).abs().partial_cmp(&(y.1 - f).abs()).unwrap()).unwrap().0
    }

    #[test]
    fn a_wire_with_delay_is_flat_and_the_delay_is_read() {
        let s = spec();
        let a = run(&Loopback { delay: 777, gain: 0.5, ..Default::default() }, &s);
        assert!(a.found);
        assert!((a.delay_samples - 777.0).abs() < 0.1, "delay {}", a.delay_samples);
        assert!((a.peak_gain_db + 6.02).abs() < 0.1, "gain {}", a.peak_gain_db);
        for (f, m) in a.freqs.iter().zip(&a.magnitude_db) {
            if *f > 40.0 && *f < 18000.0 {
                assert!((m + 6.02).abs() < 0.2, "{f} Hz: {m} dB");
            }
        }
        let i = at(&a, 1000.0);
        assert!(a.phase_deg[i].abs() < 3.0, "phase {}", a.phase_deg[i]);
        // A linear path has no distortion to speak of.
        assert!(a.thd_percent[i] < 0.05, "thd {}", a.thd_percent[i]);
    }

    #[test]
    fn a_low_pass_reads_as_one() {
        let s = spec();
        let fc = 5000.0;
        let a = run(&Loopback { delay: 100, lowpass_hz: fc, ..Default::default() }, &s);
        assert!(a.found);
        // The simulator's filter is the discrete one-pole
        // y[n] = y[n−1] + α(x[n] − y[n−1]), whose response at ω is
        // α / (1 − (1−α)e^(−jω)); compare against that exactly rather than
        // against the analogue RC it approximates.
        let dt = 1.0 / s.sr;
        let rc = 1.0 / (2.0 * std::f64::consts::PI * fc);
        let alpha = dt / (rc + dt);
        let expect = |f: f64| -> (f64, f64) {
            let w = 2.0 * std::f64::consts::PI * f / s.sr;
            let den = C64::new(1.0 - (1.0 - alpha) * w.cos(), (1.0 - alpha) * w.sin());
            let h = C64::new(alpha, 0.0) / den;
            (crate::db_amp(h.norm()), h.arg().to_degrees())
        };
        // The reported delay includes the parabolic refinement of the peak,
        // which on an asymmetric (minimum-phase) impulse response lands a
        // fraction of a sample late; the phase is presented with that same
        // fraction removed, so the two agree with each other and with the
        // filter once it is put back.
        let frac = a.delay_samples - 100.0;
        assert!(frac.abs() < 0.5, "frac {frac}");
        for f in [100.0, 500.0, 1000.0, 2000.0, fc, 2.0 * fc, 15000.0] {
            let i = at(&a, f);
            let (m, p) = expect(a.freqs[i]);
            let p = p + 360.0 * a.freqs[i] * frac / s.sr;
            assert!((a.magnitude_db[i] - m).abs() < 0.05, "at {f}: {} dB, expected {m}", a.magnitude_db[i]);
            assert!((a.phase_deg[i] - p).abs() < 0.5, "phase at {f}: {}, expected {p}", a.phase_deg[i]);
        }
    }

    #[test]
    fn cubic_distortion_shows_as_third_harmonic() {
        let s = spec();
        // y = x − k x³ on a sine of amplitude A gives a third harmonic of
        // relative amplitude k·A²/4 / (1 − 3kA²/4).
        let k = 0.4;
        let a = run(&Loopback { delay: 50, cubic: k, ..Default::default() }, &s);
        assert!(a.found);
        let amp = s.amp;
        let expect = (k * amp * amp / 4.0) / (1.0 - 3.0 * k * amp * amp / 4.0);
        let expect_db = crate::db_amp(expect);
        for f in [200.0, 1000.0, 3000.0] {
            let i = at(&a, f);
            let h3 = a.harmonics_db[1][i];
            let h2 = a.harmonics_db[0][i];
            assert!((h3 - expect_db).abs() < 1.5, "{f} Hz: H3 {h3} dB, expected {expect_db}");
            assert!(h2 < expect_db - 20.0, "{f} Hz: H2 {h2} dB should be far below H3");
            assert!((a.thd_percent[i] - expect * 100.0).abs() < expect * 100.0 * 0.2, "thd {}", a.thd_percent[i]);
        }
    }

    #[test]
    fn quadratic_distortion_shows_as_second_harmonic() {
        let s = spec();
        // y = x + k x² gives a second harmonic of relative amplitude k·A/2.
        let k = 0.2;
        let a = run(&Loopback { delay: 50, quadratic: k, ..Default::default() }, &s);
        let expect_db = crate::db_amp(k * s.amp / 2.0);
        let i = at(&a, 1000.0);
        assert!((a.harmonics_db[0][i] - expect_db).abs() < 1.5, "H2 {} expected {expect_db}", a.harmonics_db[0][i]);
    }

    #[test]
    fn silence_is_not_found() {
        let s = spec();
        let cap = crate::signal::white_noise(48000 * 6, 0.0005, 9);
        let a = analyse(&s, &cap, 2400, &SweepOptions::default());
        assert!(!a.found);
    }

    #[test]
    fn grid_is_log_spaced() {
        let g = log_grid(20.0, 20000.0, 12);
        assert!((g[0] - 20.0).abs() < 1e-9);
        assert!((g[12] - 40.0).abs() < 1e-9);
        assert!(*g.last().unwrap() <= 20000.0001);
    }
}
