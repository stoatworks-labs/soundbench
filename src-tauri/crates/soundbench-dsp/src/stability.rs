//! Glitch detection in a captured sine, and callback timing statistics.
//!
//! A buffer underrun on the output plays as a gap or a repeat; an overrun on
//! the input records one. Either way the recorded sine stops being a sine for
//! a moment. The detector fits a sine — amplitude and phase, at the known
//! frequency — to a sliding window of about four cycles and looks at what is
//! left over. A clean path leaves a small, steady residual (the device's own
//! distortion and noise); a discontinuity leaves a spike many times that
//! size; a dropout leaves nothing at all, which shows as the fitted amplitude
//! collapsing. Both are reported as events with a time and a duration.
//!
//! The judgement is relative to the recording's own median residual, so a
//! device with 0.1 % distortion is not reported as glitching continuously —
//! only a change is a glitch.

use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum GlitchKind {
    /// The signal stayed present but its phase or level jumped.
    Discontinuity,
    /// The signal disappeared for the duration.
    Dropout,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlitchEvent {
    pub kind: GlitchKind,
    /// Seconds from the start of the analysed region.
    pub at_s: f64,
    pub duration_ms: f64,
    /// Peak residual during the event, dB relative to the sine's amplitude.
    pub severity_db: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlitchReport {
    pub events: Vec<GlitchEvent>,
    pub discontinuities: usize,
    pub dropouts: usize,
    /// Median residual, dB relative to the sine's amplitude — the path's own
    /// noise and distortion.
    pub residual_median_db: f64,
    pub residual_max_db: f64,
    /// Runs of 32 or more exactly-zero samples.
    pub zero_runs: usize,
    pub amplitude_dbfs: f64,
    pub analysed_s: f64,
}

/// Find glitches in `captured`, a sine at `freq` Hz.
pub fn find_glitches(captured: &[f32], sr: f64, freq: f64) -> GlitchReport {
    let cycle = sr / freq;
    let w = ((4.0 * cycle).round() as usize).max(64).min(captured.len().max(1));
    let hop = (w / 4).max(1);
    let omega = 2.0 * std::f64::consts::PI * freq / sr;

    let mut residuals: Vec<f64> = Vec::new();
    let mut amps: Vec<f64> = Vec::new();
    let mut starts: Vec<usize> = Vec::new();
    let mut i = 0usize;
    while i + w <= captured.len() {
        let (mut ss, mut sc, mut cc, mut xs, mut xc) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for (n, &x) in captured[i..i + w].iter().enumerate() {
            let (s, c) = (omega * (i + n) as f64).sin_cos();
            let x = x as f64;
            ss += s * s;
            sc += s * c;
            cc += c * c;
            xs += x * s;
            xc += x * c;
        }
        let det = ss * cc - sc * sc;
        let (a, b) = if det.abs() > 1e-12 { ((xs * cc - xc * sc) / det, (ss * xc - sc * xs) / det) } else { (0.0, 0.0) };
        // Residual over the centre hop only, where the fit is best.
        let c0 = i + (w - hop) / 2;
        let mut r = 0.0f64;
        for n in c0..c0 + hop {
            let (s, c) = (omega * n as f64).sin_cos();
            let e = captured[n] as f64 - (a * s + b * c);
            r += e * e;
        }
        residuals.push((r / hop as f64).sqrt());
        amps.push((a * a + b * b).sqrt());
        starts.push(c0);
        i += hop;
    }

    let mut sorted = residuals.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let res_median = sorted.get(sorted.len() / 2).copied().unwrap_or(0.0);
    let mut sorted_amp = amps.clone();
    sorted_amp.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let amp_median = sorted_amp.get(sorted_amp.len() / 2).copied().unwrap_or(0.0);

    // A window is a discontinuity if its residual is far above the median and
    // above −40 dB of the sine; a dropout if the sine itself has gone.
    let disc_floor = (res_median * 6.0).max(amp_median * 0.01);
    let flagged: Vec<Option<GlitchKind>> = residuals
        .iter()
        .zip(&amps)
        .map(|(&r, &a)| {
            if amp_median > 0.0 && a < 0.5 * amp_median {
                Some(GlitchKind::Dropout)
            } else if r > disc_floor {
                Some(GlitchKind::Discontinuity)
            } else {
                None
            }
        })
        .collect();

    let mut events = Vec::new();
    let mut k = 0usize;
    while k < flagged.len() {
        if let Some(kind) = &flagged[k] {
            let mut end = k;
            let mut worst = residuals[k];
            let mut kind = kind.clone();
            while end + 1 < flagged.len() && flagged[end + 1].is_some() {
                end += 1;
                worst = worst.max(residuals[end]);
                if flagged[end] == Some(GlitchKind::Dropout) {
                    kind = GlitchKind::Dropout;
                }
            }
            let start_s = starts[k] as f64 / sr;
            let dur = (starts[end] + hop - starts[k]) as f64 / sr * 1000.0;
            let severity = if amp_median > 0.0 { crate::db_amp(worst / amp_median) } else { 0.0 };
            events.push(GlitchEvent { kind, at_s: start_s, duration_ms: dur, severity_db: severity });
            k = end + 1;
        } else {
            k += 1;
        }
    }

    // Exact-zero runs.
    let mut zero_runs = 0usize;
    let mut run = 0usize;
    for &x in captured {
        if x == 0.0 {
            run += 1;
            if run == 32 {
                zero_runs += 1;
            }
        } else {
            run = 0;
        }
    }

    let res_max = sorted.last().copied().unwrap_or(0.0);
    GlitchReport {
        discontinuities: events.iter().filter(|e| e.kind == GlitchKind::Discontinuity).count(),
        dropouts: events.iter().filter(|e| e.kind == GlitchKind::Dropout).count(),
        events,
        residual_median_db: if amp_median > 0.0 { crate::db_amp(res_median / amp_median) } else { -200.0 },
        residual_max_db: if amp_median > 0.0 { crate::db_amp(res_max / amp_median) } else { -200.0 },
        zero_runs,
        amplitude_dbfs: crate::db_amp(amp_median),
        analysed_s: captured.len() as f64 / sr,
    }
}

/// What the audio callback's own bookkeeping says about a run.
#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CallbackStats {
    pub callbacks: usize,
    pub input_overflows: usize,
    pub input_underflows: usize,
    pub output_underflows: usize,
    pub output_overflows: usize,
    /// Expected time between callbacks for the buffer size, ms.
    pub expected_interval_ms: f64,
    pub mean_interval_ms: f64,
    pub max_interval_ms: f64,
    /// Callbacks that arrived more than 1.5× the expected interval after the
    /// previous one.
    pub late_callbacks: usize,
    /// Largest number of frames delivered in one callback — hosts that do
    /// not honour the requested size show it here.
    pub max_frames: usize,
    pub min_frames: usize,
    pub cpu_load: f64,
}

/// Build the statistics from per-callback records: `(frames, interval_s, flags)`
/// where `flags` are PortAudio's status bits (input underflow 1, input
/// overflow 2, output underflow 4, output overflow 8).
pub fn callback_stats(records: &[(usize, f64, u32)], buffer_frames: usize, sr: f64, cpu_load: f64) -> CallbackStats {
    let expected = buffer_frames as f64 / sr;
    let mut s = CallbackStats {
        callbacks: records.len(),
        expected_interval_ms: expected * 1000.0,
        cpu_load,
        min_frames: usize::MAX,
        ..Default::default()
    };
    let mut sum = 0.0;
    let mut counted = 0usize;
    for (i, &(frames, interval, flags)) in records.iter().enumerate() {
        if flags & 1 != 0 {
            s.input_underflows += 1;
        }
        if flags & 2 != 0 {
            s.input_overflows += 1;
        }
        if flags & 4 != 0 {
            s.output_underflows += 1;
        }
        if flags & 8 != 0 {
            s.output_overflows += 1;
        }
        s.max_frames = s.max_frames.max(frames);
        s.min_frames = s.min_frames.min(frames);
        // The first interval is measured from stream start, not from a
        // previous callback, so it is not a callback interval.
        if i > 0 {
            sum += interval;
            counted += 1;
            s.max_interval_ms = s.max_interval_ms.max(interval * 1000.0);
            if expected > 0.0 && interval > 1.5 * expected * (frames as f64 / buffer_frames.max(1) as f64).max(1.0) {
                s.late_callbacks += 1;
            }
        }
    }
    if s.min_frames == usize::MAX {
        s.min_frames = 0;
    }
    s.mean_interval_ms = if counted > 0 { sum / counted as f64 * 1000.0 } else { 0.0 };
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::{amplitude, sine};
    use crate::testutil::Loopback;

    #[test]
    fn a_clean_sine_has_no_glitches() {
        let sr = 48000.0;
        let x = sine(sr, 1000.0, amplitude(-6.0), 48000 * 3, 0);
        let lb = Loopback { cubic: 0.05, noise_rms: 1e-4, ..Default::default() };
        let y = lb.run(&x, sr, x.len());
        let r = find_glitches(&y, sr, 1000.0);
        assert!(r.events.is_empty(), "{:?}", r.events);
        assert_eq!(r.zero_runs, 0);
        assert!((r.amplitude_dbfs + 6.0).abs() < 0.2, "amp {}", r.amplitude_dbfs);
        assert!(r.residual_median_db < -40.0);
    }

    #[test]
    fn a_dropped_buffer_and_a_gap_are_found() {
        let sr = 48000.0;
        let mut y = sine(sr, 1000.0, amplitude(-6.0), 48000 * 3, 0);
        // Drop 128 samples at 1.0 s (a repeat/skip: splice the buffer).
        y.drain(48000..48000 + 128);
        // Silence 256 samples at 2.0 s.
        for v in &mut y[96000..96000 + 256] {
            *v = 0.0;
        }
        let r = find_glitches(&y, sr, 1000.0);
        assert_eq!(r.discontinuities, 1, "{:?}", r.events);
        assert_eq!(r.dropouts, 1, "{:?}", r.events);
        assert_eq!(r.zero_runs, 1);
        let d = r.events.iter().find(|e| e.kind == GlitchKind::Discontinuity).unwrap();
        assert!((d.at_s - 1.0).abs() < 0.01, "at {}", d.at_s);
        let g = r.events.iter().find(|e| e.kind == GlitchKind::Dropout).unwrap();
        assert!((g.at_s - 2.0).abs() < 0.01, "at {}", g.at_s);
        assert!(g.duration_ms >= 4.0 && g.duration_ms < 20.0, "dur {}", g.duration_ms);
    }

    #[test]
    fn a_clock_offset_is_not_a_glitch() {
        let sr = 48000.0;
        let y = sine(sr, 1000.0 * (1.0 + 100e-6), amplitude(-6.0), 48000 * 5, 0);
        let r = find_glitches(&y, sr, 1000.0);
        assert!(r.events.is_empty(), "{:?}", r.events);
    }

    #[test]
    fn callback_stats_count_flags_and_late_callbacks() {
        let recs = vec![(256, 0.0, 0), (256, 0.00533, 0), (256, 0.00533, 4), (256, 0.02, 2), (128, 0.0027, 0)];
        let s = callback_stats(&recs, 256, 48000.0, 0.1);
        assert_eq!(s.callbacks, 5);
        assert_eq!(s.output_underflows, 1);
        assert_eq!(s.input_overflows, 1);
        assert_eq!(s.late_callbacks, 1);
        assert_eq!(s.max_frames, 256);
        assert_eq!(s.min_frames, 128);
        assert!((s.expected_interval_ms - 5.333).abs() < 0.01);
        assert!((s.max_interval_ms - 20.0).abs() < 1e-9);
    }
}
