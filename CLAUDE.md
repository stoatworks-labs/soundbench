# CLAUDE.md — SoundBench

Command reference. For the model, the invariants and the traps, read
[AGENTS.md](AGENTS.md) first.

## Commands

```bash
npm install
npm run app          # tauri dev: vite on :5174 + cargo, opens the window
npm run app:build    # release bundle for this platform (src-tauri/target/release/bundle)
npm test             # vitest — the format and report helpers
npm run lint         # oxlint
npm run typecheck    # tsc -b
cd src-tauri && cargo test --workspace            # DSP suite + PortAudio initialises
cd src-tauri && cargo run -p soundbench-audio --example bench -- list
cd src-tauri && cargo run -p soundbench-audio --example bench -- latency <in> <out> 48000 128
```

Windows with ASIO: append `-- --features asio` to the tauri commands, or
`--features asio` to cargo. See docs/asio.md.

## Release

`.github/workflows/desktop.yml` builds on a `v*` tag: macOS universal (unsigned — the fleet's
autosign agent notarises after publishing), Linux deb/rpm, Windows NSIS with ASIO. Bump the
version in `package.json`, `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml` together.

## Verifying against a device

The `bench` example drives the whole device layer from the terminal; on this Mac the Avid
"Pro Tools Audio Bridge" virtual devices loop output to input bit-exactly. Latency must read
exactly three buffers; the sweep flat to 0.01 dB; the stability sweep clean at every size.

For the app, seed `~/Library/Application Support/com.allansargeant.soundbench/settings.json`
with the device names (see `SavedSetup` in `src/types.ts`), open the bundle, and click buttons
by name with System Events (`entire contents of window 1`, `class of e is button`). The
computer-use tools' raw clicks do not reach the webview, and its screenshots go stale when
the window is occluded — the accessibility tree does not.
