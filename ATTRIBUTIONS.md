# Attributions

SoundBench is built on other people's work. This file lists what that work is, who did
it, and what it is doing here.

## Third-party code this project uses

### PortAudio

<http://www.portaudio.com>  
Licence: MIT (PortAudio Portable Real-Time Audio Library)  
Copyright: 1999–2024 Ross Bencina and Phil Burk

Vendored at `src-tauri/crates/pa-sys/vendor/portaudio`, a snapshot of upstream master at
commit `a4dbf68c51fd32734f32657db64849aac24739e7` (2026-09-04), trimmed to the sources,
headers and CMake files (no bindings, tests, examples or documentation), and built from
source by `pa-sys/build.rs`.

The whole device layer. PortAudio is what reaches CoreAudio, ASIO, WASAPI, WDM-KS,
DirectSound, MME and ALSA through one callback, and its duplex callback is what puts the
recording on the same frame clock as the signal played.

### Steinberg ASIO SDK

<https://www.steinberg.net/developers/>  
Licence: Steinberg ASIO SDK Licensing Agreement  
Copyright: Steinberg Media Technologies GmbH

Not included in this repository. A Windows build with the `asio` feature has PortAudio's
CMake download it at configure time and link it. ASIO is a trademark and software of
Steinberg Media Technologies GmbH. See docs/asio.md.

### Tauri

<https://tauri.app>  
Licence: MIT or Apache-2.0  
Copyright: The Tauri Programme within The Commons Conservancy

A Cargo and npm dependency. Puts the web front end on the Rust core in the platform's own
webview.

### React

<https://react.dev>  
Licence: MIT  
Copyright: Meta Platforms, Inc. and affiliates

An npm dependency. The UI layer.

### rustfft and realfft

<https://github.com/ejmahler/RustFFT> · <https://github.com/HEnquist/realfft>  
Licence: MIT or Apache-2.0  
Copyright: Elliott Mahler; Henrik Enquist

Cargo dependencies. Every transform in the DSP crate.

### The Rust and npm ecosystems

Libraries resolved and pinned in `src-tauri/Cargo.lock` and `package-lock.json`, which are
the authoritative lists.

## Methods and published specifications

- **Angelo Farina, "Simultaneous measurement of impulse response and distortion with a
  swept-sine technique"** (AES 108th Convention, 2000) — the log-sweep deconvolution the
  response and THD-against-frequency measurements use.
- **AES17** — the measurement band and the −1 dBFS level for THD+N.
- **Hans-Helge Albrecht, "A family of cosine-sum windows for high-resolution measurements"**
  (ICASSP 2001) — the seven-term window under the tone analysis.
- **IEC 61672** — the A-weighting curve.
- **Charles Knapp and G. Clifford Carter, "The generalized correlation method for estimation
  of time delay"** (1976) — the phase transform used to align the noise stimulus.

## Getting this wrong

If your work is here and the description is inaccurate, the licence is wrong, or you would
rather not be listed — open an issue and it will be fixed.
