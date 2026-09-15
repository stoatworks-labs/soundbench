//! Round-trip latency by cross-correlation.
//!
//! The device side plays [`signal::latency_burst`](crate::signal::latency_burst)
//! several times at known frame positions and records what comes back on the
//! same frame clock. For each burst this finds where the template's
//! cross-correlation with the recording peaks; the distance from where the
//! burst was played is the round-trip delay — DAC, cable, ADC and every buffer
//! in between, in samples of the stream's own clock, which is the one a DAW
//! would have to compensate.
//!
//! The peak is refined by parabolic interpolation to a fraction of a sample,
//! and its sign gives the polarity of the path. Several bursts give a median
//! and a spread; a spread of more than a sample or two means the device's
//! latency is not stable between buffers, which is itself a finding.

use serde::Serialize;

use crate::fft;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BurstResult {
    /// Frame at which the burst was played.
    pub played_at: usize,
    /// Round-trip delay in samples, fractional.
    pub delay_samples: f64,
    /// Peak-to-RMS ratio of the correlation in the search window. Below
    /// [`MIN_CONFIDENCE`] the burst was not found.
    pub confidence: f64,
    pub found: bool,
    pub inverted: bool,
    /// Peak level of the received burst in dBFS.
    pub level_dbfs: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyResult {
    pub bursts: Vec<BurstResult>,
    pub found: usize,
    /// Median over the bursts that were found.
    pub median_samples: f64,
    pub median_ms: f64,
    pub min_samples: f64,
    pub max_samples: f64,
    /// max − min, in samples: the jitter between bursts.
    pub spread_samples: f64,
    pub inverted: bool,
}

/// Peak-to-RMS ratio below which a correlation peak is treated as noise.
pub const MIN_CONFIDENCE: f64 = 8.0;

/// Locate each burst in `captured`.
///
/// `played_at` are the frames at which the template was written to the
/// output; `max_delay` bounds the search (in samples) after each one.
pub fn measure(
    captured: &[f32],
    template: &[f32],
    played_at: &[usize],
    max_delay: usize,
    sr: f64,
) -> LatencyResult {
    let t: Vec<f64> = template.iter().map(|&v| v as f64).collect();
    let mut bursts = Vec::with_capacity(played_at.len());

    for &start in played_at {
        let end = (start + max_delay + template.len()).min(captured.len());
        if start >= captured.len() || end <= start + template.len() {
            bursts.push(BurstResult {
                played_at: start,
                delay_samples: 0.0,
                confidence: 0.0,
                found: false,
                inverted: false,
                level_dbfs: -200.0,
            });
            continue;
        }
        let seg: Vec<f64> = captured[start..end].iter().map(|&v| v as f64).collect();
        let c = fft::xcorr(&seg, &t);
        // Only lags where the whole template fits are meaningful.
        let usable = seg.len().saturating_sub(t.len()).max(1).min(c.len());
        let c = &c[..usable];
        let (k, pk) = c
            .iter()
            .enumerate()
            .fold((0usize, 0.0f64), |m, (i, &v)| if v.abs() > m.1 { (i, v.abs()) } else { m });
        let rms = (c.iter().map(|v| v * v).sum::<f64>() / c.len() as f64).sqrt();
        let confidence = if rms > 0.0 { pk / rms } else { 0.0 };
        let inverted = c[k] < 0.0;
        let frac = if k > 0 && k + 1 < c.len() {
            parabolic_offset(c[k - 1].abs(), c[k].abs(), c[k + 1].abs())
        } else {
            0.0
        };
        let level = crate::peak(&captured[start + k..(start + k + t.len()).min(captured.len())]);
        bursts.push(BurstResult {
            played_at: start,
            delay_samples: k as f64 + frac,
            confidence,
            found: confidence >= MIN_CONFIDENCE,
            inverted,
            level_dbfs: crate::db_amp(level),
        });
    }

    let mut delays: Vec<f64> = bursts.iter().filter(|b| b.found).map(|b| b.delay_samples).collect();
    delays.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let found = delays.len();
    let median = if found == 0 {
        0.0
    } else if found % 2 == 1 {
        delays[found / 2]
    } else {
        0.5 * (delays[found / 2 - 1] + delays[found / 2])
    };
    let min = delays.first().copied().unwrap_or(0.0);
    let max = delays.last().copied().unwrap_or(0.0);
    let inverted_votes = bursts.iter().filter(|b| b.found && b.inverted).count();

    LatencyResult {
        found,
        median_samples: median,
        median_ms: median / sr * 1000.0,
        min_samples: min,
        max_samples: max,
        spread_samples: max - min,
        inverted: found > 0 && inverted_votes * 2 > found,
        bursts,
    }
}

/// Offset of the true peak from the centre sample, given the three samples
/// around a discrete maximum, by fitting a parabola.
pub fn parabolic_offset(left: f64, centre: f64, right: f64) -> f64 {
    let denom = left - 2.0 * centre + right;
    if denom.abs() < 1e-12 {
        0.0
    } else {
        (0.5 * (left - right) / denom).clamp(-0.5, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::latency_burst;
    use crate::testutil::Loopback;

    fn play(sr: f64, starts: &[usize], total: usize) -> (Vec<f32>, Vec<f32>) {
        let burst = latency_burst(sr, 0.04, 0.5);
        let mut out = vec![0.0f32; total];
        for &s in starts {
            out[s..s + burst.len()].copy_from_slice(&burst);
        }
        (burst, out)
    }

    #[test]
    fn finds_an_exact_delay_through_a_noisy_filtered_path() {
        let sr = 48000.0;
        let starts = [4800, 24000, 43200];
        let (burst, out) = play(sr, &starts, 96000);
        let lb = Loopback { delay: 1234, gain: 0.3, lowpass_hz: 12000.0, noise_rms: 0.02, ..Default::default() };
        let cap = lb.run(&out, sr, out.len());
        let r = measure(&cap, &burst, &starts, 12000, sr);
        assert_eq!(r.found, 3, "{:?}", r.bursts);
        // The one-pole low-pass adds a fraction of a sample of group delay; the
        // integer part must be exact and the fraction small.
        assert!((r.median_samples - 1234.0).abs() < 1.0, "median {}", r.median_samples);
        assert!(r.spread_samples < 0.5, "spread {}", r.spread_samples);
        assert!(!r.inverted);
        assert!((r.median_ms - 1234.0 / 48.0).abs() < 0.05);
    }

    #[test]
    fn detects_polarity_inversion() {
        let sr = 48000.0;
        let starts = [1000, 20000];
        let (burst, out) = play(sr, &starts, 48000);
        let lb = Loopback { delay: 100, invert: true, ..Default::default() };
        let cap = lb.run(&out, sr, out.len());
        let r = measure(&cap, &burst, &starts, 10000, sr);
        assert_eq!(r.found, 2);
        assert!(r.inverted);
        assert!((r.median_samples - 100.0).abs() < 0.05);
    }

    #[test]
    fn reports_nothing_found_on_silence() {
        let sr = 48000.0;
        let burst = latency_burst(sr, 0.04, 0.5);
        let cap = crate::signal::white_noise(48000, 0.001, 3);
        let r = measure(&cap, &burst, &[1000, 20000], 10000, sr);
        assert_eq!(r.found, 0);
        assert!(r.bursts.iter().all(|b| !b.found));
    }

    #[test]
    fn parabola_centres() {
        assert_eq!(parabolic_offset(1.0, 2.0, 1.0), 0.0);
        assert!(parabolic_offset(1.0, 2.0, 1.5) > 0.0);
        assert!(parabolic_offset(1.5, 2.0, 1.0) < 0.0);
    }
}
