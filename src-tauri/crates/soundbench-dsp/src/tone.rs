//! THD, THD+N, SNR and level from a single steady tone.
//!
//! One long FFT of the steady-state part of the recording under a seven-term
//! cosine window (sidelobes below −180 dB, so the window's own leakage stays
//! under the noise of any real converter). The fundamental is the strongest bin; its exact
//! frequency comes from interpolating the bins around it, which is also how a
//! clock difference between the output and input converters shows up — a
//! tone generated at 1000.000 Hz that reads 1000.012 Hz has been through two
//! clocks 12 ppm apart.
//!
//! Powers are sums over the window's main lobe (±8 bins) around each peak, on
//! the convention that a full-scale sine is 0 dBFS. The band for THD+N is
//! 20 Hz to 20 kHz, capped just under Nyquist, as AES17 measures it — so a
//! device measured at 96 kHz gets the same number as at 48 kHz rather than a
//! worse one for the extra octave of noise it can carry.

use serde::Serialize;

use crate::fft;
use crate::window::{blackman_harris_7, power_gain};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToneAnalysis {
    pub found: bool,
    pub expected_hz: f64,
    pub fundamental_hz: f64,
    pub frequency_error_ppm: f64,
    /// Fundamental level in dBFS.
    pub level_dbfs: f64,
    /// `harmonics_db[k-2]`: the `k`-th harmonic relative to the fundamental (dBc).
    pub harmonics_db: Vec<f64>,
    pub thd_percent: f64,
    pub thd_db: f64,
    pub thd_n_percent: f64,
    pub thd_n_db: f64,
    pub snr_db: f64,
    pub sinad_db: f64,
    /// Noise alone (everything in band that is neither the fundamental nor
    /// a harmonic), in dBFS.
    pub noise_dbfs: f64,
    pub dc_offset: f64,
    pub fft_size: usize,
    pub band_lo_hz: f64,
    pub band_hi_hz: f64,
    /// Spectrum for display, on a log grid: dBFS per bin, max-held per cell.
    pub spectrum_freqs: Vec<f64>,
    pub spectrum_db: Vec<f64>,
}

/// Half-width of the summing window around a peak, in bins: the seven-term
/// window's main lobe.
const LOBE: usize = 8;

/// Mean-square value represented by a set of one-sided bins.
fn mean_square(bins: &[fft::C64], n: usize, pg: f64, range: std::ops::Range<usize>) -> f64 {
    let lo = range.start.min(bins.len());
    let hi = range.end.min(bins.len());
    let s: f64 = bins[lo..hi].iter().map(|c| c.norm_sqr()).sum();
    2.0 * s / (n as f64 * pg)
}

/// Analyse a captured tone. `captured` should be the steady-state part only
/// — start it after the round trip and any fade-in.
pub fn analyse(captured: &[f32], sr: f64, expected_hz: f64, max_harmonic: u32) -> ToneAnalysis {
    let n = fft::next_pow2(captured.len().max(2)) / 2;
    let n = n.max(4096).min(1 << 18);
    let empty = ToneAnalysis {
        found: false,
        expected_hz,
        fundamental_hz: 0.0,
        frequency_error_ppm: 0.0,
        level_dbfs: -200.0,
        harmonics_db: vec![],
        thd_percent: 0.0,
        thd_db: -200.0,
        thd_n_percent: 0.0,
        thd_n_db: -200.0,
        snr_db: 0.0,
        sinad_db: 0.0,
        noise_dbfs: -200.0,
        dc_offset: 0.0,
        fft_size: n,
        band_lo_hz: 20.0,
        band_hi_hz: 20000.0f64.min(0.45 * sr),
        spectrum_freqs: vec![],
        spectrum_db: vec![],
    };
    if captured.len() < n {
        return empty;
    }
    let tail = &captured[captured.len() - n..];
    let dc = crate::mean(tail);
    let w = blackman_harris_7(n);
    let pg = power_gain(&w);
    let x: Vec<f64> = tail.iter().zip(&w).map(|(&v, w)| (v as f64 - dc) * w).collect();
    let bins = fft::forward(&x, n);
    let bin_hz = sr / n as f64;
    let band_lo = 20.0;
    let band_hi = 20000.0f64.min(0.45 * sr);
    let b_lo = (band_lo / bin_hz).ceil() as usize;
    let b_hi = ((band_hi / bin_hz).floor() as usize).min(bins.len() - 1);

    // Fundamental: the strongest bin in band.
    let (k0, _) = bins[b_lo..=b_hi]
        .iter()
        .enumerate()
        .fold((0, 0.0), |m, (i, c)| if c.norm_sqr() > m.1 { (i, c.norm_sqr()) } else { m });
    let k0 = k0 + b_lo;
    let mag_db = |k: usize| 20.0 * bins[k].norm().max(1e-30).log10();
    let frac = crate::latency::parabolic_offset(mag_db(k0 - 1), mag_db(k0), mag_db(k0 + 1));
    let f0 = (k0 as f64 + frac) * bin_hz;

    let p1 = mean_square(&bins, n, pg, k0.saturating_sub(LOBE)..k0 + LOBE + 1);
    let level_dbfs = crate::dbfs_rms(p1.sqrt());
    if level_dbfs < -90.0 {
        return ToneAnalysis { dc_offset: dc, ..empty };
    }

    // DC, properly: the mean over a whole number of cycles of the tone, so
    // the tone itself does not leak into it. The first estimate above was
    // over an arbitrary span and is only good enough to centre the FFT.
    let period = sr / f0;
    let cycles = (tail.len() as f64 / period).floor().max(1.0);
    let span = ((cycles * period).round() as usize).min(tail.len());
    let dc = crate::mean(&tail[tail.len() - span..]);

    let p_total = mean_square(&bins, n, pg, b_lo..b_hi + 1);
    let mut p_harm = 0.0;
    let mut harmonics_db = Vec::new();
    for h in 2..=max_harmonic {
        let fk = f0 * h as f64;
        if fk > band_hi {
            harmonics_db.push(f64::NAN);
            continue;
        }
        let kk = (fk / bin_hz).round() as usize;
        let pk = mean_square(&bins, n, pg, kk.saturating_sub(LOBE)..kk + LOBE + 1);
        p_harm += pk;
        harmonics_db.push(crate::db_power(pk / p1));
    }
    // Floors: a bit-exact loopback leaves nothing outside the fundamental's
    // lobe but rounding, and a THD+N of −∞ helps nobody. −150 dB is below
    // any converter and above the arithmetic.
    let floor = p1 * 1e-15;
    let p_nd = (p_total - p1).max(floor);
    let p_noise = (p_nd - p_harm).max(floor);
    let thd = (p_harm / p1).sqrt().max(1e-8);
    let thd_n = (p_nd / p1).sqrt();

    // Display spectrum: dBFS per bin, max-held into 96 cells per octave.
    let grid = crate::response::log_grid(band_lo.max(bin_hz * 2.0), 0.5 * sr, 96);
    let half = 2f64.powf(1.0 / 192.0);
    let mut spectrum_db = Vec::with_capacity(grid.len());
    for &f in &grid {
        let a = ((f / half / bin_hz).round() as usize).clamp(1, bins.len() - 1);
        let b = ((f * half / bin_hz).round() as usize).clamp(a, bins.len() - 1);
        let mut best = 0.0f64;
        for k in a..=b {
            let ms = 2.0 * bins[k].norm_sqr() / (n as f64 * pg);
            best = best.max(ms);
        }
        spectrum_db.push(crate::db_power(best * 2.0));
    }

    ToneAnalysis {
        found: true,
        expected_hz,
        fundamental_hz: f0,
        frequency_error_ppm: if expected_hz > 0.0 { (f0 - expected_hz) / expected_hz * 1e6 } else { 0.0 },
        level_dbfs,
        harmonics_db,
        thd_percent: thd * 100.0,
        thd_db: crate::db_amp(thd),
        thd_n_percent: thd_n * 100.0,
        thd_n_db: crate::db_amp(thd_n),
        snr_db: crate::db_power(p1 / p_noise),
        sinad_db: crate::db_power(p1 / p_nd),
        noise_dbfs: crate::dbfs_rms(p_noise.sqrt()),
        dc_offset: dc,
        fft_size: n,
        band_lo_hz: band_lo,
        band_hi_hz: band_hi,
        spectrum_freqs: grid,
        spectrum_db,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::{amplitude, sine};
    use crate::testutil::Loopback;

    #[test]
    fn a_clean_tone_reads_its_level_and_frequency() {
        let sr = 48000.0;
        let x = sine(sr, 1000.0, amplitude(-12.0), 3 * 48000, 0);
        let a = analyse(&x, sr, 1000.0, 5);
        assert!(a.found);
        assert!((a.level_dbfs + 12.0).abs() < 0.05, "level {}", a.level_dbfs);
        assert!((a.fundamental_hz - 1000.0).abs() < 0.05, "f0 {}", a.fundamental_hz);
        assert!(a.thd_n_db < -100.0 && a.thd_n_db >= -160.0, "thd+n {}", a.thd_n_db);
        assert!(a.snr_db > 100.0 && a.snr_db <= 160.0, "snr {}", a.snr_db);
    }

    #[test]
    fn distortion_and_noise_are_measured() {
        let sr = 48000.0;
        let amp = amplitude(-6.0);
        let x = sine(sr, 997.0, amp, 4 * 48000, 0);
        let k = 0.1;
        let lb = Loopback { cubic: k, noise_rms: 1e-4, ..Default::default() };
        let y = lb.run(&x, sr, x.len());
        let a = analyse(&y, sr, 997.0, 5);
        assert!(a.found);
        let h3 = (k * amp * amp / 4.0) / (1.0 - 3.0 * k * amp * amp / 4.0);
        let expect_thd = h3 * 100.0;
        assert!((a.thd_percent - expect_thd).abs() < expect_thd * 0.05, "thd {} expected {expect_thd}", a.thd_percent);
        assert!(a.harmonics_db[0] < a.harmonics_db[1] - 30.0, "H2 {} H3 {}", a.harmonics_db[0], a.harmonics_db[1]);
        // Noise at 1e-4 RMS in a 20 kHz band out of a 24 kHz Nyquist band: about
        // −77 dBFS. THD+N is dominated by the harmonic here.
        assert!((a.noise_dbfs - crate::dbfs_rms(1e-4 * (20000.0f64 / 24000.0).sqrt())).abs() < 1.0, "noise {}", a.noise_dbfs);
        assert!(a.thd_n_percent >= a.thd_percent);
    }

    #[test]
    fn dc_offset_is_read_without_the_tone_leaking_in() {
        let sr = 48000.0;
        let mut x = sine(sr, 997.0, amplitude(-1.0), 3 * 48000, 0);
        for v in &mut x {
            *v += 0.001;
        }
        let a = analyse(&x, sr, 997.0, 5);
        assert!((a.dc_offset - 0.001).abs() < 1e-5, "dc {}", a.dc_offset);
        let clean = analyse(&sine(sr, 997.0, amplitude(-1.0), 3 * 48000, 0), sr, 997.0, 5);
        assert!(clean.dc_offset.abs() < 1e-5, "dc {}", clean.dc_offset);
    }

    #[test]
    fn clock_offset_reads_in_ppm() {
        let sr = 48000.0;
        // 50 ppm high.
        let x = sine(sr, 1000.0 * (1.0 + 50e-6), amplitude(-12.0), 4 * 48000, 0);
        let a = analyse(&x, sr, 1000.0, 5);
        assert!((a.frequency_error_ppm - 50.0).abs() < 10.0, "ppm {}", a.frequency_error_ppm);
    }

    #[test]
    fn silence_is_not_a_tone() {
        let a = analyse(&crate::signal::silence(48000), 48000.0, 1000.0, 5);
        assert!(!a.found);
    }
}
