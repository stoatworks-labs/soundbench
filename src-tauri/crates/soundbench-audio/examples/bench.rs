//! Command-line bench, for checking the device layer without the app.
//!
//!   cargo run -p soundbench-audio --example bench -- list
//!   cargo run -p soundbench-audio --example bench -- detail <device>
//!   cargo run -p soundbench-audio --example bench -- latency <in> <out> [rate] [buffer]
//!   cargo run -p soundbench-audio --example bench -- sweep|tone|noise|transfer <in> <out> [rate] [buffer]
//!   cargo run -p soundbench-audio --example bench -- stability <in> <out> [rate] [sizes,comma,separated]
//!
//! Device numbers are PortAudio's global indices from `list`. Input channel
//! 0 and output channel 0 are used unless SB_IN / SB_OUT (0-based) are set.

use std::sync::atomic::AtomicBool;

use soundbench_audio::bench::{self, Progress, Target};
use soundbench_audio::Engine;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let engine = Engine::new().expect("PortAudio");
    println!("{}", Engine::version());
    let cmd = args.first().map(String::as_str).unwrap_or("list");
    match cmd {
        "list" => {
            for h in engine.hosts() {
                println!("host {:?} {:?} {} devices{}", h.index, h.kind, h.device_count, h.note.map(|n| format!(" — {n}")).unwrap_or_default());
            }
            for d in engine.devices() {
                println!(
                    "{:3}  {:<40} in {:3} out {:3}  {:>6.0} Hz  low {:.1}/{:.1} ms  [{}]{}{}",
                    d.index,
                    d.name,
                    d.max_input_channels,
                    d.max_output_channels,
                    d.default_sample_rate,
                    d.default_low_input_latency_ms,
                    d.default_low_output_latency_ms,
                    d.host_name,
                    if d.is_default_input { " *in" } else { "" },
                    if d.is_default_output { " *out" } else { "" }
                );
            }
        }
        "detail" => {
            let idx: i32 = args[1].parse().unwrap();
            let d = engine.detail(idx).expect("detail");
            println!("{}", serde_json::to_string_pretty(&d).unwrap());
        }
        test => {
            let input: i32 = args[1].parse().unwrap();
            let output: i32 = args[2].parse().unwrap();
            let rate: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(48000.0);
            let buffer: u32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(256);
            let in_ch: usize = std::env::var("SB_IN").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
            let out_ch: usize = std::env::var("SB_OUT").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
            let target = Target {
                input_device: input,
                output_device: output,
                input_channels: vec![in_ch],
                output_channels: vec![out_ch],
                sample_rate: rate,
                buffer_frames: buffer,
                wasapi_exclusive: true,
                mac_change_device: true,
            };
            let cancel = AtomicBool::new(false);
            let mut report = |e: bench::ProgressEvent| {
                if !e.message.is_empty() {
                    eprintln!("[{}] {:.0}% {}", e.phase, e.fraction * 100.0, e.message);
                }
            };
            let mut progress = Progress { cancel: &cancel, report: &mut report };
            let json = match test {
                "latency" => serde_json::to_string_pretty(&bench::latency_test(&engine, &target, &Default::default(), &mut progress).unwrap()),
                "sweep" => {
                    let r = bench::sweep_test(&engine, &target, &bench::SweepOptions { duration_s: 4.0, ..Default::default() }, &mut progress).unwrap();
                    let a = &r.channels[0].analysis;
                    eprintln!("found {} delay {:.1} gain {:.2} dB", a.found, a.delay_samples, a.peak_gain_db);
                    for (i, f) in a.freqs.iter().enumerate().step_by(24) {
                        eprintln!("{:8.1} Hz  {:+6.2} dB  {:+7.1}°  THD {:.4} %", f, a.magnitude_db[i], a.phase_deg[i], a.thd_percent[i]);
                    }
                    serde_json::to_string(&r.facts)
                }
                "tone" => serde_json::to_string_pretty(&bench::tone_test(&engine, &target, &Default::default(), &mut progress).unwrap().channels[0].analysis).map(|s| {
                    // Keep the spectrum out of the terminal.
                    s.lines().filter(|l| !l.trim_start().starts_with(char::is_numeric)).collect::<Vec<_>>().join("\n")
                }),
                "noise" => serde_json::to_string_pretty(&bench::noise_test(&engine, &target, &Default::default(), &mut progress).unwrap().channels[0].analysis)
                    .map(|s| s.lines().filter(|l| !l.trim_start().starts_with(char::is_numeric)).collect::<Vec<_>>().join("\n")),
                "transfer" => {
                    let r = bench::transfer_test(&engine, &target, &Default::default(), &mut progress).unwrap();
                    let a = &r.channels[0].analysis;
                    eprintln!("found {} delay {:.1} blocks {}", a.found, a.delay_samples, a.blocks);
                    for (i, f) in a.freqs.iter().enumerate().step_by(12) {
                        eprintln!("{:8.1} Hz  {:+6.2} dB  {:+7.1}°  coh {:.3}", f, a.magnitude_db[i], a.phase_deg[i], a.coherence[i]);
                    }
                    serde_json::to_string(&r.facts)
                }
                "stability" => {
                    let sizes: Vec<u32> = args
                        .get(4)
                        .map(|s| s.split(',').filter_map(|v| v.parse().ok()).collect())
                        .unwrap_or_else(|| vec![32, 64, 128, 256, 512, 1024]);
                    let r = bench::stability_test(
                        &engine,
                        &target,
                        &bench::StabilityOptions { buffer_sizes: sizes, duration_s: 3.0, frequency: 1000.0, level_dbfs: -12.0, max_delay_s: 1.0 },
                        &mut progress,
                    )
                    .unwrap();
                    for row in &r.rows {
                        let g = row.glitches.first().map(|g| format!("disc {} drop {} zero-runs {} resid {:.1} dB", g.analysis.discontinuities, g.analysis.dropouts, g.analysis.zero_runs, g.analysis.residual_median_db)).unwrap_or_default();
                        let cb = row.callbacks.as_ref().map(|c| format!("cb {} xruns {}/{}/{}/{} late {} max {:.2} ms min/max frames {}/{} cpu {:.1}%", c.callbacks, c.input_underflows, c.input_overflows, c.output_underflows, c.output_overflows, c.late_callbacks, c.max_interval_ms, c.min_frames, c.max_frames, c.cpu_load * 100.0)).unwrap_or_default();
                        let facts = row.facts.as_ref().map(|f| format!("rep in {:.2} out {:.2} host {:?}", f.input_latency_ms, f.output_latency_ms, f.host_buffer_frames)).unwrap_or_default();
                        eprintln!("{:5} {:?} lat {:?} ms | {} | {} | {} {}", row.requested_frames, row.verdict, row.latency_ms, facts, cb, g, row.error.clone().unwrap_or_default());
                    }
                    Ok(String::new())
                }
                other => panic!("unknown test {other}"),
            };
            println!("{}", json.unwrap());
        }
    }
}
