//! Noise floor of the input, with silence at the output.
//!
//! Level is reported three ways: unweighted in the 20 Hz–20 kHz band (the
//! number a spec sheet calls "noise floor" or "dynamic range" when subtracted
//! from 0 dBFS), A-weighted, and as a peak. The spectrum is a Welch average of
//! 16384-point Blackman-Harris blocks, and the strongest narrow peaks above the
//! surrounding floor are listed separately: those are the hum, the USB packet
//! whine and the switch-mode supply that a single RMS number hides.

use serde::Serialize;

use crate::fft;
use crate::window::{blackman_harris, power_gain};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpectralPeak {
    pub hz: f64,
    /// Level of the peak in dBFS (as a sine of that level would read).
    pub level_dbfs: f64,
    /// How far it stands above the floor around it, in dB.
    pub prominence_db: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseAnalysis {
    /// RMS over the full captured bandwidth, dBFS.
    pub rms_dbfs: f64,
    /// RMS within 20 Hz–20 kHz, dBFS.
    pub band_rms_dbfs: f64,
    /// A-weighted RMS, dBFS(A).
    pub a_weighted_dbfs: f64,
    pub peak_dbfs: f64,
    pub dc_offset: f64,
    pub dc_offset_dbfs: f64,
    pub peaks: Vec<SpectralPeak>,
    pub spectrum_freqs: Vec<f64>,
    /// dBFS per bin at the analysis resolution, max-held onto the grid.
    pub spectrum_db: Vec<f64>,
    pub fft_size: usize,
    pub blocks: usize,
}

/// IEC 61672 A-weighting, as a power ratio at `f` Hz.
pub fn a_weight_power(f: f64) -> f64 {
    let f2 = f * f;
    let num = 12194.0f64.powi(2) * f2 * f2;
    let den = (f2 + 20.6f64.powi(2))
        * ((f2 + 107.7f64.powi(2)) * (f2 + 737.9f64.powi(2))).sqrt()
        * (f2 + 12194.0f64.powi(2));
    let ra = num / den;
    let db = 20.0 * ra.log10() + 2.0;
    10f64.powf(db / 10.0)
}

pub fn analyse(captured: &[f32], sr: f64) -> NoiseAnalysis {
    let n = 16384usize.min(fft::next_pow2(captured.len().max(2)) / 2).max(256);
    let dc = crate::mean(captured);
    let w = blackman_harris(n);
    let pg = power_gain(&w);
    let hop = n / 2;
    let mut acc = vec![0.0f64; n / 2 + 1];
    let mut blocks = 0usize;
    let mut start = 0usize;
    while start + n <= captured.len() {
        let x: Vec<f64> = captured[start..start + n].iter().zip(&w).map(|(&v, w)| (v as f64 - dc) * w).collect();
        let bins = fft::forward(&x, n);
        for (a, b) in acc.iter_mut().zip(&bins) {
            *a += b.norm_sqr();
        }
        blocks += 1;
        start += hop;
    }
    if blocks == 0 {
        // Too short for a block: report the time-domain numbers only.
        let r = crate::rms(captured);
        return NoiseAnalysis {
            rms_dbfs: crate::dbfs_rms(r),
            band_rms_dbfs: crate::dbfs_rms(r),
            a_weighted_dbfs: crate::dbfs_rms(r),
            peak_dbfs: crate::db_amp(crate::peak(captured)),
            dc_offset: dc,
            dc_offset_dbfs: crate::db_amp(dc.abs()),
            peaks: vec![],
            spectrum_freqs: vec![],
            spectrum_db: vec![],
            fft_size: n,
            blocks: 0,
        };
    }
    // Mean-square per bin.
    let ms: Vec<f64> = acc.iter().map(|a| 2.0 * a / (blocks as f64 * n as f64 * pg)).collect();
    let bin_hz = sr / n as f64;
    let band_lo = (20.0 / bin_hz).ceil() as usize;
    let band_hi = ((20000.0f64.min(0.45 * sr) / bin_hz).floor() as usize).min(ms.len() - 1);
    let band_ms: f64 = ms[band_lo..=band_hi].iter().sum();
    let a_ms: f64 = ms[1..]
        .iter()
        .enumerate()
        .map(|(i, v)| v * a_weight_power((i + 1) as f64 * bin_hz))
        .sum();

    // Peaks: bins more than 12 dB above the median of their ±20-bin
    // neighbourhood (the lobe itself excluded), strongest first, at most eight.
    let db: Vec<f64> = ms.iter().map(|&v| crate::db_power(v * 2.0)).collect();
    let mut peaks: Vec<SpectralPeak> = Vec::new();
    const HOOD: usize = 20;
    for k in band_lo.max(HOOD + 1)..band_hi.min(db.len().saturating_sub(HOOD + 1)) {
        if db[k] < db[k - 1] || db[k] < db[k + 1] {
            continue;
        }
        let mut hood: Vec<f64> = db[k - HOOD..k - 5].iter().chain(db[k + 6..k + HOOD + 1].iter()).copied().collect();
        hood.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let floor = hood[hood.len() / 2];
        let prominence = db[k] - floor;
        if prominence > 12.0 {
            // Sum the lobe so a tone reads its true level rather than its peak bin's.
            let lobe: f64 = ms[k - 5..=k + 5].iter().sum();
            peaks.push(SpectralPeak { hz: k as f64 * bin_hz, level_dbfs: crate::db_power(lobe * 2.0), prominence_db: prominence });
        }
    }
    peaks.sort_by(|a, b| b.level_dbfs.partial_cmp(&a.level_dbfs).unwrap());
    peaks.truncate(8);

    let grid = crate::response::log_grid((bin_hz * 2.0).max(10.0), 0.5 * sr, 96);
    let half = 2f64.powf(1.0 / 192.0);
    let spectrum_db: Vec<f64> = grid
        .iter()
        .map(|&f| {
            let a = ((f / half / bin_hz).round() as usize).clamp(1, ms.len() - 1);
            let b = ((f * half / bin_hz).round() as usize).clamp(a, ms.len() - 1);
            let best = ms[a..=b].iter().cloned().fold(0.0, f64::max);
            crate::db_power(best * 2.0)
        })
        .collect();

    NoiseAnalysis {
        rms_dbfs: crate::dbfs_rms(crate::rms(captured)),
        band_rms_dbfs: crate::dbfs_rms(band_ms.sqrt()),
        a_weighted_dbfs: crate::dbfs_rms(a_ms.sqrt()),
        peak_dbfs: crate::db_amp(crate::peak(captured)),
        dc_offset: dc,
        dc_offset_dbfs: crate::db_amp(dc.abs()),
        peaks,
        spectrum_freqs: grid,
        spectrum_db,
        fft_size: n,
        blocks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::{amplitude, sine, white_noise};

    #[test]
    fn white_noise_level_is_read() {
        let sr = 48000.0;
        let x = white_noise(48000 * 4, 1e-3, 11);
        let a = analyse(&x, sr);
        let expect = crate::dbfs_rms(1e-3);
        assert!((a.rms_dbfs - expect).abs() < 0.2, "rms {} expected {expect}", a.rms_dbfs);
        // 20 kHz of a 24 kHz band: 0.8 dB less.
        assert!((a.band_rms_dbfs - (expect - 0.79)).abs() < 0.4, "band {}", a.band_rms_dbfs);
        assert!(a.a_weighted_dbfs < a.band_rms_dbfs);
        assert!(a.peaks.is_empty(), "spurious peaks: {:?}", a.peaks);
        assert!(a.blocks > 5);
    }

    #[test]
    fn a_hum_is_listed_as_a_peak() {
        let sr = 48000.0;
        let mut x = white_noise(48000 * 4, 1e-4, 5);
        let hum = sine(sr, 100.0, amplitude(-60.0), x.len(), 0);
        for (a, b) in x.iter_mut().zip(&hum) {
            *a += b;
        }
        let a = analyse(&x, sr);
        assert!(!a.peaks.is_empty());
        let p = &a.peaks[0];
        assert!((p.hz - 100.0).abs() < 4.0, "peak at {}", p.hz);
        assert!((p.level_dbfs + 60.0).abs() < 1.0, "peak level {}", p.level_dbfs);
        assert!(p.prominence_db > 12.0);
    }

    #[test]
    fn a_weighting_is_zero_at_one_k() {
        let db = 10.0 * a_weight_power(1000.0).log10();
        assert!(db.abs() < 0.05, "{db}");
        let low = 10.0 * a_weight_power(100.0).log10();
        assert!((low + 19.1).abs() < 0.3, "{low}");
    }
}
