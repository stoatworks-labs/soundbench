//! Builds the vendored PortAudio as a static library and tells cargo how to
//! link it.
//!
//! PortAudio is vendored (`vendor/portaudio`, a pinned snapshot of upstream
//! master — see ATTRIBUTIONS.md for the commit) rather than found on the
//! system, because the whole point of this app is which host APIs the build
//! carries. A distro PortAudio is built with whatever that distro chose; this
//! one is built with exactly the set below, on every platform, every time.
//!
//! Host APIs per platform:
//!
//!   macOS    CoreAudio
//!   Windows  WASAPI, WDM-KS, DirectSound, MME — and ASIO with `--features asio`
//!   Linux    ALSA
//!
//! JACK and PulseAudio are switched off deliberately. PortAudio links their
//! client libraries directly, so a build that includes them will not start on a
//! machine that lacks them, and both are reachable through ALSA on a PipeWire
//! desktop anyway.

use std::env;
use std::path::PathBuf;

fn main() {
    let vendor = PathBuf::from("vendor/portaudio");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=vendor/portaudio/CMakeLists.txt");
    println!("cargo:rerun-if-changed=vendor/portaudio/src");
    println!("cargo:rerun-if-changed=vendor/portaudio/include");
    println!("cargo:rerun-if-env-changed=ASIO_SDK_ZIP_PATH");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let asio = env::var("CARGO_FEATURE_ASIO").is_ok() && target_os == "windows";

    let mut cfg = cmake::Config::new(&vendor);
    cfg.define("PA_BUILD_SHARED_LIBS", "OFF")
        .define("PA_BUILD_TESTS", "OFF")
        .define("PA_BUILD_EXAMPLES", "OFF")
        .define("PA_USE_SKELETON", "OFF")
        .define("PA_USE_JACK", "OFF")
        .define("PA_USE_PULSEAUDIO", "OFF")
        .define("PA_USE_OSS", "OFF")
        .define("PA_USE_SNDIO", "OFF")
        .define("CMAKE_INSTALL_LIBDIR", "lib")
        // PortAudio's own CMake floor is 3.10, which CMake 4 refuses to
        // configure; this is the same floor PortAudio's master branch
        // builds with.
        .define("CMAKE_POLICY_VERSION_MINIMUM", "3.10");

    match target_os.as_str() {
        "macos" => {
            // Cargo builds one architecture at a time (Tauri's universal
            // bundle is two cargo builds lipo'd together), so PortAudio is
            // compiled for that one architecture and nothing else.
            let arch = match target_arch.as_str() {
                "aarch64" => "arm64",
                other => other,
            };
            cfg.define("CMAKE_OSX_ARCHITECTURES", arch);
            let min = env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "10.15".into());
            cfg.define("CMAKE_OSX_DEPLOYMENT_TARGET", min);
        }
        "windows" => {
            cfg.define("PA_USE_WASAPI", "ON")
                .define("PA_USE_WDMKS", "ON")
                .define("PA_USE_WDMKS_DEVICE_INFO", "ON")
                .define("PA_USE_DS", "ON")
                .define("PA_USE_WMME", "ON")
                .define("PA_USE_ASIO", if asio { "ON" } else { "OFF" });
            if asio {
                // PortAudio's CMake downloads the SDK from Steinberg when it
                // is not already present; a local copy of the zip skips the
                // download (and lets CI cache it).
                if let Ok(zip) = env::var("ASIO_SDK_ZIP_PATH") {
                    cfg.define("ASIO_SDK_ZIP_PATH", zip);
                }
            }
        }
        _ => {
            cfg.define("PA_USE_ALSA", "ON");
        }
    }

    let dst = cfg.build();
    println!("cargo:rustc-link-search=native={}", dst.join("lib").display());
    println!("cargo:rustc-link-lib=static=portaudio");

    match target_os.as_str() {
        "macos" => {
            for fw in ["CoreAudio", "AudioToolbox", "AudioUnit", "CoreFoundation", "CoreServices"] {
                println!("cargo:rustc-link-lib=framework={fw}");
            }
        }
        "windows" => {
            for lib in ["winmm", "ole32", "uuid", "setupapi", "dsound", "advapi32", "user32"] {
                println!("cargo:rustc-link-lib={lib}");
            }
            if asio && target_env == "msvc" {
                // The ASIO SDK is C++ (operator new, and a handful of
                // std:: calls in asiolist.cpp); Rust's MSVC target links the
                // C runtime but not the C++ one.
                println!("cargo:rustc-link-lib=msvcprt");
            }
        }
        _ => {
            println!("cargo:rustc-link-lib=asound");
            println!("cargo:rustc-link-lib=pthread");
            println!("cargo:rustc-link-lib=m");
        }
    }
}
