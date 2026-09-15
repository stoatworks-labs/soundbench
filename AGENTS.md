# AGENTS.md — bringing an LLM up to speed on SoundBench

Orientation for an AI assistant (or a new human) picking this project up cold. `CLAUDE.md`
holds the short command reference; this file explains the model and the traps.

## 1. What this is

A desktop bench for audio interfaces: it plays test signals out of one port, records them back
through another, and turns the recording into numbers — latency, response, distortion, noise,
and which buffer sizes run clean. Tauri v2, React front end, Rust core, a vendored PortAudio for
the drivers. No network, no accounts, no telemetry; the only file it writes unasked is its own
settings.

It is a desktop app and not a browser tool for one reason: a browser cannot reach ASIO, cannot
set a device's buffer size, and on Windows only ever sees the shared-mode mixer. The whole
point is the driver.

## 2. Layout

```
src/
  types.ts                the JSON shapes shared with Rust — read this first
  store.ts                zustand: setup, options, results, running state, settings persistence
  lib/ipc.ts              every invoke/listen in one place
  lib/report.ts           the Markdown and JSON report
  components/Setup.tsx    the left column
  components/*Tab.tsx     one per test, plus Device and Report
  components/Plot.tsx     the one canvas plot everything uses
src-tauri/
  src/lib.rs              Tauri commands: startup/rescan, device_detail, run_test, settings, save
  crates/pa-sys/          hand-written PortAudio FFI; build.rs compiles vendor/portaudio with cmake
  crates/soundbench-dsp/  signal.rs (generators), latency.rs, response.rs (sweep), tone.rs,
                          noise.rs, transfer.rs, stability.rs, fft.rs, window.rs
  crates/soundbench-audio/
    lib.rs                Engine: hosts(), devices(), detail() — plus mac.rs / win.rs extras
    stream.rs             THE RUNNER: one duplex pass, real-time-safe callback
    bench.rs              the tests: build a signal, run a pass, analyse
    examples/bench.rs     command-line driver for all of it
```

## 3. Invariants

- **The audio callback allocates nothing, locks nothing, blocks nothing.** Everything it
  touches is allocated before `Pa_OpenStream`. It copies, counts and returns. The main thread
  collects the buffers only after the stream is closed. Anything that breaks this is a glitch
  in the very measurement that looks for glitches.
- **The output and the recording share one frame clock.** Frame `k` written to the output and
  frame `k` read from the input are the same nominal instant, so a delay found by correlation
  is the round trip with no calibration. Never resample or re-time either buffer.
- **0 dBFS is a full-scale sine.** Levels everywhere (`signal::amplitude`, `dbfs_rms`, the
  tone's `level_dbfs`) use that convention, so a −12 dBFS tone through a unity path reads −12.
- **Every analysis is a pure function with a loopback test.** `soundbench_dsp::testutil::Loopback`
  simulates delay, gain, a one-pole filter, cubic and quadratic distortion, noise and inversion;
  a new analysis gets a test that runs through it and checks the known answer.
- **Rust decides, TypeScript displays.** The UI never computes a measurement; it picks rows out
  of the report and draws them. If a number appears in the UI that is not in `types.ts`, it
  came from the wrong side.
- **One test at a time.** `AppState.running` is a hard lock; two streams on one interface would
  measure each other.
- **A device is remembered by name, not index.** PortAudio indices change with every rescan and
  every plug event; `SavedSetup` stores names and host kind and `applySaved` looks them up.

## 4. Traps

- `Pa_IsFormatSupported` with `paMacCoreChangeDeviceParameters` **sets the sample rate on the
  device and waits for it to take**, several seconds per rate the device lacks. `detail()`
  reads `kAudioDevicePropertyAvailableNominalSampleRates` instead on CoreAudio and probes with
  the change flag off elsewhere. The CLI hung on a Yamaha DM3 before this was found.
- PortAudio's `unsigned long` is 32-bit on Windows. `pa-sys` uses `c_ulong` everywhere for it;
  the ASIO buffer-size functions take `long*`, which is `c_long`. Do not "clean this up" to u64.
- PortAudio does not expose the `AudioDeviceID` behind a device index, so `mac::find_device`
  matches by name and channel counts. Two virtual devices with the same name and shape will
  resolve to the first.
- The seven-term window's main lobe is ±7 bins; `tone::LOBE = 8`. With the four-term
  Blackman-Harris the THD+N floor was −88 dB, which is worse than a real converter.
- The sweep's THD is blanked below `2·f1`; the harmonic products of the first octave land where
  the fade-in still shapes the response and read as 1–8 % distortion on a perfect wire.
- The parabolic refinement of an impulse-response peak is biased on a minimum-phase response
  (a one-pole filter gave +0.3 samples). The reported `delay_samples` includes it, the phase is
  presented with the same fraction removed, so the two agree with each other and with the
  filter once it is put back — the test in `response.rs` pins exactly this.
- WKWebView stops painting when its window is occluded, so a screenshot of the app taken while
  another app is in front shows the last frame, not the state. The accessibility tree stays
  live: read it (System Events `entire contents of window 1`) to see what the DOM says.
- The computer-use `app_click` raw input does not reach buttons inside the webview. System
  Events `click` on the AX button does; HTML `<select>` popups are native menus that System
  Events cannot walk — restore a setup through `settings.json` instead.
- The CoreAudio device the app opens gets its buffer size and nominal rate changed. That is
  the feature, but another application using the same device at the time will hear it.
- `PaMacCore_GetBufferSizeRange` reports the *device's* range; the DM3 says 29–4096. The
  candidates list keeps the reported minimum even when it is not a power of two.

## 5. Verifying

- `cargo test --workspace` in `src-tauri` runs the DSP suite and PortAudio's initialisation.
- `cargo run -p soundbench-audio --example bench -- list` shows what PortAudio sees; the
  `latency`, `sweep`, `tone`, `noise`, `transfer` and `stability` subcommands run a test on two
  device indices. On this Mac the Avid "Pro Tools Audio Bridge" virtual devices loop output to
  input bit-exactly and are the fastest way to prove the pipeline: latency must read exactly
  three buffers, the sweep must be flat to 0.01 dB.
- For the app: write `~/Library/Application Support/com.allansargeant.soundbench/settings.json`
  with the device names, launch the bundle, drive the buttons with System Events, and read the
  results from the AX tree or from a saved report.
