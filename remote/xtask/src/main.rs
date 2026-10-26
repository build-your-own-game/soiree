//! `cargo xtask ios [--target <triple>] [--release]`
//!
//! Builds `remote-ffi` as a static library for iOS and writes its Swift
//! bindings to `ios/Generated`. Xcode runs it before compiling, with the
//! target and profile of the build; by hand, it builds device and simulator.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use uniffi::{SwiftBindingsOptions, generate_swift_bindings};

const CRATE: &str = "remote-ffi";
const LIBRARY: &str = "libremote_ffi.a";
const IOS_TARGETS: [&str; 2] = ["aarch64-apple-ios", "aarch64-apple-ios-sim"];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("ios") => ios(&args[1..]),
        _ => Err("usage: cargo xtask ios [--target <triple>] [--release]".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}

fn ios(args: &[String]) -> Result<(), String> {
    let mut targets: Vec<String> = Vec::new();
    let mut release = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--target" => targets.push(args.next().ok_or("--target needs a triple")?.clone()),
            "--release" => release = true,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if targets.is_empty() {
        targets = IOS_TARGETS.map(String::from).to_vec();
    }

    let root = workspace_root();
    for target in &targets {
        let mut cargo = Command::new(env!("CARGO"));
        cargo
            .current_dir(&root)
            .args(["build", "--lib", "--package", CRATE, "--target", target]);
        if release {
            cargo.arg("--release");
        }
        run(&mut cargo)?;
    }

    // Bindings only depend on the Rust API, identical for every target.
    let profile = if release { "release" } else { "debug" };
    let library = root
        .join("target")
        .join(&targets[0])
        .join(profile)
        .join(LIBRARY);
    let out_dir = root.join("ios").join("Generated");
    generate_swift_bindings(SwiftBindingsOptions {
        generate_swift_sources: true,
        generate_headers: true,
        generate_modulemap: true,
        source: utf8(&library)?,
        out_dir: utf8(&out_dir)?,
        xcframework: false,
        module_name: Some("remote_ffiFFI".into()),
        modulemap_filename: Some("module.modulemap".into()),
        metadata_no_deps: false,
        link_frameworks: Vec::new(),
        config: None,
    })
    .map_err(|e| format!("generating Swift bindings: {e:#}"))?;
    println!(
        "xtask: {} for {} → {}",
        LIBRARY,
        targets.join(", "),
        out_dir.display()
    );
    Ok(())
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn utf8(path: &Path) -> Result<camino::Utf8PathBuf, String> {
    camino::Utf8PathBuf::from_path_buf(path.to_path_buf())
        .map_err(|p| format!("path is not UTF-8: {}", p.display()))
}

fn run(command: &mut Command) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|e| format!("starting {command:?}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{command:?} failed with {status}"))
    }
}
