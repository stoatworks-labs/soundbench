//! SoundBench — the Tauri layer.
//!
//! Thin on purpose. Everything that decides a number lives in
//! `soundbench-dsp`, everything that touches a driver lives in
//! `soundbench-audio`, and this crate only wires them to a window: list what
//! is there, run one test at a time on a worker thread, stream progress
//! events back, and write the report file the user asked for.
//!
//! One test at a time is a rule, not a limitation: two streams on one
//! interface would measure each other.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use soundbench_audio::bench::{self, Progress, ProgressEvent, Target};
use soundbench_audio::{DeviceDetail, DeviceInfo, Engine, HostInfo};
use tauri::{AppHandle, Emitter, Manager, State};

struct AppState {
    engine: Mutex<Option<Engine>>,
    cancel: Arc<AtomicBool>,
    running: AtomicBool,
}

/// Everything the UI needs on startup or after a rescan.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    portaudio: String,
    platform: String,
    hosts: Vec<HostInfo>,
    devices: Vec<DeviceInfo>,
    asio_built_in: bool,
}

fn platform_name() -> String {
    let os = match std::env::consts::OS {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        o => o,
    };
    format!("{os} {}", std::env::consts::ARCH)
}

fn snapshot(engine: &Engine) -> Snapshot {
    Snapshot {
        portaudio: Engine::version(),
        platform: platform_name(),
        hosts: engine.hosts(),
        devices: engine.devices(),
        asio_built_in: cfg!(feature = "asio"),
    }
}

/// Take the engine, making one if this is the first call.
fn with_engine<T>(state: &AppState, f: impl FnOnce(&Engine) -> Result<T, String>) -> Result<T, String> {
    let mut guard = state.engine.lock().map_err(|_| "the audio engine lock is poisoned; restart SoundBench".to_string())?;
    if guard.is_none() {
        *guard = Some(Engine::new().map_err(|e| e.to_string())?);
    }
    f(guard.as_ref().unwrap())
}

#[tauri::command]
fn startup(state: State<AppState>) -> Result<Snapshot, String> {
    with_engine(&state, |e| Ok(snapshot(e)))
}

/// Throw the PortAudio instance away and make a new one, so a device plugged
/// in since the app started appears. Refused while a test is running.
#[tauri::command]
fn rescan(state: State<AppState>) -> Result<Snapshot, String> {
    if state.running.load(Ordering::SeqCst) {
        return Err("a test is running; wait for it or cancel it first".into());
    }
    let mut guard = state.engine.lock().map_err(|_| "engine lock poisoned".to_string())?;
    *guard = None;
    *guard = Some(Engine::new().map_err(|e| e.to_string())?);
    Ok(snapshot(guard.as_ref().unwrap()))
}

#[tauri::command]
fn device_detail(state: State<AppState>, index: i32) -> Result<DeviceDetail, String> {
    with_engine(&state, |e| e.detail(index).map_err(|e| e.to_string()))
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressPayload {
    kind: String,
    phase: String,
    fraction: f64,
    message: String,
}

/// Run one test. `kind` picks the test, `options` is that test's option
/// struct as JSON (missing fields take their defaults), and the result is
/// that test's report as JSON. Runs on a blocking thread; progress arrives
/// as `soundbench://progress` events.
#[tauri::command]
async fn run_test(app: AppHandle, kind: String, target: Target, options: serde_json::Value) -> Result<serde_json::Value, String> {
    let state = app.state::<AppState>();
    if state.running.swap(true, Ordering::SeqCst) {
        return Err("a test is already running".into());
    }
    state.cancel.store(false, Ordering::SeqCst);
    let cancel = state.cancel.clone();
    let app2 = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let state = app2.state::<AppState>();
        let kind_for_progress = kind.clone();
        let app3 = app2.clone();
        let mut report = move |e: ProgressEvent| {
            let _ = app3.emit(
                "soundbench://progress",
                ProgressPayload { kind: kind_for_progress.clone(), phase: e.phase, fraction: e.fraction, message: e.message },
            );
        };
        let mut progress = Progress { cancel: &cancel, report: &mut report };
        let value = with_engine(&state, |engine| {
            let out = match kind.as_str() {
                "latency" => {
                    let o: bench::LatencyOptions = serde_json::from_value(options).map_err(|e| e.to_string())?;
                    serde_json::to_value(bench::latency_test(engine, &target, &o, &mut progress).map_err(|e| e.to_string())?)
                }
                "sweep" => {
                    let o: bench::SweepOptions = serde_json::from_value(options).map_err(|e| e.to_string())?;
                    serde_json::to_value(bench::sweep_test(engine, &target, &o, &mut progress).map_err(|e| e.to_string())?)
                }
                "tone" => {
                    let o: bench::ToneOptions = serde_json::from_value(options).map_err(|e| e.to_string())?;
                    serde_json::to_value(bench::tone_test(engine, &target, &o, &mut progress).map_err(|e| e.to_string())?)
                }
                "noise" => {
                    let o: bench::NoiseOptions = serde_json::from_value(options).map_err(|e| e.to_string())?;
                    serde_json::to_value(bench::noise_test(engine, &target, &o, &mut progress).map_err(|e| e.to_string())?)
                }
                "transfer" => {
                    let o: bench::TransferOptions = serde_json::from_value(options).map_err(|e| e.to_string())?;
                    serde_json::to_value(bench::transfer_test(engine, &target, &o, &mut progress).map_err(|e| e.to_string())?)
                }
                "stability" => {
                    let o: bench::StabilityOptions = serde_json::from_value(options).map_err(|e| e.to_string())?;
                    serde_json::to_value(bench::stability_test(engine, &target, &o, &mut progress).map_err(|e| e.to_string())?)
                }
                other => return Err(format!("unknown test '{other}'")),
            };
            out.map_err(|e| e.to_string())
        });
        state.running.store(false, Ordering::SeqCst);
        value
    })
    .await
    .map_err(|e| format!("the test thread failed: {e}"))?;
    result
}

#[tauri::command]
fn cancel_test(state: State<AppState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

#[tauri::command]
fn is_running(state: State<AppState>) -> bool {
    state.running.load(Ordering::SeqCst)
}

/// Write a report the dialog plugin has already chosen a path for.
#[tauri::command]
fn save_text(path: String, text: String) -> Result<(), String> {
    std::fs::write(&path, text).map_err(|e| format!("could not write {path}: {e}"))
}

fn settings_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    Ok(dir.join("settings.json"))
}

/// The last setup, as the UI saved it. The shape is the UI's; this side only
/// keeps the file.
#[tauri::command]
fn load_settings(app: AppHandle) -> Result<Option<serde_json::Value>, String> {
    let path = settings_path(&app)?;
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map(Some).map_err(|e| format!("{} is not valid JSON: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("could not read {}: {e}", path.display())),
    }
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: serde_json::Value) -> Result<(), String> {
    let path = settings_path(&app)?;
    let text = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("could not write {}: {e}", path.display()))
}

/// The ASIO driver's own control panel, where buffer size and sample rate
/// often have to be set for drivers that ignore the host's request.
#[tauri::command]
fn show_asio_panel(state: State<AppState>, device: i32) -> Result<(), String> {
    show_asio_panel_impl(&state, device)
}

#[cfg(all(target_os = "windows", feature = "asio"))]
fn show_asio_panel_impl(state: &AppState, device: i32) -> Result<(), String> {
    with_engine(state, |_| soundbench_audio::win::show_asio_control_panel(device).map_err(|e| e.to_string()))
}

#[cfg(not(all(target_os = "windows", feature = "asio")))]
fn show_asio_panel_impl(_state: &AppState, _device: i32) -> Result<(), String> {
    Err("this build has no ASIO support".into())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { engine: Mutex::new(None), cancel: Arc::new(AtomicBool::new(false)), running: AtomicBool::new(false) })
        .invoke_handler(tauri::generate_handler![startup, rescan, device_detail, run_test, cancel_test, is_running, save_text, show_asio_panel, load_settings, save_settings])
        .run(tauri::generate_context!())
        .expect("error while running SoundBench");
}
