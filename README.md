> **AI-assisted project.** This codebase was created with [Claude](https://claude.com/claude-code)
> (Anthropic), directed and reviewed by a human author.
> The measurements are covered by tests that run every analysis through a simulated loopback with
> known delay, gain, filtering and distortion, and the whole bench has been run end to end on
> macOS against a CoreAudio device. **Windows has only been compiled, not run**, so read
> [Status](#status) before you trust a number from an ASIO or WASAPI interface.

# SoundBench

**Put an audio interface on the bench.** Pick it, cable one of its outputs back to one of its
inputs, and SoundBench tells you what the driver claims and what the hardware actually does,
which aren't always on speaking terms:

- **Everything the driver says about it:** channels and their names, the sample rates it accepts,
  the buffer sizes it allows, the latency it declares, and on macOS the whole HAL property list,
  including transport, device UID, safety offsets, the physical stream format (the bit depth the
  converters are actually running) and whether another process holds it.
- **Round-trip latency:** a chirp is played and found again by cross-correlation, so the delay
  from output buffer to DAC to cable to ADC to input buffer is measured in samples of the
  stream's own clock. Next to it you get what the driver *reports*, and the difference between
  them, which is the latency a DAW compensating from the driver's number will get wrong.
- **Frequency response, phase and harmonic distortion** from one log sweep, by Farina's
  deconvolution: the linear response and each order of harmonic distortion fall out as separate
  impulse responses, so THD against frequency comes from the same ten seconds as the response.
- **THD+N, THD, SNR and SINAD** from a tone, measured the way AES17 does (20 Hz–20 kHz band,
  seven-term window), plus the tone's received frequency in ppm, which is the clock difference
  between two converters if there is one.
- **Noise floor**, unweighted and A-weighted, with the narrow peaks above it listed by frequency:
  hum, USB packet whine and switch-mode supplies each have a signature.
- **Transfer function and coherence** from pink or white noise: a second estimator with a
  different stimulus, and the coherence curve that tells you which part of the response to
  believe.
- **Buffer stability:** the interface is opened at each buffer size in turn and a tone is run
  through the loop. For each size you get whether the host flagged an underrun or overrun,
  whether the recorded tone has a discontinuity or a dropout in it, and the measured round-trip
  latency, which should grow with the buffer and gives the game away when a host has ignored the
  size it was asked for without saying so.

Every test is a report: Markdown for people, and JSON with every curve for machines.

![SoundBench's Buffer stability tab after a full run: nine buffer sizes, each clean, with measured and claimed latency](docs/screenshots/buffer-stability.png)

<sub>The Buffer stability tab after a full run against Pro Tools Audio Bridge, a virtual CoreAudio loopback rather than a hardware interface, which is why every size comes back clean and the round trip is exactly the buffer. Nine sizes from 16 to 4096 frames, each checked for xruns, for glitches in the recorded tone, and for the latency the driver claims against the one measured.</sub>

## Which drivers

SoundBench is a desktop app built on a vendored [PortAudio](http://www.portaudio.com), compiled
from source so the build carries every host API it can reach rather than whatever a system
library happened to include:

| Platform | Host APIs |
| --- | --- |
| macOS | CoreAudio |
| Windows | WASAPI (shared and exclusive), WDM-KS, DirectSound, MME — and **ASIO** when built with the `asio` feature |
| Linux | ALSA |

On CoreAudio the buffer size and sample rate chosen in the app are applied to the device itself
(the same setting a DAW's I/O buffer size changes), and the app fails rather than resampling if
the device can't run at the rate asked for. On ASIO it reads the driver's own buffer size list
and offers that. On WASAPI, exclusive mode is on by default, so you're measuring the device and
not the Windows mixer.

That's why it isn't a browser tool. A web page only sees the browser's audio stack (CoreAudio on
a Mac, shared-mode WASAPI on Windows, never ASIO) and can't set a device's buffer size at all.

## Running it

Builds for macOS (universal), Windows and Linux go on the
[releases page](https://github.com/stoatworks-labs/soundbench/releases) when a version is
tagged, and none has been yet. Until then, build it yourself:

```bash
npm install
npm run app          # tauri dev: vite + cargo, opens the window
npm run app:build    # release bundle for this platform
```

You need Rust stable, Node 22 and CMake (CMake builds PortAudio). On Linux you also need
`libasound2-dev` and the webkit2gtk stack Tauri needs, and on Windows the MSVC toolchain. For
ASIO, add `-- --features asio` to either command. [docs/asio.md](docs/asio.md) explains what that
involves, and why the published Windows build won't carry it until Steinberg's agreement is
signed.

There's also a command-line bench that drives the device layer without the app:

```bash
cd src-tauri
cargo run -p soundbench-audio --example bench -- list
cargo run -p soundbench-audio --example bench -- latency <in> <out> 48000 128
cargo run -p soundbench-audio --example bench -- stability <in> <out> 48000 32,64,128,256
```

## Using it

1. Loop an output back to an input. Analogue is fine (that's the idea, since it measures the
   converters), but keep the interface's own gain moderate. The test signals sit at −12 dBFS
   (the tone at −1 dBFS, as AES17 specifies), and a hot input stage clips before the ADC does.
2. Pick the audio API, the output and the input, and which channels carry the signal and listen
   for it. Several input channels can be recorded at once, and each gets its own set of results.
3. Choose a sample rate and a buffer size. Only the ones the driver accepts are offered.
4. Run one test, or **Run the whole bench**. Results stay until the device or the rate changes.
5. **Report** gathers everything measured on this setup, to save or copy.

Each tab explains what its numbers mean, and the [user guide](docs/USER-GUIDE.md) goes further.

## Status

- **Measurements:** every analysis in `soundbench-dsp` has tests against a simulated loopback
  (delay, gain, one-pole filter, cubic and quadratic distortion, noise, inversion) that check the
  right numbers come out. There are 36 of them.
- **macOS:** the whole bench has run end to end through a CoreAudio loopback device: latency
  reads exactly three buffers at every buffer size from 16 to 4096, the response is flat to
  0.01 dB, THD reads its floor, and the stability sweep is clean at every size. The HAL
  property list has been read on a Yamaha DM3 (USB) and on virtual devices. No physical
  loopback cable has been through it yet (the DM3 didn't have one connected), so the analogue
  path hasn't been tried on real converters.
- **Windows:** compiles in CI with ASIO on. **Not run against hardware.** The WASAPI exclusive
  path, the WDM-KS path and the ASIO buffer-size handling are PortAudio's, and nothing here has
  checked what they do on a real interface yet.
- **Linux:** compiles in CI. Not run.

## How it measures

**Latency.** Five 40 ms linear chirps, played at known frame positions. In a duplex PortAudio
callback the input and output buffers cover the same frames, so the frame where a chirp was
written and the frame where it's found (by cross-correlation, refined to a fraction of a sample
by parabolic interpolation) differ by exactly the round trip, buffers included. The sign of the
peak gives polarity. PortAudio's own `inputLatency + outputLatency` for the stream is shown
beside it, and the difference is the *unreported* latency.

**Sweep.** An exponential sine sweep (10 Hz–22 kHz, 10 s, −12 dBFS), deconvolved with its inverse
filter. The linear impulse response is windowed and transformed against the same sweep through
a perfect wire, which cancels the sweep's own band-edge shape and fades. The `k`-th harmonic's
impulse response arrives `L·ln k` earlier (with `L` the sweep rate constant). Each is windowed
out and transformed, and read at `k·f` to give the `k`-th harmonic relative to the fundamental
at `f`. It's left blank in the first octave above the sweep's start, where the fade-in still
colours things.

**Tone.** One FFT of the steady state under Albrecht's seven-term cosine window (sidelobes below
−180 dB, so the window's own leakage stays under any converter's noise). Powers are lobe sums
(±8 bins), and the band is 20 Hz to 20 kHz. The tone is 997 Hz so the harmonics don't sit on bin
boundaries.

**Stability.** For each buffer size: open, play a chirp then a tone, close. The chirp gives the
round-trip latency at that size. The tone is fitted, window by window, with a sine at the known
frequency, and the residual is watched for spikes (a discontinuity) and for the fitted amplitude
collapsing (a dropout). The callback's own bookkeeping (PortAudio's underflow and overflow flags,
and the interval between callbacks) is reported alongside, and a size only counts as *clean*
when all three agree.

## Layout

```
src/                      React + TypeScript UI (Vite)
src-tauri/
  src/lib.rs              the Tauri layer: list, run one test at a time, save a report
  crates/pa-sys/          PortAudio bindings + the vendored PortAudio source, built by cmake
  crates/soundbench-dsp/  signals and analyses, pure, tested
  crates/soundbench-audio/ hosts, devices, the duplex runner, the test sequences
```

`AGENTS.md` explains the model and the traps, and `CLAUDE.md` is the command reference.

<!-- attributions:start -->
This project is built on other people's work — see [ATTRIBUTIONS.md](ATTRIBUTIONS.md).
<!-- attributions:end -->

## Licence

MIT. PortAudio is MIT-licensed and vendored under `src-tauri/crates/pa-sys/vendor/portaudio`
with its licence; see `ATTRIBUTIONS.md`. ASIO is a trademark of Steinberg Media Technologies
GmbH, and a build with ASIO support links their SDK — see `docs/asio.md`.
