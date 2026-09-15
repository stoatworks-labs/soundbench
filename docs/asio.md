# ASIO

ASIO is Windows-only, and SoundBench's release build for Windows includes it. Building it
needs Steinberg's ASIO SDK; using it in a product needs Steinberg's agreement.

## What the build does

With `--features asio`, `pa-sys/build.rs` turns on `PA_USE_ASIO` in the vendored
PortAudio's CMake, which downloads `asiosdk.zip` from <https://www.steinberg.net/asiosdk>
into the build directory (PortAudio's own logic — see its `CMakeLists.txt`), extracts it,
and compiles `pa_asio.cpp` against it. Nothing from the SDK is checked into this
repository, and the SDK is never redistributed; only the resulting binary carries code
compiled from it.

To use a copy you already have — or to build offline — point `ASIO_SDK_ZIP_PATH` at the
zip:

```powershell
$env:ASIO_SDK_ZIP_PATH = "C:\sdk\asiosdk_2.3.3_2019-06-14.zip"
npm run app:build -- --features asio
```

The MSVC toolchain is required (`x86_64-pc-windows-msvc`); the ASIO host code is C++ and
`build.rs` links `msvcprt` for it.

## The licence

Steinberg's ASIO SDK Licensing Agreement allows the SDK to be downloaded and used free of
charge, forbids redistributing the SDK itself, and requires the developer of a product that
uses ASIO to have signed the agreement with Steinberg. "ASIO" is a trademark and software of
Steinberg Media Technologies GmbH.

**Before publishing a Windows release with ASIO on, the ASIO SDK Licensing Agreement must
have been signed and returned to Steinberg.** The CI workflow builds with the feature on
regardless; the agreement is a matter of paperwork, not code, and it is the maintainer's to
do. Until then, build Windows releases without the feature (remove `--features asio` from
`.github/workflows/desktop.yml`'s Windows row) — WASAPI exclusive mode and WDM-KS still
give a direct path to the hardware.

## What to expect from an ASIO interface

- **One application at a time.** Most ASIO drivers are exclusive. Close the DAW.
- **The driver's buffer sizes are the only ones.** `PaAsio_GetAvailableBufferSizes` reports
  the minimum, maximum, preferred size and granularity, and the Buffer picker offers exactly
  those. Some drivers ignore the host's request and use what their control panel says —
  the **Open the ASIO control panel** button on the Device tab is for those, and the
  measured round-trip latency on the Buffer stability tab shows whether a size took effect.
- **The sample rate may be the driver's too.** PortAudio asks; some drivers refuse anything
  but the panel setting.
- **Channel names come from the driver** (`ASIOGetChannelInfo`), so the channel chips read
  "Analog In 1" rather than "Channel 1".

## Status

Compiled and linked in CI. **Not run against an ASIO device.** The development Mac's
Windows VM is ARM64, for which ASIO drivers barely exist, so the first real ASIO
measurement will be on a user's machine. Read the numbers with that in mind and report what
you see.
