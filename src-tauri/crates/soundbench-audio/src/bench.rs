//! The tests, each one stream pass plus one analysis.
//!
//! Every test takes a [`Target`] — which device, which channels, which rate
//! and buffer — builds a signal with `soundbench-dsp::signal`, runs it
//! through [`stream::run`], and hands the recording to the matching
//! analysis. The reports carry both the measurement and what the stream
//! said about itself (`StreamFacts`), because the interesting number is
//! often the difference: the latency a driver *reports* against the one
//! that was *measured* is what a DAW's compensation will get wrong.
//!
//! Signals are played from a short pre-roll so the device is clocking
//! steadily before anything that matters starts, and every recording runs
//! on past the end of the signal by at least the longest round trip the
//! test is prepared to find.

use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};
use soundbench_dsp::signal;
use soundbench_dsp::{latency, noise, response, stability, tone, transfer};

use crate::stream::{self, Job, Outcome, SideConfig, StreamConfig, StreamFacts};
use crate::{AudioError, Engine, Result};

/// Which device and channels a test runs on.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub input_device: i32,
    pub output_device: i32,
    /// Input channels to record (0-based).
    pub input_channels: Vec<usize>,
    /// Output channels that carry the signal (0-based).
    pub output_channels: Vec<usize>,
    pub sample_rate: f64,
    pub buffer_frames: u32,
    #[serde(default)]
    pub wasapi_exclusive: bool,
    #[serde(default = "yes")]
    pub mac_change_device: bool,
}

fn yes() -> bool {
    true
}

impl Target {
    pub fn stream_config(&self, buffer_frames: Option<u32>) -> Result<StreamConfig> {
        if self.input_channels.is_empty() {
            return Err(AudioError::Config("pick at least one input channel".into()));
        }
        if self.output_channels.is_empty() {
            return Err(AudioError::Config("pick at least one output channel".into()));
        }
        Ok(StreamConfig {
            input: Some(SideConfig { device: self.input_device, channels: self.input_channels.iter().max().unwrap() + 1 }),
            output: Some(SideConfig { device: self.output_device, channels: self.output_channels.iter().max().unwrap() + 1 }),
            sample_rate: self.sample_rate,
            buffer_frames: buffer_frames.unwrap_or(self.buffer_frames),
            suggested_latency_s: None,
            wasapi_exclusive: self.wasapi_exclusive,
            mac_change_device: self.mac_change_device,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub phase: String,
    pub fraction: f64,
    pub message: String,
}

pub struct Progress<'a> {
    pub cancel: &'a AtomicBool,
    pub report: &'a mut dyn FnMut(ProgressEvent),
}

impl Progress<'_> {
    fn send(&mut self, phase: &str, fraction: f64, message: &str) {
        (self.report)(ProgressEvent { phase: phase.into(), fraction, message: message.into() });
    }
}

/// Play `signal` from `pre_roll` frames in, record for `total` frames.
fn pass(engine: &Engine, target: &Target, buffer: Option<u32>, signal: Vec<f32>, pre_roll: usize, total: usize, progress: &mut Progress, phase: &str) -> Result<Outcome> {
    let cfg = target.stream_config(buffer)?;
    let mut full = vec![0.0f32; pre_roll];
    full.extend_from_slice(&signal);
    let job = Job { signal: full, out_channels: target.output_channels.clone(), in_channels: target.input_channels.clone(), total_frames: total };
    let cancel = progress.cancel;
    let report = &mut *progress.report;
    let phase_owned = phase.to_string();
    let mut on_progress = |f: f64| {
        report(ProgressEvent { phase: phase_owned.clone(), fraction: f, message: String::new() });
    };
    stream::run(engine, &cfg, job, cancel, &mut on_progress)
}

/// Where a signal that started at about `expected` frames actually became
/// audible in a recording, and the steady stretch after it.
fn steady_region(captured: &[f32], sr: f64, expected: usize, duration: usize) -> std::ops::Range<usize> {
    let block = ((0.01 * sr) as usize).max(16);
    let n = captured.len() / block;
    let mut env = Vec::with_capacity(n);
    for b in 0..n {
        env.push(soundbench_dsp::rms(&captured[b * block..(b + 1) * block]));
    }
    let peak = env.iter().cloned().fold(0.0, f64::max);
    let thresh = peak * 0.01; // −40 dB
    let start_block = (expected / block).min(n.saturating_sub(1));
    let onset = env.iter().enumerate().skip(start_block).find(|(_, &v)| v >= thresh).map(|(i, _)| i * block).unwrap_or(expected);
    let settle = (0.25 * sr) as usize;
    let a = (onset + settle).min(captured.len());
    let b = (onset + duration).saturating_sub((0.1 * sr) as usize).min(captured.len());
    if b <= a {
        a..captured.len()
    } else {
        a..b
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelReport<T> {
    pub input_channel: usize,
    pub analysis: T,
}

// ------------------------------------------------------------- latency

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyOptions {
    #[serde(default = "LatencyOptions::default_bursts")]
    pub bursts: usize,
    #[serde(default = "LatencyOptions::default_max_delay")]
    pub max_delay_s: f64,
    #[serde(default = "LatencyOptions::default_level")]
    pub level_dbfs: f64,
}

impl LatencyOptions {
    fn default_bursts() -> usize {
        5
    }
    fn default_max_delay() -> f64 {
        1.0
    }
    fn default_level() -> f64 {
        -12.0
    }
}

impl Default for LatencyOptions {
    fn default() -> Self {
        LatencyOptions { bursts: 5, max_delay_s: 1.0, level_dbfs: -12.0 }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyReport {
    pub facts: StreamFacts,
    pub channels: Vec<ChannelReport<latency::LatencyResult>>,
    /// Input plus output latency as PortAudio reported them, ms.
    pub reported_total_ms: f64,
    /// Measured (first channel's median) minus reported, ms. Positive means
    /// the device has latency its driver does not declare.
    pub unreported_ms: Option<f64>,
    pub measured_ms: Option<f64>,
    pub measured_samples: Option<f64>,
}

pub fn latency_test(engine: &Engine, target: &Target, opts: &LatencyOptions, progress: &mut Progress) -> Result<LatencyReport> {
    let sr = target.sample_rate;
    let burst = signal::latency_burst(sr, 0.04, signal::amplitude(opts.level_dbfs));
    let max_delay = (opts.max_delay_s * sr) as usize;
    let spacing = max_delay + burst.len() + (0.1 * sr) as usize;
    let pre_roll = (0.5 * sr) as usize;
    let n = opts.bursts.clamp(1, 50);
    let mut sig = vec![0.0f32; n * spacing];
    let mut played_at = Vec::with_capacity(n);
    for i in 0..n {
        let at = i * spacing;
        sig[at..at + burst.len()].copy_from_slice(&burst);
        played_at.push(pre_roll + at);
    }
    let total = pre_roll + sig.len() + max_delay + (0.3 * sr) as usize;
    progress.send("latency", 0.0, &format!("{n} bursts, listening up to {:.0} ms", opts.max_delay_s * 1000.0));
    let out = pass(engine, target, None, sig, pre_roll, total, progress, "latency")?;
    let channels: Vec<_> = out
        .captured
        .iter()
        .zip(&target.input_channels)
        .map(|(c, &ch)| ChannelReport { input_channel: ch, analysis: latency::measure(c, &burst, &played_at, max_delay, sr) })
        .collect();
    let reported = out.facts.input_latency_ms + out.facts.output_latency_ms;
    let first = channels.first().filter(|c| c.analysis.found > 0);
    Ok(LatencyReport {
        reported_total_ms: reported,
        unreported_ms: first.map(|c| c.analysis.median_ms - reported),
        measured_ms: first.map(|c| c.analysis.median_ms),
        measured_samples: first.map(|c| c.analysis.median_samples),
        facts: out.facts,
        channels,
    })
}

// ------------------------------------------------------------- sweep

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepOptions {
    #[serde(default = "SweepOptions::d_f1")]
    pub f1: f64,
    #[serde(default = "SweepOptions::d_f2")]
    pub f2: f64,
    #[serde(default = "SweepOptions::d_dur")]
    pub duration_s: f64,
    #[serde(default = "SweepOptions::d_level")]
    pub level_dbfs: f64,
    #[serde(default = "SweepOptions::d_max_delay")]
    pub max_delay_s: f64,
}

impl SweepOptions {
    fn d_f1() -> f64 {
        10.0
    }
    fn d_f2() -> f64 {
        22000.0
    }
    fn d_dur() -> f64 {
        10.0
    }
    fn d_level() -> f64 {
        -12.0
    }
    fn d_max_delay() -> f64 {
        1.0
    }
}

impl Default for SweepOptions {
    fn default() -> Self {
        SweepOptions { f1: 10.0, f2: 22000.0, duration_s: 10.0, level_dbfs: -12.0, max_delay_s: 1.0 }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepReport {
    pub facts: StreamFacts,
    pub spec: SweepSpecOut,
    pub channels: Vec<ChannelReport<response::SweepAnalysis>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepSpecOut {
    pub f1: f64,
    pub f2: f64,
    pub duration_s: f64,
    pub level_dbfs: f64,
}

pub fn sweep_test(engine: &Engine, target: &Target, opts: &SweepOptions, progress: &mut Progress) -> Result<SweepReport> {
    let sr = target.sample_rate;
    let f2 = opts.f2.min(0.48 * sr);
    let f1 = opts.f1.max(5.0).min(f2 / 4.0);
    let spec = signal::SweepSpec { sr, f1, f2, duration_s: opts.duration_s.clamp(1.0, 60.0), amp: signal::amplitude(opts.level_dbfs), fade_s: 0.01 };
    let sig = spec.samples();
    let pre_roll = (0.5 * sr) as usize;
    let max_delay = (opts.max_delay_s * sr) as usize;
    let total = pre_roll + sig.len() + max_delay + sr as usize;
    progress.send("sweep", 0.0, &format!("{f1:.0} Hz → {f2:.0} Hz over {:.0} s", spec.duration_s));
    let out = pass(engine, target, None, sig, pre_roll, total, progress, "sweep")?;
    progress.send("sweep", 1.0, "deconvolving");
    let ropts = response::SweepOptions { max_delay, ..Default::default() };
    let channels: Vec<_> = out
        .captured
        .iter()
        .zip(&target.input_channels)
        .map(|(c, &ch)| ChannelReport { input_channel: ch, analysis: response::analyse(&spec, c, pre_roll, &ropts) })
        .collect();
    Ok(SweepReport {
        facts: out.facts,
        spec: SweepSpecOut { f1, f2, duration_s: spec.duration_s, level_dbfs: opts.level_dbfs },
        channels,
    })
}

// ------------------------------------------------------------- tone

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToneOptions {
    #[serde(default = "ToneOptions::d_freq")]
    pub frequency: f64,
    #[serde(default = "ToneOptions::d_dur")]
    pub duration_s: f64,
    #[serde(default = "ToneOptions::d_level")]
    pub level_dbfs: f64,
}

impl ToneOptions {
    fn d_freq() -> f64 {
        997.0
    }
    fn d_dur() -> f64 {
        4.0
    }
    fn d_level() -> f64 {
        -1.0
    }
}

impl Default for ToneOptions {
    fn default() -> Self {
        ToneOptions { frequency: 997.0, duration_s: 4.0, level_dbfs: -1.0 }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToneReport {
    pub facts: StreamFacts,
    pub frequency: f64,
    pub level_dbfs: f64,
    pub channels: Vec<ChannelReport<tone::ToneAnalysis>>,
}

pub fn tone_test(engine: &Engine, target: &Target, opts: &ToneOptions, progress: &mut Progress) -> Result<ToneReport> {
    let sr = target.sample_rate;
    let dur = (opts.duration_s.clamp(1.0, 60.0) * sr) as usize;
    let f = opts.frequency.clamp(10.0, 0.45 * sr);
    let sig = signal::sine(sr, f, signal::amplitude(opts.level_dbfs.min(0.0)), dur, (0.01 * sr) as usize);
    let pre_roll = (0.5 * sr) as usize;
    let total = pre_roll + dur + (1.5 * sr) as usize;
    progress.send("tone", 0.0, &format!("{f:.0} Hz at {:.0} dBFS", opts.level_dbfs));
    let out = pass(engine, target, None, sig, pre_roll, total, progress, "tone")?;
    let channels: Vec<_> = out
        .captured
        .iter()
        .zip(&target.input_channels)
        .map(|(c, &ch)| {
            let r = steady_region(c, sr, pre_roll, dur);
            ChannelReport { input_channel: ch, analysis: tone::analyse(&c[r], sr, f, 9) }
        })
        .collect();
    Ok(ToneReport { facts: out.facts, frequency: f, level_dbfs: opts.level_dbfs, channels })
}

// ------------------------------------------------------------- noise floor

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseOptions {
    #[serde(default = "NoiseOptions::d_dur")]
    pub duration_s: f64,
}

impl NoiseOptions {
    fn d_dur() -> f64 {
        4.0
    }
}

impl Default for NoiseOptions {
    fn default() -> Self {
        NoiseOptions { duration_s: 4.0 }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseReport {
    pub facts: StreamFacts,
    pub channels: Vec<ChannelReport<noise::NoiseAnalysis>>,
}

pub fn noise_test(engine: &Engine, target: &Target, opts: &NoiseOptions, progress: &mut Progress) -> Result<NoiseReport> {
    let sr = target.sample_rate;
    let dur = (opts.duration_s.clamp(1.0, 60.0) * sr) as usize;
    let pre_roll = (0.5 * sr) as usize;
    let total = pre_roll + dur;
    progress.send("noise", 0.0, "silence out, listening");
    let out = pass(engine, target, None, signal::silence(dur), pre_roll, total, progress, "noise")?;
    let channels: Vec<_> = out
        .captured
        .iter()
        .zip(&target.input_channels)
        .map(|(c, &ch)| ChannelReport { input_channel: ch, analysis: noise::analyse(&c[pre_roll.min(c.len())..], sr) })
        .collect();
    Ok(NoiseReport { facts: out.facts, channels })
}

// ------------------------------------------------------------- noise transfer

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferOptions {
    #[serde(default = "TransferOptions::d_dur")]
    pub duration_s: f64,
    #[serde(default = "TransferOptions::d_level")]
    pub level_dbfs: f64,
    /// `pink` or `white`.
    #[serde(default = "TransferOptions::d_colour")]
    pub colour: String,
    #[serde(default = "TransferOptions::d_max_delay")]
    pub max_delay_s: f64,
}

impl TransferOptions {
    fn d_dur() -> f64 {
        8.0
    }
    fn d_level() -> f64 {
        -20.0
    }
    fn d_colour() -> String {
        "pink".into()
    }
    fn d_max_delay() -> f64 {
        1.0
    }
}

impl Default for TransferOptions {
    fn default() -> Self {
        TransferOptions { duration_s: 8.0, level_dbfs: -20.0, colour: "pink".into(), max_delay_s: 1.0 }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferReport {
    pub facts: StreamFacts,
    pub colour: String,
    pub level_dbfs: f64,
    pub channels: Vec<ChannelReport<transfer::TransferAnalysis>>,
}

pub fn transfer_test(engine: &Engine, target: &Target, opts: &TransferOptions, progress: &mut Progress) -> Result<TransferReport> {
    let sr = target.sample_rate;
    let dur = (opts.duration_s.clamp(2.0, 60.0) * sr) as usize;
    // Level is RMS; pink noise peaks about 12 dB above it, so cap the RMS at
    // −14 dBFS to stay clear of clipping.
    let rms = signal::amplitude(opts.level_dbfs.min(-14.0)) / std::f64::consts::SQRT_2;
    let mut sig = if opts.colour == "white" { signal::white_noise(dur, rms, 0xa11ce) } else { signal::pink_noise(dur, rms, 0xa11ce) };
    signal::fade_ends(&mut sig, (0.02 * sr) as usize);
    let pre_roll = (0.5 * sr) as usize;
    let max_delay = (opts.max_delay_s * sr) as usize;
    let total = pre_roll + dur + max_delay + (0.5 * sr) as usize;
    progress.send("transfer", 0.0, &format!("{} noise for {:.0} s", opts.colour, opts.duration_s));
    let played = sig.clone();
    let out = pass(engine, target, None, sig, pre_roll, total, progress, "transfer")?;
    let channels: Vec<_> = out
        .captured
        .iter()
        .zip(&target.input_channels)
        .map(|(c, &ch)| ChannelReport { input_channel: ch, analysis: transfer::analyse(&played, c, pre_roll, max_delay, sr, 24) })
        .collect();
    Ok(TransferReport { facts: out.facts, colour: opts.colour.clone(), level_dbfs: opts.level_dbfs, channels })
}

// ------------------------------------------------------------- stability

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StabilityOptions {
    /// Buffer sizes to try, in order.
    pub buffer_sizes: Vec<u32>,
    #[serde(default = "StabilityOptions::d_dur")]
    pub duration_s: f64,
    #[serde(default = "StabilityOptions::d_freq")]
    pub frequency: f64,
    #[serde(default = "StabilityOptions::d_level")]
    pub level_dbfs: f64,
    #[serde(default = "StabilityOptions::d_max_delay")]
    pub max_delay_s: f64,
}

impl StabilityOptions {
    fn d_dur() -> f64 {
        6.0
    }
    fn d_freq() -> f64 {
        1000.0
    }
    fn d_level() -> f64 {
        -12.0
    }
    fn d_max_delay() -> f64 {
        1.0
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// No dropouts, no discontinuities, no host-reported xruns.
    Clean,
    /// The host flagged an underrun or overrun but the audio came through.
    XrunsOnly,
    /// The recording shows glitches.
    Glitchy,
    /// The stream would not open at this size.
    FailedToOpen,
    /// The stream opened but the signal never came back.
    NoSignal,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StabilityRow {
    pub requested_frames: u32,
    pub verdict: Verdict,
    pub error: Option<String>,
    pub facts: Option<StreamFacts>,
    pub callbacks: Option<stability::CallbackStats>,
    pub latency_ms: Option<f64>,
    pub latency_samples: Option<f64>,
    pub glitches: Vec<ChannelReport<stability::GlitchReport>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StabilityReport {
    pub rows: Vec<StabilityRow>,
    pub duration_s: f64,
    pub frequency: f64,
}

pub fn stability_test(engine: &Engine, target: &Target, opts: &StabilityOptions, progress: &mut Progress) -> Result<StabilityReport> {
    let sr = target.sample_rate;
    let dur_s = opts.duration_s.clamp(1.0, 60.0);
    let dur = (dur_s * sr) as usize;
    let f = opts.frequency.clamp(20.0, 0.4 * sr);
    let burst = signal::latency_burst(sr, 0.04, signal::amplitude(opts.level_dbfs));
    let max_delay = (opts.max_delay_s * sr) as usize;
    let pre_roll = (0.3 * sr) as usize;
    let gap = max_delay + (0.2 * sr) as usize;
    let tone_at = burst.len() + gap;
    let mut sig = vec![0.0f32; tone_at];
    sig[..burst.len()].copy_from_slice(&burst);
    sig.extend(signal::sine(sr, f, signal::amplitude(opts.level_dbfs), dur, (0.01 * sr) as usize));
    let total = pre_roll + sig.len() + max_delay + (0.3 * sr) as usize;

    let sizes: Vec<u32> = opts.buffer_sizes.iter().copied().filter(|&s| (8..=65536).contains(&s)).collect();
    if sizes.is_empty() {
        return Err(AudioError::Config("no buffer sizes to try".into()));
    }
    let mut rows = Vec::with_capacity(sizes.len());
    for (i, &size) in sizes.iter().enumerate() {
        if progress.cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(AudioError::Cancelled);
        }
        let base = i as f64 / sizes.len() as f64;
        progress.send("stability", base, &format!("{size} frames ({:.2} ms) — {}/{}", size as f64 / sr * 1000.0, i + 1, sizes.len()));
        let mut sub_report = |e: ProgressEvent| {
            let f = base + e.fraction / sizes.len() as f64;
            (progress.report)(ProgressEvent { phase: "stability".into(), fraction: f, message: e.message });
        };
        let mut sub = Progress { cancel: progress.cancel, report: &mut sub_report };
        match pass(engine, target, Some(size), sig.clone(), pre_roll, total, &mut sub, "stability") {
            Err(AudioError::Cancelled) => return Err(AudioError::Cancelled),
            Err(e) => rows.push(StabilityRow {
                requested_frames: size,
                verdict: Verdict::FailedToOpen,
                error: Some(e.to_string()),
                facts: None,
                callbacks: None,
                latency_ms: None,
                latency_samples: None,
                glitches: vec![],
            }),
            Ok(out) => {
                let stats = stability::callback_stats(&out.records, size as usize, sr, out.facts.cpu_load);
                let lat = out.captured.first().map(|c| latency::measure(c, &burst, &[pre_roll], max_delay, sr));
                let found = lat.as_ref().map(|l| l.found > 0).unwrap_or(false);
                let delay = lat.as_ref().filter(|l| l.found > 0).map(|l| l.median_samples as usize).unwrap_or(0);
                let glitches: Vec<_> = out
                    .captured
                    .iter()
                    .zip(&target.input_channels)
                    .map(|(c, &ch)| {
                        let start = pre_roll + tone_at + delay;
                        let r = steady_region(c, sr, start, dur);
                        ChannelReport { input_channel: ch, analysis: stability::find_glitches(&c[r], sr, f) }
                    })
                    .collect();
                let xruns = stats.input_overflows + stats.input_underflows + stats.output_underflows + stats.output_overflows;
                let glitched = glitches.iter().any(|g| g.analysis.discontinuities + g.analysis.dropouts > 0);
                let verdict = if !found {
                    Verdict::NoSignal
                } else if glitched {
                    Verdict::Glitchy
                } else if xruns > 0 {
                    Verdict::XrunsOnly
                } else {
                    Verdict::Clean
                };
                rows.push(StabilityRow {
                    requested_frames: size,
                    verdict,
                    error: None,
                    latency_ms: lat.as_ref().filter(|l| l.found > 0).map(|l| l.median_ms),
                    latency_samples: lat.as_ref().filter(|l| l.found > 0).map(|l| l.median_samples),
                    facts: Some(out.facts),
                    callbacks: Some(stats),
                    glitches,
                });
            }
        }
    }
    progress.send("stability", 1.0, "done");
    Ok(StabilityReport { rows, duration_s: dur_s, frequency: f })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_region_skips_the_onset() {
        let sr = 48000.0;
        let mut x = vec![0.0f32; 48000];
        x.extend(signal::sine(sr, 1000.0, 0.5, 96000, 0));
        x.extend(vec![0.0f32; 24000]);
        let r = steady_region(&x, sr, 40000, 96000);
        // Onset at 48000; region starts a quarter second in and ends a tenth
        // of a second before the tone stops.
        assert!((r.start as i64 - 60000).abs() < 600, "{r:?}");
        assert!((r.end as i64 - (48000 + 96000 - 4800)).abs() < 600, "{r:?}");
    }
}
