# Attributions

SoundBench is built on other people's work. This file lists what that work is, who did
it, and what it is doing here.

It is generated — the master lists live in the `stoatworks-backend` repo and are
pushed out by `scripts/sync-attributions.py`. Edit it there, not here.

## Third-party code this project uses

Libraries, SDKs and frameworks the project is built on or bundles.

### PortAudio

<http://www.portaudio.com>  
Licence: MIT (PortAudio Portable Real-Time Audio Library licence)  
Copyright: 1999-2006 Ross Bencina and Phil Burk

Vendored under src-tauri/crates/pa-sys/vendor/portaudio as a pinned snapshot of upstream master (a4dbf68c, 2026-09-04), trimmed to sources, headers and CMake files, and built from source by the crate's build.rs.

The whole device layer: one callback that reaches CoreAudio, ASIO, WASAPI, WDM-KS, DirectSound, MME and ALSA, and whose duplex form puts the recording on the same frame clock as the signal played.

### Tauri

<https://tauri.app>  
Licence: MIT or Apache-2.0  
Copyright: The Tauri Programme within The Commons Conservancy

A Cargo and npm dependency — of the app itself under src-tauri/, or of the desktop launcher under launcher/src-tauri/.

Wraps a web front end in a native desktop app using the platform's own webview rather than a bundled browser, so the binary stays small.

### React

<https://react.dev>  
Licence: MIT  
Copyright: Meta Platforms, Inc. and affiliates

An npm dependency.

The UI layer for the browser tools and the Electron and Tauri front ends.

### The Rust crate ecosystem

<https://crates.io>  
Licence: predominantly MIT or Apache-2.0  
Copyright: the individual crate authors

Cargo dependencies, resolved and pinned in Cargo.lock.

Async runtimes, protocol codecs, serialisation and GUI toolkits. The exact set and versions for any build are in that repo's Cargo.lock, which is the authoritative list.

### The npm ecosystem

<https://www.npmjs.com>  
Licence: predominantly MIT  
Copyright: the individual package authors

npm dependencies, resolved and pinned in the lockfile.

Build tooling, test runners and the libraries the front ends are assembled from. The exact set and versions for any build are in that repo's lockfile, which is the authoritative list.

The full transitive dependency set for any build is pinned in this repo's lockfile,
which is the authoritative list. What is named above is the layers a reader would
want to know about, not every package that has ever been resolved.

## Standards and published specifications

What the implementation is measured against.

- **Angelo Farina, "Simultaneous measurement of impulse response and distortion with a swept-sine technique" (AES 108th Convention, 2000)** — The log-sweep deconvolution the response, phase and THD-against-frequency measurements use: the harmonic distortion of order k lands L·ln(k) before the linear impulse response and is windowed out separately.
- **AES17** — The 20 Hz–20 kHz band and the −1 dBFS level for THD+N.
- **Hans-Helge Albrecht, "A family of cosine-sum windows for high-resolution measurements" (ICASSP 2001)** — The seven-term window under the tone analysis, whose sidelobes sit below −180 dB.
- **IEC 61672** — The A-weighting curve applied to the noise floor.
- **C. H. Knapp and G. C. Carter, "The generalized correlation method for estimation of time delay" (IEEE Trans. ASSP, 1976)** — The phase transform (GCC-PHAT) used to align the noise stimulus with its recording.

## Getting this wrong

If your work is here and the description is inaccurate, the licence is wrong, or you would rather not be listed — open an issue and it will be fixed.
