//! One duplex pass through a device: play a signal, record what returns.
//!
//! ## The rule that shapes this file
//!
//! **The driver callback must not allocate, lock or block.** Everything it
//! touches is allocated before the stream opens: the signal to play, the
//! buffers to record into, and a fixed-capacity table of per-callback
//! records. The callback copies samples in and out, notes the time, and
//! returns. When it has played and recorded `total_frames` it returns
//! `paComplete`, and the thread that opened the stream — which has been
//! polling — collects the buffers after the stream has stopped. Nothing is
//! shared while the stream runs, so nothing needs a lock.
//!
//! ## Why the output and the recording share a frame clock
//!
//! In a duplex PortAudio callback the input and output buffers cover the
//! same frames: frame `k` of the output buffer leaves the DAC at the same
//! nominal time frame `k` of the input buffer entered the ADC. So the
//! position at which a burst was written and the position at which it is
//! found in the recording differ by exactly the round-trip delay, buffers
//! included — with no clock to synchronise and nothing to calibrate.
//!
//! ## What `buffer_frames` does
//!
//! It is passed to `Pa_OpenStream` as `framesPerBuffer`. On CoreAudio (with
//! the "change device parameters" flag, which this crate always sets unless
//! told not to) PortAudio applies it to the device's own buffer size. On
//! ASIO it selects the driver's buffer size if the driver allows that
//! value. On WASAPI exclusive mode it sizes the period. Where a host cannot
//! honour it, PortAudio adapts with its own buffering and the callback still
//! receives that many frames — which is why [`Outcome`] carries what the
//! host reports about the buffer it actually used, and why the bench
//! measures the round-trip latency at every size rather than trusting any
//! of this.

use std::os::raw::{c_int, c_ulong, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::{check, AudioError, Engine, HostKind, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SideConfig {
    pub device: i32,
    /// Number of channels to open — the highest selected channel plus one.
    pub channels: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamConfig {
    pub input: Option<SideConfig>,
    pub output: Option<SideConfig>,
    pub sample_rate: f64,
    /// Frames per buffer; 0 lets the host choose.
    pub buffer_frames: u32,
    /// Latency to suggest to PortAudio, in seconds. `None` means one
    /// buffer's worth, which is the lowest that works everywhere.
    pub suggested_latency_s: Option<f64>,
    /// WASAPI: open the device in exclusive mode.
    #[serde(default)]
    pub wasapi_exclusive: bool,
    /// CoreAudio: set the device's own sample rate and buffer size, and fail
    /// rather than resample if it cannot. Default true.
    #[serde(default = "default_true")]
    pub mac_change_device: bool,
}

fn default_true() -> bool {
    true
}

/// What to play and what to record.
pub struct Job {
    /// Mono signal, written to each of `out_channels`.
    pub signal: Vec<f32>,
    pub out_channels: Vec<usize>,
    /// Input channels to record, each into its own buffer.
    pub in_channels: Vec<usize>,
    /// Frames to run for. The signal is padded with silence to this length.
    pub total_frames: usize,
}

/// What PortAudio said about the stream once it was open.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StreamFacts {
    pub sample_rate: f64,
    pub input_latency_ms: f64,
    pub output_latency_ms: f64,
    /// Requested frames per buffer.
    pub buffer_frames: u32,
    /// What the host reported for the buffer it actually used, where the
    /// host can say (CoreAudio device buffer, WASAPI host buffer).
    pub host_buffer_frames: Option<u32>,
    pub host_buffer_source: Option<String>,
    /// `outputBufferDacTime − inputBufferAdcTime` on the first callback:
    /// the host's own claim for the loop's latency, in ms.
    pub reported_loop_ms: Option<f64>,
    pub cpu_load: f64,
}

pub struct Outcome {
    /// One buffer per entry in `Job::in_channels`.
    pub captured: Vec<Vec<f32>>,
    pub frames: usize,
    /// Per callback: frames delivered, seconds since the previous callback,
    /// PortAudio status flags.
    pub records: Vec<(usize, f64, u32)>,
    pub facts: StreamFacts,
}

/// Holder for the host-specific stream info structs, which must outlive the
/// `PaStreamParameters` that point at them.
pub struct HostSpecific {
    #[cfg(target_os = "macos")]
    mac: Option<Box<pa_sys::mac::PaMacCoreStreamInfo>>,
    #[cfg(target_os = "windows")]
    wasapi: Option<Box<pa_sys::win::PaWasapiStreamInfo>>,
    _kind: HostKind,
}

impl HostSpecific {
    #[allow(unused_variables)]
    pub fn for_device(engine: &Engine, device: i32, mac_change_device: bool, wasapi_exclusive: bool) -> HostSpecific {
        let kind = engine.host_kind_of(device);
        HostSpecific {
            #[cfg(target_os = "macos")]
            mac: (kind == HostKind::CoreAudio).then(|| {
                let mut info: Box<pa_sys::mac::PaMacCoreStreamInfo> = Box::new(unsafe { std::mem::zeroed() });
                let flags = if mac_change_device { pa_sys::mac::paMacCorePro } else { pa_sys::mac::paMacCorePlayNice };
                unsafe { pa_sys::mac::PaMacCore_SetupStreamInfo(&mut *info, flags) };
                info
            }),
            #[cfg(target_os = "windows")]
            wasapi: (kind == HostKind::Wasapi).then(|| {
                let mut info: Box<pa_sys::win::PaWasapiStreamInfo> = Box::new(unsafe { std::mem::zeroed() });
                info.size = std::mem::size_of::<pa_sys::win::PaWasapiStreamInfo>() as c_ulong;
                info.hostApiType = pa_sys::paWASAPI;
                info.version = 1;
                info.flags = pa_sys::win::paWinWasapiThreadPriority;
                info.threadPriority = pa_sys::win::eThreadPriorityProAudio;
                if wasapi_exclusive {
                    info.flags |= pa_sys::win::paWinWasapiExclusive;
                }
                info
            }),
            _kind: kind,
        }
    }

    pub fn ptr(&mut self) -> *mut c_void {
        #[cfg(target_os = "macos")]
        if let Some(m) = &mut self.mac {
            return &mut **m as *mut _ as *mut c_void;
        }
        #[cfg(target_os = "windows")]
        if let Some(w) = &mut self.wasapi {
            return &mut **w as *mut _ as *mut c_void;
        }
        std::ptr::null_mut()
    }
}

struct Shared {
    signal: Vec<f32>,
    out_channels: Vec<usize>,
    out_nch: usize,
    in_channels: Vec<usize>,
    in_nch: usize,
    captured: Vec<Vec<f32>>,
    total: usize,
    pos: usize,
    records: Vec<(usize, f64, u32)>,
    last: Option<Instant>,
    first_time: Option<(f64, f64, f64)>,
    done: AtomicBool,
}

unsafe extern "C" fn callback(
    input: *const c_void,
    output: *mut c_void,
    frame_count: c_ulong,
    time_info: *const pa_sys::PaStreamCallbackTimeInfo,
    flags: pa_sys::PaStreamCallbackFlags,
    user: *mut c_void,
) -> c_int {
    let s = &mut *(user as *mut Shared);
    let n = frame_count as usize;

    let now = Instant::now();
    let interval = s.last.map(|l| now.duration_since(l).as_secs_f64()).unwrap_or(0.0);
    s.last = Some(now);
    if s.records.len() < s.records.capacity() {
        s.records.push((n, interval, flags as u32));
    }
    if s.first_time.is_none() && !time_info.is_null() {
        let t = *time_info;
        s.first_time = Some((t.inputBufferAdcTime, t.currentTime, t.outputBufferDacTime));
    }

    if !output.is_null() && s.out_nch > 0 {
        let out = std::slice::from_raw_parts_mut(output as *mut f32, n * s.out_nch);
        out.fill(0.0);
        for f in 0..n {
            let idx = s.pos + f;
            let v = if idx < s.signal.len() { s.signal[idx] } else { 0.0 };
            for &ch in &s.out_channels {
                out[f * s.out_nch + ch] = v;
            }
        }
    }
    if !input.is_null() && s.in_nch > 0 {
        let inp = std::slice::from_raw_parts(input as *const f32, n * s.in_nch);
        for (si, &ch) in s.in_channels.iter().enumerate() {
            let buf = &mut s.captured[si];
            for f in 0..n {
                let idx = s.pos + f;
                if idx < buf.len() {
                    buf[idx] = inp[f * s.in_nch + ch];
                }
            }
        }
    }
    s.pos += n;
    if s.pos >= s.total {
        s.done.store(true, Ordering::Release);
        pa_sys::paComplete
    } else {
        pa_sys::paContinue
    }
}

/// Run one job. `progress` is called with the fraction complete; `cancel`
/// is checked between polls and aborts the stream.
pub fn run(engine: &Engine, cfg: &StreamConfig, job: Job, cancel: &AtomicBool, progress: &mut dyn FnMut(f64)) -> Result<Outcome> {
    if cfg.input.is_none() && cfg.output.is_none() {
        return Err(AudioError::Config("a stream needs an input, an output or both".into()));
    }
    let sr = cfg.sample_rate;
    if !(8000.0..=768000.0).contains(&sr) {
        return Err(AudioError::Config(format!("sample rate {sr} is not sensible")));
    }
    let latency = cfg
        .suggested_latency_s
        .unwrap_or(if cfg.buffer_frames > 0 { cfg.buffer_frames as f64 / sr } else { 0.0 });

    let mut in_host = cfg.input.as_ref().map(|s| HostSpecific::for_device(engine, s.device, cfg.mac_change_device, cfg.wasapi_exclusive));
    let mut out_host = cfg.output.as_ref().map(|s| HostSpecific::for_device(engine, s.device, cfg.mac_change_device, cfg.wasapi_exclusive));

    let in_params = cfg.input.as_ref().map(|s| pa_sys::PaStreamParameters {
        device: s.device,
        channelCount: s.channels as c_int,
        sampleFormat: pa_sys::paFloat32,
        suggestedLatency: latency,
        hostApiSpecificStreamInfo: in_host.as_mut().map(|h| h.ptr()).unwrap_or(std::ptr::null_mut()),
    });
    let out_params = cfg.output.as_ref().map(|s| pa_sys::PaStreamParameters {
        device: s.device,
        channelCount: s.channels as c_int,
        sampleFormat: pa_sys::paFloat32,
        suggestedLatency: latency,
        hostApiSpecificStreamInfo: out_host.as_mut().map(|h| h.ptr()).unwrap_or(std::ptr::null_mut()),
    });

    let in_nch = cfg.input.as_ref().map(|s| s.channels).unwrap_or(0);
    let out_nch = cfg.output.as_ref().map(|s| s.channels).unwrap_or(0);
    for &c in &job.in_channels {
        if c >= in_nch {
            return Err(AudioError::Config(format!("input channel {c} is outside the {in_nch} channels opened")));
        }
    }
    for &c in &job.out_channels {
        if c >= out_nch {
            return Err(AudioError::Config(format!("output channel {c} is outside the {out_nch} channels opened")));
        }
    }

    let total = job.total_frames.max(job.signal.len());
    let min_buffer = if cfg.buffer_frames > 0 { cfg.buffer_frames as usize } else { 16 };
    let shared = Box::new(Shared {
        signal: job.signal,
        out_channels: job.out_channels,
        out_nch,
        in_channels: job.in_channels.clone(),
        in_nch,
        captured: job.in_channels.iter().map(|_| vec![0.0f32; total]).collect(),
        total,
        pos: 0,
        records: Vec::with_capacity(total / min_buffer + 64),
        last: None,
        first_time: None,
        done: AtomicBool::new(false),
    });
    let shared_ptr = Box::into_raw(shared);

    let mut stream: *mut pa_sys::PaStream = std::ptr::null_mut();
    let open = unsafe {
        pa_sys::Pa_OpenStream(
            &mut stream,
            in_params.as_ref().map(|p| p as *const _).unwrap_or(std::ptr::null()),
            out_params.as_ref().map(|p| p as *const _).unwrap_or(std::ptr::null()),
            sr,
            cfg.buffer_frames as c_ulong,
            pa_sys::paClipOff,
            Some(callback),
            shared_ptr as *mut c_void,
        )
    };
    if let Err(e) = check(open) {
        drop(unsafe { Box::from_raw(shared_ptr) });
        return Err(e);
    }

    let result = (|| -> Result<StreamFacts> {
        let mut facts = StreamFacts { buffer_frames: cfg.buffer_frames, ..Default::default() };
        let info = unsafe { pa_sys::Pa_GetStreamInfo(stream) };
        if !info.is_null() {
            let i = unsafe { *info };
            facts.sample_rate = i.sampleRate;
            facts.input_latency_ms = i.inputLatency * 1000.0;
            facts.output_latency_ms = i.outputLatency * 1000.0;
        }
        host_buffer(engine, cfg, stream, &mut facts);

        check(unsafe { pa_sys::Pa_StartStream(stream) })?;
        let started = Instant::now();
        let expected = Duration::from_secs_f64(total as f64 / sr + 5.0);
        loop {
            std::thread::sleep(Duration::from_millis(40));
            let done = unsafe { (*shared_ptr).done.load(Ordering::Acquire) };
            let active = unsafe { pa_sys::Pa_IsStreamActive(stream) };
            let pos = unsafe { (*shared_ptr).pos };
            progress((pos as f64 / total as f64).min(1.0));
            if cancel.load(Ordering::Relaxed) {
                unsafe { pa_sys::Pa_AbortStream(stream) };
                return Err(AudioError::Cancelled);
            }
            if active == 0 || (done && active <= 0) {
                break;
            }
            if done && started.elapsed() > Duration::from_secs(3) {
                // paComplete was returned but the host never went inactive.
                unsafe { pa_sys::Pa_AbortStream(stream) };
                break;
            }
            if started.elapsed() > expected {
                unsafe { pa_sys::Pa_AbortStream(stream) };
                return Err(AudioError::Config("the stream ran but never delivered its frames — the device is not clocking".into()));
            }
        }
        facts.cpu_load = unsafe { pa_sys::Pa_GetStreamCpuLoad(stream) };
        unsafe { pa_sys::Pa_StopStream(stream) };
        Ok(facts)
    })();

    unsafe { pa_sys::Pa_CloseStream(stream) };
    let shared = unsafe { Box::from_raw(shared_ptr) };
    let mut facts = result?;
    if let Some((adc, _cur, dac)) = shared.first_time {
        if adc != 0.0 && dac != 0.0 {
            facts.reported_loop_ms = Some((dac - adc) * 1000.0);
        }
    }
    let frames = shared.pos.min(total);
    Ok(Outcome { captured: shared.captured, frames, records: shared.records, facts })
}

/// Ask the host what buffer it really used, where it can say.
#[allow(unused_variables)]
fn host_buffer(engine: &Engine, cfg: &StreamConfig, stream: *mut pa_sys::PaStream, facts: &mut StreamFacts) {
    #[cfg(target_os = "macos")]
    {
        let dev = unsafe {
            if cfg.output.is_some() {
                pa_sys::mac::PaMacCore_GetStreamOutputDevice(stream)
            } else {
                pa_sys::mac::PaMacCore_GetStreamInputDevice(stream)
            }
        };
        if dev != 0 {
            if let Some(frames) = crate::mac::buffer_frame_size(dev) {
                facts.host_buffer_frames = Some(frames);
                facts.host_buffer_source = Some("CoreAudio device buffer frame size".into());
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        let kind = cfg.output.as_ref().or(cfg.input.as_ref()).map(|s| engine.host_kind_of(s.device));
        if kind == Some(HostKind::Wasapi) {
            let (mut i, mut o) = (0u32, 0u32);
            let r = unsafe { pa_sys::win::PaWasapi_GetFramesPerHostBuffer(stream, &mut i, &mut o) };
            if r == pa_sys::paNoError {
                facts.host_buffer_frames = Some(if cfg.output.is_some() { o } else { i });
                facts.host_buffer_source = Some("WASAPI frames per host buffer".into());
            }
        }
    }
}
