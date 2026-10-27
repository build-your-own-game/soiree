//! A tiny game on salon, to try it end to end: every phone taps, the TV
//! counts, and everybody sees who is here. `cargo run -p salon-demo`.

#[cfg(not(target_arch = "wasm32"))]
mod protocol;

// The host is a native program; on wasm32 there is nothing to run, so a
// workspace-wide wasm check passes (tools/check.sh).
#[cfg(not(target_arch = "wasm32"))]
mod native;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    native::main()
}

#[cfg(target_arch = "wasm32")]
fn main() {}
