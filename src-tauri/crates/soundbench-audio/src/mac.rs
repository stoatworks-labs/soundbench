//! What CoreAudio itself says about a device, beyond what PortAudio relays.
//!
//! PortAudio's `PaDeviceInfo` is the lowest common denominator across nine
//! host APIs. CoreAudio knows far more — the transport (USB, Thunderbolt,
//! built-in, virtual, aggregate), the device UID and manufacturer, the
//! nominal and available sample rates, the buffer size the device is set to
//! right now and the range it allows, the latency and safety offset the
//! driver declares for each direction, the physical stream format (the
//! bit depth the hardware is actually running), whether another process is
//! using it, and whether anyone has it in hog mode. All of that is read here
//! straight from the HAL with `AudioObjectGetPropertyData`.
//!
//! PortAudio does not expose the `AudioDeviceID` behind a device index, so
//! the device is found by name and channel counts in the HAL's own list.
//! When the match is ambiguous the first candidate is used, which is right
//! for every real interface and wrong only for two virtual devices that
//! share a name and a shape.

use std::os::raw::{c_char, c_void};

use crate::{DeviceInfo, KeyValue};

type OSStatus = i32;
type AudioObjectID = u32;

#[repr(C)]
#[derive(Clone, Copy)]
struct AudioObjectPropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
struct AudioValueRange {
    min: f64,
    max: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
struct AudioStreamBasicDescription {
    sample_rate: f64,
    format_id: u32,
    format_flags: u32,
    bytes_per_packet: u32,
    frames_per_packet: u32,
    bytes_per_frame: u32,
    channels_per_frame: u32,
    bits_per_channel: u32,
    reserved: u32,
}

#[repr(C)]
struct AudioBuffer {
    number_channels: u32,
    data_byte_size: u32,
    data: *mut c_void,
}

#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    fn AudioObjectGetPropertyDataSize(
        id: AudioObjectID,
        addr: *const AudioObjectPropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        out_size: *mut u32,
    ) -> OSStatus;
    fn AudioObjectGetPropertyData(
        id: AudioObjectID,
        addr: *const AudioObjectPropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        io_size: *mut u32,
        out_data: *mut c_void,
    ) -> OSStatus;
    fn AudioObjectHasProperty(id: AudioObjectID, addr: *const AudioObjectPropertyAddress) -> u8;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringGetCString(s: *const c_void, buf: *mut c_char, len: isize, encoding: u32) -> u8;
    fn CFRelease(cf: *const c_void);
}

const fn fourcc(s: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*s)
}

const SYSTEM_OBJECT: AudioObjectID = 1;
const SCOPE_GLOBAL: u32 = fourcc(b"glob");
const SCOPE_INPUT: u32 = fourcc(b"inpt");
const SCOPE_OUTPUT: u32 = fourcc(b"outp");
const HW_DEVICES: u32 = fourcc(b"dev#");
const OBJ_NAME: u32 = fourcc(b"lnam");
const OBJ_MANUFACTURER: u32 = fourcc(b"lmak");
const DEV_UID: u32 = fourcc(b"uid ");
const DEV_MODEL_UID: u32 = fourcc(b"muid");
const DEV_TRANSPORT: u32 = fourcc(b"tran");
const DEV_NOMINAL_RATE: u32 = fourcc(b"nsrt");
const DEV_AVAILABLE_RATES: u32 = fourcc(b"nsr#");
const DEV_BUFFER_FRAMES: u32 = fourcc(b"fsiz");
const DEV_BUFFER_RANGE: u32 = fourcc(b"fsz#");
const DEV_LATENCY: u32 = fourcc(b"ltnc");
const DEV_SAFETY_OFFSET: u32 = fourcc(b"saft");
const DEV_STREAMS: u32 = fourcc(b"stm#");
const DEV_STREAM_CONFIG: u32 = fourcc(b"slay");
const DEV_RUNNING_SOMEWHERE: u32 = fourcc(b"gone");
const DEV_HOG_MODE: u32 = fourcc(b"oink");
const DEV_CLOCK_SOURCE: u32 = fourcc(b"csrc");
const DEV_CLOCK_SOURCES: u32 = fourcc(b"csr#");
const DEV_IS_ALIVE: u32 = fourcc(b"livn");
const STREAM_PHYSICAL_FORMAT: u32 = fourcc(b"pft ");
const STREAM_LATENCY: u32 = fourcc(b"ltnc");
const CF_UTF8: u32 = 0x0800_0100;

fn addr(selector: u32, scope: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress { selector, scope, element: 0 }
}

fn get_u32(id: AudioObjectID, selector: u32, scope: u32) -> Option<u32> {
    let a = addr(selector, scope);
    let mut v: u32 = 0;
    let mut size = 4u32;
    let r = unsafe { AudioObjectGetPropertyData(id, &a, 0, std::ptr::null(), &mut size, &mut v as *mut u32 as *mut c_void) };
    (r == 0).then_some(v)
}

fn get_f64(id: AudioObjectID, selector: u32, scope: u32) -> Option<f64> {
    let a = addr(selector, scope);
    let mut v: f64 = 0.0;
    let mut size = 8u32;
    let r = unsafe { AudioObjectGetPropertyData(id, &a, 0, std::ptr::null(), &mut size, &mut v as *mut f64 as *mut c_void) };
    (r == 0).then_some(v)
}

fn get_range(id: AudioObjectID, selector: u32, scope: u32) -> Option<AudioValueRange> {
    let a = addr(selector, scope);
    let mut v = AudioValueRange::default();
    let mut size = std::mem::size_of::<AudioValueRange>() as u32;
    let r = unsafe { AudioObjectGetPropertyData(id, &a, 0, std::ptr::null(), &mut size, &mut v as *mut _ as *mut c_void) };
    (r == 0).then_some(v)
}

fn get_ranges(id: AudioObjectID, selector: u32, scope: u32) -> Vec<AudioValueRange> {
    let a = addr(selector, scope);
    let mut size = 0u32;
    if unsafe { AudioObjectGetPropertyDataSize(id, &a, 0, std::ptr::null(), &mut size) } != 0 || size == 0 {
        return vec![];
    }
    let n = size as usize / std::mem::size_of::<AudioValueRange>();
    let mut v = vec![AudioValueRange::default(); n];
    let r = unsafe { AudioObjectGetPropertyData(id, &a, 0, std::ptr::null(), &mut size, v.as_mut_ptr() as *mut c_void) };
    if r != 0 {
        return vec![];
    }
    v
}

fn get_u32s(id: AudioObjectID, selector: u32, scope: u32) -> Vec<u32> {
    let a = addr(selector, scope);
    let mut size = 0u32;
    if unsafe { AudioObjectGetPropertyDataSize(id, &a, 0, std::ptr::null(), &mut size) } != 0 || size == 0 {
        return vec![];
    }
    let mut v = vec![0u32; size as usize / 4];
    let r = unsafe { AudioObjectGetPropertyData(id, &a, 0, std::ptr::null(), &mut size, v.as_mut_ptr() as *mut c_void) };
    if r != 0 {
        return vec![];
    }
    v
}

fn get_string(id: AudioObjectID, selector: u32, scope: u32) -> Option<String> {
    let a = addr(selector, scope);
    if unsafe { AudioObjectHasProperty(id, &a) } == 0 {
        return None;
    }
    let mut cf: *const c_void = std::ptr::null();
    let mut size = std::mem::size_of::<*const c_void>() as u32;
    let r = unsafe { AudioObjectGetPropertyData(id, &a, 0, std::ptr::null(), &mut size, &mut cf as *mut _ as *mut c_void) };
    if r != 0 || cf.is_null() {
        return None;
    }
    let mut buf = vec![0 as c_char; 1024];
    let ok = unsafe { CFStringGetCString(cf, buf.as_mut_ptr(), buf.len() as isize, CF_UTF8) };
    unsafe { CFRelease(cf) };
    if ok == 0 {
        return None;
    }
    let s = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned();
    Some(s)
}

/// Channel count of a device on one scope, from its stream configuration.
fn channel_count(id: AudioObjectID, scope: u32) -> u32 {
    let a = addr(DEV_STREAM_CONFIG, scope);
    let mut size = 0u32;
    if unsafe { AudioObjectGetPropertyDataSize(id, &a, 0, std::ptr::null(), &mut size) } != 0 || size < 8 {
        return 0;
    }
    let mut raw = vec![0u8; size as usize];
    let r = unsafe { AudioObjectGetPropertyData(id, &a, 0, std::ptr::null(), &mut size, raw.as_mut_ptr() as *mut c_void) };
    if r != 0 {
        return 0;
    }
    // AudioBufferList { UInt32 mNumberBuffers; AudioBuffer mBuffers[1]; }
    let n = u32::from_ne_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
    let mut total = 0u32;
    let stride = std::mem::size_of::<AudioBuffer>();
    // The first buffer starts after the count, padded to pointer alignment.
    let base = std::mem::align_of::<AudioBuffer>().max(4);
    for i in 0..n {
        let off = base + i * stride;
        if off + 4 > raw.len() {
            break;
        }
        total += u32::from_ne_bytes([raw[off], raw[off + 1], raw[off + 2], raw[off + 3]]);
    }
    total
}

fn transport_name(t: u32) -> String {
    let s = match &t.to_be_bytes() {
        b"bltn" => "Built-in",
        b"aggr" => "Aggregate device",
        b"virt" => "Virtual",
        b"pci " => "PCI",
        b"usb " => "USB",
        b"fwir" => "FireWire",
        b"blue" => "Bluetooth",
        b"blea" => "Bluetooth LE",
        b"hdmi" => "HDMI",
        b"dprt" => "DisplayPort",
        b"airp" => "AirPlay",
        b"avb " => "AVB",
        b"thun" => "Thunderbolt",
        b"cont" => "Continuity Capture",
        b"ccwd" => "Continuity Capture (wired)",
        b"ccwl" => "Continuity Capture (wireless)",
        [0, 0, 0, 0] => "unknown (driver does not say)",
        _ => "",
    };
    if s.is_empty() {
        let b = t.to_be_bytes();
        format!("'{}'", String::from_utf8_lossy(&b))
    } else {
        s.to_string()
    }
}

/// Find the HAL device behind a PortAudio device.
pub fn find_device(info: &DeviceInfo) -> Option<AudioObjectID> {
    let ids = get_u32s(SYSTEM_OBJECT, HW_DEVICES, SCOPE_GLOBAL);
    let mut by_name: Vec<AudioObjectID> = Vec::new();
    for id in ids {
        if get_string(id, OBJ_NAME, SCOPE_GLOBAL).as_deref() == Some(info.name.as_str()) {
            by_name.push(id);
        }
    }
    if by_name.len() <= 1 {
        return by_name.first().copied();
    }
    by_name
        .iter()
        .copied()
        .find(|&id| {
            channel_count(id, SCOPE_INPUT) as i32 == info.max_input_channels && channel_count(id, SCOPE_OUTPUT) as i32 == info.max_output_channels
        })
        .or(by_name.first().copied())
}

/// The nominal sample rates the HAL lists for a device, as (min, max) ranges.
pub fn available_rates(info: &DeviceInfo) -> Option<Vec<(f64, f64)>> {
    let id = find_device(info)?;
    let r = get_ranges(id, DEV_AVAILABLE_RATES, SCOPE_GLOBAL);
    if r.is_empty() {
        None
    } else {
        Some(r.iter().map(|v| (v.min, v.max)).collect())
    }
}

/// The device's current buffer frame size.
pub fn buffer_frame_size(id: AudioObjectID) -> Option<u32> {
    get_u32(id, DEV_BUFFER_FRAMES, SCOPE_GLOBAL)
}

fn physical_format(id: AudioObjectID, scope: u32) -> Option<String> {
    let streams = get_u32s(id, DEV_STREAMS, scope);
    let sid = *streams.first()?;
    let a = addr(STREAM_PHYSICAL_FORMAT, SCOPE_GLOBAL);
    let mut d = AudioStreamBasicDescription::default();
    let mut size = std::mem::size_of::<AudioStreamBasicDescription>() as u32;
    let r = unsafe { AudioObjectGetPropertyData(sid, &a, 0, std::ptr::null(), &mut size, &mut d as *mut _ as *mut c_void) };
    if r != 0 {
        return None;
    }
    let kind = if d.format_flags & 1 != 0 {
        "float"
    } else if d.format_flags & 4 != 0 {
        "integer"
    } else {
        "unsigned"
    };
    let fmt = String::from_utf8_lossy(&d.format_id.to_be_bytes()).into_owned();
    let container = if d.channels_per_frame > 0 { d.bytes_per_frame * 8 / d.channels_per_frame } else { 0 };
    let mut s = format!("{} bit {kind}", d.bits_per_channel);
    if container > d.bits_per_channel {
        s.push_str(&format!(" in {container}"));
    }
    s.push_str(&format!(", {} ch, {} Hz", d.channels_per_frame, d.sample_rate));
    if fmt.trim() != "lpcm" {
        s.push_str(&format!(" ({fmt})"));
    }
    let lat = get_u32(sid, STREAM_LATENCY, SCOPE_GLOBAL);
    if let Some(l) = lat {
        s.push_str(&format!("; stream latency {l} frames"));
    }
    Some(s)
}

/// Append everything CoreAudio says about the device.
pub fn describe(info: &DeviceInfo, out: &mut Vec<KeyValue>, notes: &mut Vec<String>) {
    let Some(id) = find_device(info) else {
        notes.push("CoreAudio device not matched by name, so no HAL properties are shown.".into());
        return;
    };
    let mut push = |k: &str, v: String| out.push(KeyValue { key: k.into(), value: v });
    push("AudioDeviceID", id.to_string());
    if let Some(v) = get_string(id, DEV_UID, SCOPE_GLOBAL) {
        push("Device UID", v);
    }
    if let Some(v) = get_string(id, OBJ_MANUFACTURER, SCOPE_GLOBAL) {
        push("Manufacturer", v);
    }
    if let Some(v) = get_string(id, DEV_MODEL_UID, SCOPE_GLOBAL) {
        push("Model UID", v);
    }
    if let Some(t) = get_u32(id, DEV_TRANSPORT, SCOPE_GLOBAL) {
        push("Transport", transport_name(t));
    }
    if let Some(v) = get_u32(id, DEV_IS_ALIVE, SCOPE_GLOBAL) {
        push("Alive", if v != 0 { "yes".into() } else { "no".into() });
    }
    if let Some(v) = get_f64(id, DEV_NOMINAL_RATE, SCOPE_GLOBAL) {
        push("Nominal sample rate", format!("{v} Hz"));
    }
    let rates = get_ranges(id, DEV_AVAILABLE_RATES, SCOPE_GLOBAL);
    if !rates.is_empty() {
        let list: Vec<String> = rates
            .iter()
            .map(|r| if r.min == r.max { format!("{}", r.min) } else { format!("{}–{}", r.min, r.max) })
            .collect();
        push("Available sample rates", format!("{} Hz", list.join(", ")));
    }
    if let Some(v) = get_u32(id, DEV_BUFFER_FRAMES, SCOPE_GLOBAL) {
        push("Buffer frame size (now)", format!("{v} frames"));
    }
    if let Some(r) = get_range(id, DEV_BUFFER_RANGE, SCOPE_GLOBAL) {
        push("Buffer frame size range", format!("{}–{} frames", r.min, r.max));
    }
    for (scope, label) in [(SCOPE_INPUT, "Input"), (SCOPE_OUTPUT, "Output")] {
        let ch = channel_count(id, scope);
        if ch == 0 {
            continue;
        }
        if let Some(v) = get_u32(id, DEV_LATENCY, scope) {
            push(&format!("{label} latency (declared)"), format!("{v} frames"));
        }
        if let Some(v) = get_u32(id, DEV_SAFETY_OFFSET, scope) {
            push(&format!("{label} safety offset"), format!("{v} frames"));
        }
        if let Some(f) = physical_format(id, scope) {
            push(&format!("{label} physical format"), f);
        }
    }
    let sources = get_u32s(id, DEV_CLOCK_SOURCES, SCOPE_GLOBAL);
    if !sources.is_empty() {
        let cur = get_u32(id, DEV_CLOCK_SOURCE, SCOPE_GLOBAL);
        push(
            "Clock sources",
            format!("{} available{}", sources.len(), cur.map(|c| format!(", current id {c}")).unwrap_or_default()),
        );
    }
    if let Some(v) = get_u32(id, DEV_RUNNING_SOMEWHERE, SCOPE_GLOBAL) {
        push("In use by another process", if v != 0 { "yes".into() } else { "no".into() });
    }
    if let Some(pid) = get_u32(id, DEV_HOG_MODE, SCOPE_GLOBAL) {
        let pid = pid as i32;
        push("Hog mode", if pid == -1 { "free".into() } else { format!("held by pid {pid}") });
        if pid != -1 && pid != std::process::id() as i32 {
            notes.push(format!("Another process (pid {pid}) has this device in hog mode; streams will fail to open until it lets go."));
        }
    }
}
