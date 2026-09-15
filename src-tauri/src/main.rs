// Suppress the console window on a Windows release build. Debug builds keep it
// so PortAudio's own diagnostics have somewhere to go.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    soundbench_lib::run()
}
