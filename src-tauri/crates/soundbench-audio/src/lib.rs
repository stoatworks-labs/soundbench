//! The device side of the bench.
//!
//! ```text
//!   Engine ── hosts() ── devices() ── detail(device)      what is there
//!      │
//!      └── stream::run(config, job)                        one duplex pass:
//!               driver callback ─▶ preallocated buffers    play this, record that
//!      │
//!      └── bench::*                                        the tests, built on run()
//! ```
//!
//! PortAudio is initialised once per [`Engine`] and terminated when it is
//! dropped. Its device list is a snapshot taken at initialisation, so
//! plugging in an interface means making a new engine — the app's "rescan".
//!
//! Every host API the vendored PortAudio was built with is listed, whether
//! or not it has devices, and ASIO is listed on Windows even in a build that
//! lacks it, marked unavailable and saying why. An interface that has
//! silently vanished from a list is a support question every time; a row
//! that says "not compiled in" is not.

pub mod bench;
pub mod stream;

#[cfg(target_os = "macos")]
pub mod mac;
#[cfg(target_os = "windows")]
pub mod win;

use std::ffi::CStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("PortAudio: {text} ({code})")]
    Pa { code: i32, text: String, host: Option<String> },
    #[error("no such device index {0}")]
    NoDevice(i32),
    #[error("{0}")]
    Config(String),
    #[error("cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, AudioError>;

/// Turn a PortAudio return code into an error, with the host's own text when
/// PortAudio has one (it does for `paUnanticipatedHostError`, which is where
/// the useful message usually is).
pub(crate) fn check(code: i32) -> Result<()> {
    if code >= 0 {
        return Ok(());
    }
    let text = unsafe { cstr(pa_sys::Pa_GetErrorText(code)) };
    let host = if code == pa_sys::paUnanticipatedHostError {
        let h = unsafe { pa_sys::Pa_GetLastHostErrorInfo() };
        if h.is_null() {
            None
        } else {
            let h = unsafe { *h };
            let t = unsafe { cstr(h.errorText) };
            Some(format!("{t} (host error {})", h.errorCode))
        }
    } else {
        None
    };
    Err(AudioError::Pa { code, text, host })
}

pub(crate) unsafe fn cstr(p: *const std::os::raw::c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}

/// The families of audio API a device can belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostKind {
    CoreAudio,
    Asio,
    Wasapi,
    WdmKs,
    DirectSound,
    Mme,
    Alsa,
    Jack,
    PulseAudio,
    Other,
}

impl HostKind {
    pub fn from_type_id(t: i32) -> HostKind {
        match t {
            pa_sys::paCoreAudio => HostKind::CoreAudio,
            pa_sys::paASIO => HostKind::Asio,
            pa_sys::paWASAPI => HostKind::Wasapi,
            pa_sys::paWDMKS => HostKind::WdmKs,
            pa_sys::paDirectSound => HostKind::DirectSound,
            pa_sys::paMME => HostKind::Mme,
            pa_sys::paALSA => HostKind::Alsa,
            pa_sys::paJACK => HostKind::Jack,
            pa_sys::paPulseAudio => HostKind::PulseAudio,
            _ => HostKind::Other,
        }
    }

    /// What the host API is, in a sentence, for the UI's tooltip.
    pub fn describe(self) -> &'static str {
        match self {
            HostKind::CoreAudio => "Apple's audio HAL. The buffer size and sample rate set here are applied to the device itself.",
            HostKind::Asio => "Steinberg's driver model. One application at a time; the driver owns the buffer size list and usually the sample rate.",
            HostKind::Wasapi => "Windows Audio Session API. Shared mode goes through the Windows mixer at its 10 ms period; exclusive mode talks to the device directly.",
            HostKind::WdmKs => "Windows kernel streaming, below the mixer. Low latency, exclusive, and not every driver behaves.",
            HostKind::DirectSound => "The legacy games API, layered on WASAPI on modern Windows. Not a low-latency path.",
            HostKind::Mme => "The oldest Windows audio API (waveOut). High latency by design; here for completeness.",
            HostKind::Alsa => "Linux kernel audio. On a PipeWire or Pulse desktop this is the compatibility layer, not the hardware.",
            HostKind::Jack => "The JACK audio server.",
            HostKind::PulseAudio => "The PulseAudio sound server.",
            HostKind::Other => "",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostInfo {
    /// PortAudio host API index; `None` for a host this build does not carry.
    pub index: Option<i32>,
    pub kind: HostKind,
    pub name: String,
    pub description: String,
    pub device_count: i32,
    pub default_input: Option<i32>,
    pub default_output: Option<i32>,
    pub is_default: bool,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    /// Global PortAudio device index — the handle everything else takes.
    pub index: i32,
    pub host: i32,
    pub host_kind: HostKind,
    pub host_name: String,
    pub name: String,
    pub max_input_channels: i32,
    pub max_output_channels: i32,
    pub default_sample_rate: f64,
    pub default_low_input_latency_ms: f64,
    pub default_low_output_latency_ms: f64,
    pub default_high_input_latency_ms: f64,
    pub default_high_output_latency_ms: f64,
    pub is_default_input: bool,
    pub is_default_output: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleRateSupport {
    pub rate: u32,
    pub input: bool,
    pub output: bool,
    pub duplex: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BufferSizes {
    /// Where the numbers came from: the driver, or a guess.
    pub source: String,
    pub min: Option<u32>,
    pub max: Option<u32>,
    pub preferred: Option<u32>,
    /// ASIO's granularity: −1 for powers of two, 0 for a single size.
    pub granularity: Option<i32>,
    /// Sizes worth trying, ascending.
    pub candidates: Vec<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyValue {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDetail {
    pub info: DeviceInfo,
    pub sample_rates: Vec<SampleRateSupport>,
    pub buffer_sizes: BufferSizes,
    pub input_channel_names: Vec<String>,
    pub output_channel_names: Vec<String>,
    /// Everything the platform's own API adds, for display, in the order
    /// the driver lists it.
    pub platform: Vec<KeyValue>,
    pub notes: Vec<String>,
}

/// Rates worth probing. The device's own default is added if it is not here.
pub const PROBE_RATES: [u32; 8] = [44100, 48000, 88200, 96000, 176400, 192000, 352800, 384000];

/// Powers of two from 16 to 4096 — the universal buffer-size ladder.
pub const POW2_SIZES: [u32; 9] = [16, 32, 64, 128, 256, 512, 1024, 2048, 4096];

/// An initialised PortAudio.
pub struct Engine {
    _private: (),
}

impl Engine {
    pub fn new() -> Result<Engine> {
        check(unsafe { pa_sys::Pa_Initialize() })?;
        Ok(Engine { _private: () })
    }

    pub fn version() -> String {
        let v = unsafe { pa_sys::Pa_GetVersionInfo() };
        if v.is_null() {
            return "PortAudio (unknown version)".into();
        }
        unsafe { cstr((*v).versionText) }
    }

    /// Every host API this build knows about — present or not.
    pub fn hosts(&self) -> Vec<HostInfo> {
        let mut out = Vec::new();
        let n = unsafe { pa_sys::Pa_GetHostApiCount() };
        let default = unsafe { pa_sys::Pa_GetDefaultHostApi() };
        for i in 0..n.max(0) {
            let p = unsafe { pa_sys::Pa_GetHostApiInfo(i) };
            if p.is_null() {
                continue;
            }
            let h = unsafe { *p };
            let kind = HostKind::from_type_id(h.type_);
            out.push(HostInfo {
                index: Some(i),
                kind,
                name: unsafe { cstr(h.name) },
                description: kind.describe().into(),
                device_count: h.deviceCount,
                default_input: (h.defaultInputDevice >= 0).then_some(h.defaultInputDevice),
                default_output: (h.defaultOutputDevice >= 0).then_some(h.defaultOutputDevice),
                is_default: i == default,
                available: true,
                note: None,
            });
        }
        // ASIO is worth a row even when it is not here.
        if cfg!(target_os = "windows") && !out.iter().any(|h| h.kind == HostKind::Asio) {
            out.push(HostInfo {
                index: None,
                kind: HostKind::Asio,
                name: "ASIO".into(),
                description: HostKind::Asio.describe().into(),
                device_count: 0,
                default_input: None,
                default_output: None,
                is_default: false,
                available: false,
                note: Some("This build was made without ASIO support. See docs/asio.md.".into()),
            });
        }
        out
    }

    /// Every device, across every host.
    pub fn devices(&self) -> Vec<DeviceInfo> {
        let n = unsafe { pa_sys::Pa_GetDeviceCount() };
        let din = unsafe { pa_sys::Pa_GetDefaultInputDevice() };
        let dout = unsafe { pa_sys::Pa_GetDefaultOutputDevice() };
        (0..n.max(0)).filter_map(|i| self.device(i, din, dout)).collect()
    }

    pub fn device_info(&self, index: i32) -> Result<DeviceInfo> {
        let din = unsafe { pa_sys::Pa_GetDefaultInputDevice() };
        let dout = unsafe { pa_sys::Pa_GetDefaultOutputDevice() };
        self.device(index, din, dout).ok_or(AudioError::NoDevice(index))
    }

    fn device(&self, i: i32, din: i32, dout: i32) -> Option<DeviceInfo> {
        let p = unsafe { pa_sys::Pa_GetDeviceInfo(i) };
        if p.is_null() {
            return None;
        }
        let d = unsafe { *p };
        let hp = unsafe { pa_sys::Pa_GetHostApiInfo(d.hostApi) };
        let (host_kind, host_name) = if hp.is_null() {
            (HostKind::Other, String::new())
        } else {
            let h = unsafe { *hp };
            (HostKind::from_type_id(h.type_), unsafe { cstr(h.name) })
        };
        Some(DeviceInfo {
            index: i,
            host: d.hostApi,
            host_kind,
            host_name,
            name: unsafe { cstr(d.name) },
            max_input_channels: d.maxInputChannels,
            max_output_channels: d.maxOutputChannels,
            default_sample_rate: d.defaultSampleRate,
            default_low_input_latency_ms: d.defaultLowInputLatency * 1000.0,
            default_low_output_latency_ms: d.defaultLowOutputLatency * 1000.0,
            default_high_input_latency_ms: d.defaultHighInputLatency * 1000.0,
            default_high_output_latency_ms: d.defaultHighOutputLatency * 1000.0,
            is_default_input: i == din,
            is_default_output: i == dout,
        })
    }

    /// Everything the driver will say about one device: which sample rates it
    /// accepts, what buffer sizes it offers, channel names, and the
    /// platform's own property list.
    pub fn detail(&self, index: i32) -> Result<DeviceDetail> {
        let info = self.device_info(index)?;
        let mut notes = Vec::new();

        // Sample rates. On CoreAudio the HAL publishes the list outright, so
        // it is read rather than probed: PortAudio's probe with the
        // change-device-parameters flag actually SETS each candidate rate on
        // the device and waits for it to take, which is slow, intrusive, and
        // hangs for seconds on every rate the device lacks. Elsewhere
        // `Pa_IsFormatSupported` is asked three ways.
        let mut rates: Vec<u32> = PROBE_RATES.to_vec();
        let def = info.default_sample_rate.round() as u32;
        if def > 0 && !rates.contains(&def) {
            rates.push(def);
            rates.sort_unstable();
        }
        let mut sample_rates = Vec::new();
        #[allow(unused_mut)]
        let mut hal_rates: Option<Vec<(f64, f64)>> = None;
        #[cfg(target_os = "macos")]
        if info.host_kind == HostKind::CoreAudio {
            hal_rates = mac::available_rates(&info);
        }
        for r in rates {
            let (input, output, duplex) = if let Some(ranges) = &hal_rates {
                let ok = ranges.iter().any(|&(lo, hi)| (r as f64) >= lo - 0.5 && (r as f64) <= hi + 0.5);
                (ok && info.max_input_channels > 0, ok && info.max_output_channels > 0, ok && info.max_input_channels > 0 && info.max_output_channels > 0)
            } else {
                let input = info.max_input_channels > 0 && self.format_ok(index, info.max_input_channels, true, r as f64);
                let output = info.max_output_channels > 0 && self.format_ok(index, info.max_output_channels, false, r as f64);
                let duplex = input && output && self.duplex_ok(index, info.max_input_channels, info.max_output_channels, r as f64);
                (input, output, duplex)
            };
            sample_rates.push(SampleRateSupport { rate: r, input, output, duplex });
        }

        let mut buffer_sizes = self.buffer_sizes(&info, &mut notes);
        if buffer_sizes.candidates.is_empty() {
            buffer_sizes.candidates = POW2_SIZES.iter().copied().filter(|&s| s >= 32).collect();
        }

        let mut input_channel_names = Vec::new();
        let mut output_channel_names = Vec::new();
        let mut platform = Vec::new();
        self.platform_detail(&info, &mut input_channel_names, &mut output_channel_names, &mut platform, &mut notes);
        for (names, count) in [(&mut input_channel_names, info.max_input_channels), (&mut output_channel_names, info.max_output_channels)] {
            while (names.len() as i32) < count {
                names.push(format!("Channel {}", names.len() + 1));
            }
        }

        Ok(DeviceDetail { info, sample_rates, buffer_sizes, input_channel_names, output_channel_names, platform, notes })
    }

    fn format_ok(&self, device: i32, channels: i32, input: bool, rate: f64) -> bool {
        let mut host = stream::HostSpecific::for_device(self, device, false, false);
        let p = pa_sys::PaStreamParameters {
            device,
            channelCount: channels,
            sampleFormat: pa_sys::paFloat32,
            suggestedLatency: 0.0,
            hostApiSpecificStreamInfo: host.ptr(),
        };
        let r = unsafe {
            if input {
                pa_sys::Pa_IsFormatSupported(&p, std::ptr::null(), rate)
            } else {
                pa_sys::Pa_IsFormatSupported(std::ptr::null(), &p, rate)
            }
        };
        r == pa_sys::paFormatIsSupported
    }

    fn duplex_ok(&self, device: i32, in_ch: i32, out_ch: i32, rate: f64) -> bool {
        let mut host = stream::HostSpecific::for_device(self, device, false, false);
        let pi = pa_sys::PaStreamParameters {
            device,
            channelCount: in_ch,
            sampleFormat: pa_sys::paFloat32,
            suggestedLatency: 0.0,
            hostApiSpecificStreamInfo: host.ptr(),
        };
        let po = pa_sys::PaStreamParameters { channelCount: out_ch, ..pi };
        unsafe { pa_sys::Pa_IsFormatSupported(&pi, &po, rate) == pa_sys::paFormatIsSupported }
    }

    /// Host kind of a device, for the stream layer.
    pub(crate) fn host_kind_of(&self, device: i32) -> HostKind {
        self.device_info(device).map(|d| d.host_kind).unwrap_or(HostKind::Other)
    }

    #[allow(unused_variables, unused_mut)]
    fn buffer_sizes(&self, info: &DeviceInfo, notes: &mut Vec<String>) -> BufferSizes {
        let mut b = BufferSizes { source: "assumed — powers of two; the run reports what the host actually delivered".into(), ..Default::default() };
        #[cfg(target_os = "macos")]
        if info.host_kind == HostKind::CoreAudio {
            let (mut lo, mut hi) = (0i64, 0i64);
            let r = unsafe { pa_sys::mac::PaMacCore_GetBufferSizeRange(info.index, &mut lo as *mut i64 as *mut _, &mut hi as *mut i64 as *mut _) };
            if r == pa_sys::paNoError && hi > 0 {
                b.source = "CoreAudio kAudioDevicePropertyBufferFrameSizeRange".into();
                b.min = Some(lo as u32);
                b.max = Some(hi as u32);
                let mut c: Vec<u32> = POW2_SIZES.iter().copied().filter(|&s| s as i64 >= lo && s as i64 <= hi).collect();
                if lo > 0 && !c.contains(&(lo as u32)) {
                    c.insert(0, lo as u32);
                }
                if !c.contains(&(hi as u32)) && hi <= 8192 {
                    c.push(hi as u32);
                }
                b.candidates = c;
            }
        }
        #[cfg(all(target_os = "windows", feature = "asio"))]
        if info.host_kind == HostKind::Asio {
            let (mut lo, mut hi, mut pref, mut gran) = (0i32, 0i32, 0i32, 0i32);
            let r = unsafe {
                pa_sys::win::asio::PaAsio_GetAvailableBufferSizes(
                    info.index,
                    &mut lo as *mut i32 as *mut _,
                    &mut hi as *mut i32 as *mut _,
                    &mut pref as *mut i32 as *mut _,
                    &mut gran as *mut i32 as *mut _,
                )
            };
            if r == pa_sys::paNoError && hi > 0 {
                b.source = "ASIOGetBufferSize".into();
                b.min = Some(lo as u32);
                b.max = Some(hi as u32);
                b.preferred = Some(pref as u32);
                b.granularity = Some(gran);
                let mut c = Vec::new();
                if gran == -1 {
                    let mut s = lo.max(1);
                    while s <= hi && c.len() < 32 {
                        c.push(s as u32);
                        s *= 2;
                    }
                } else if gran <= 0 || lo == hi {
                    c.push(pref.max(lo) as u32);
                } else {
                    let mut s = lo;
                    while s <= hi && c.len() < 32 {
                        c.push(s as u32);
                        s += gran;
                    }
                }
                if !c.contains(&(pref as u32)) && pref > 0 {
                    c.push(pref as u32);
                    c.sort_unstable();
                }
                b.candidates = c;
            }
        }
        if info.host_kind == HostKind::Wasapi {
            notes.push("WASAPI shared mode runs at the Windows mixer's 10 ms period whatever is asked for; exclusive mode honours the request. The run reports the host buffer the stream actually got.".into());
        }
        b
    }

    #[allow(unused_variables)]
    fn platform_detail(
        &self,
        info: &DeviceInfo,
        in_names: &mut Vec<String>,
        out_names: &mut Vec<String>,
        platform: &mut Vec<KeyValue>,
        notes: &mut Vec<String>,
    ) {
        #[cfg(target_os = "macos")]
        if info.host_kind == HostKind::CoreAudio {
            for i in 0..info.max_input_channels {
                let p = unsafe { pa_sys::mac::PaMacCore_GetChannelName(info.index, i, true) };
                let n = unsafe { cstr(p) };
                in_names.push(if n.is_empty() { format!("Channel {}", i + 1) } else { n });
            }
            for i in 0..info.max_output_channels {
                let p = unsafe { pa_sys::mac::PaMacCore_GetChannelName(info.index, i, false) };
                let n = unsafe { cstr(p) };
                out_names.push(if n.is_empty() { format!("Channel {}", i + 1) } else { n });
            }
            mac::describe(info, platform, notes);
        }
        #[cfg(target_os = "windows")]
        win::describe(info, in_names, out_names, platform, notes);
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        unsafe {
            pa_sys::Pa_Terminate();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_initialises_and_lists_something() {
        let e = Engine::new().expect("PortAudio initialises");
        let hosts = e.hosts();
        assert!(!hosts.is_empty(), "no host APIs at all");
        // Whatever the machine has, the listing must not panic, and every
        // device must belong to a listed host.
        for d in e.devices() {
            assert!(hosts.iter().any(|h| h.index == Some(d.host)), "{d:?}");
        }
        assert!(Engine::version().contains("PortAudio"));
    }
}
