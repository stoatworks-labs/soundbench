//! What the Windows host APIs add beyond `PaDeviceInfo`.
//!
//! WASAPI: the device's default (shared-mode mix) format, whether it is a
//! loopback capture endpoint, and its role. ASIO (with the `asio` feature):
//! the driver's channel names and buffer-size rules, which PortAudio relays
//! from `ASIOGetChannelInfo` and `ASIOGetBufferSize`.
//!
//! None of this has been exercised on real Windows hardware yet — see the
//! README's status section. It compiles on the Windows CI runner and the
//! ASIO path enumerates in a VM without an ASIO driver, which is where the
//! honesty of this comment ends.

use crate::{DeviceInfo, HostKind, KeyValue};

/// WAVEFORMATEXTENSIBLE, as bytes; enough to read the fields we report.
fn describe_waveformat(buf: &[u8]) -> Option<String> {
    if buf.len() < 18 {
        return None;
    }
    let u16_at = |o: usize| u16::from_le_bytes([buf[o], buf[o + 1]]);
    let u32_at = |o: usize| u32::from_le_bytes([buf[o], buf[o + 1], buf[o + 2], buf[o + 3]]);
    let tag = u16_at(0);
    let channels = u16_at(2);
    let rate = u32_at(4);
    let bits = u16_at(14);
    let cb = u16_at(16);
    let mut s = format!("{bits} bit, {channels} ch, {rate} Hz");
    if tag == 0xFFFE && cb >= 22 && buf.len() >= 40 {
        let valid = u16_at(18);
        let mask = u32_at(20);
        // SubFormat GUID: first four bytes identify PCM (1) or IEEE float (3).
        let sub = u32_at(24);
        let kind = match sub {
            1 => "integer",
            3 => "float",
            _ => "other",
        };
        s = format!("{valid} bit {kind} in {bits}, {channels} ch, {rate} Hz, channel mask 0x{mask:x}");
    } else if tag == 3 {
        s.push_str(" float");
    } else if tag == 1 {
        s.push_str(" integer");
    }
    Some(s)
}

#[allow(unused_variables)]
pub fn describe(info: &DeviceInfo, in_names: &mut Vec<String>, out_names: &mut Vec<String>, out: &mut Vec<KeyValue>, notes: &mut Vec<String>) {
    let mut push = |k: &str, v: String| out.push(KeyValue { key: k.into(), value: v });
    match info.host_kind {
        HostKind::Wasapi => {
            let mut buf = vec![0u8; 64];
            let n = unsafe { pa_sys::win::PaWasapi_GetDeviceDefaultFormat(buf.as_mut_ptr() as *mut _, buf.len() as u32, info.index) };
            if n > 0 {
                if let Some(s) = describe_waveformat(&buf[..n as usize]) {
                    push("Default format", s);
                }
            }
            let mut mix = vec![0u8; 64];
            let n = unsafe { pa_sys::win::PaWasapi_GetDeviceMixFormat(mix.as_mut_ptr() as *mut _, mix.len() as u32, info.index) };
            if n > 0 {
                if let Some(s) = describe_waveformat(&mix[..n as usize]) {
                    push("Mix format (shared mode)", s);
                }
            }
            let lb = unsafe { pa_sys::win::PaWasapi_IsLoopback(info.index) };
            if lb == 1 {
                push("Loopback endpoint", "yes — this records what the output plays".into());
            }
            let role = unsafe { pa_sys::win::PaWasapi_GetDeviceRole(info.index) };
            let role_name = match role {
                1 => "console",
                2 => "multimedia",
                3 => "communications",
                _ => "",
            };
            if !role_name.is_empty() {
                push("Default role", role_name.into());
            }
        }
        #[cfg(feature = "asio")]
        HostKind::Asio => {
            use std::os::raw::c_char;
            for i in 0..info.max_input_channels {
                let mut p: *const c_char = std::ptr::null();
                let r = unsafe { pa_sys::win::asio::PaAsio_GetInputChannelName(info.index, i, &mut p) };
                let n = if r == pa_sys::paNoError { unsafe { crate::cstr(p) } } else { String::new() };
                in_names.push(if n.is_empty() { format!("Channel {}", i + 1) } else { n });
            }
            for i in 0..info.max_output_channels {
                let mut p: *const c_char = std::ptr::null();
                let r = unsafe { pa_sys::win::asio::PaAsio_GetOutputChannelName(info.index, i, &mut p) };
                let n = if r == pa_sys::paNoError { unsafe { crate::cstr(p) } } else { String::new() };
                out_names.push(if n.is_empty() { format!("Channel {}", i + 1) } else { n });
            }
            notes.push("ASIO drivers are exclusive: close any DAW using this interface before running a test.".into());
        }
        _ => {}
    }
}

/// Open the ASIO driver's own control panel for a device.
#[cfg(feature = "asio")]
pub fn show_asio_control_panel(device: i32) -> crate::Result<()> {
    crate::check(unsafe { pa_sys::win::asio::PaAsio_ShowControlPanel(device, std::ptr::null_mut()) })
}
