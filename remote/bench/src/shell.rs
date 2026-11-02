//! The simulator and the app on it.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

const APP: &str = "fr.buildyourowngame.remote.bench";
/// The real binary: `xcrun simctl` hangs when `CoreSimulator` is older than Xcode.
const SIMCTL: &str =
    "/Library/Developer/PrivateFrameworks/CoreSimulator.framework/Versions/A/Resources/bin/simctl";
const SIM_TARGET: &str = "aarch64-apple-ios-sim";
const SWIFT_TARGET: &str = "arm64-apple-ios18.0-simulator";
/// What a single simctl call may take: it can hang, and a boot is slow on CI.
const SIMCTL_LIMIT: Duration = Duration::from_secs(180);

fn remote_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("bench/ is inside remote/")
        .to_path_buf()
}

fn bench_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(command: &mut Command) {
    let output = command
        .output()
        .unwrap_or_else(|e| panic!("starting {command:?}: {e}"));
    assert!(
        output.status.success(),
        "{command:?}: {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// `simctl`, bounded: it is the one tool here known to hang.
fn simctl(args: &[&str]) -> String {
    let scratch = bench_dir().join("target/simctl");
    fs::create_dir_all(&scratch).expect("scratch directory");
    let output = scratch.join(format!("{}.out", std::process::id()));
    let file = fs::File::create(&output).expect("simctl output");
    let mut child = Command::new(SIMCTL)
        .args(args)
        .stdin(Stdio::null())
        .stdout(file)
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("{SIMCTL}: {e}; the bench needs Xcode and a simulator"));
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("simctl status") {
            assert!(status.success(), "simctl {args:?}: {status}");
            return fs::read_to_string(&output).unwrap_or_default();
        }
        assert!(start.elapsed() < SIMCTL_LIMIT, "simctl {args:?} hangs");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Builds `Bench.app`: the shell's sources, as the app's target compiles them,
/// with the bench's `@main` in place of the app's.
fn build_app() -> PathBuf {
    let remote = remote_dir();
    run(Command::new(env!("CARGO"))
        .current_dir(&remote)
        .args(["xtask", "ios", "--target", SIM_TARGET]));
    let sdk = Command::new("xcrun")
        .args(["--sdk", "iphonesimulator", "--show-sdk-path"])
        .output()
        .expect("xcrun");
    let sdk = String::from_utf8_lossy(&sdk.stdout).trim().to_owned();
    let app = bench_dir().join("target/Bench.app");
    fs::create_dir_all(&app).expect("app directory");
    fs::copy(bench_dir().join("app/Info.plist"), app.join("Info.plist")).expect("Info.plist");
    let generated = remote.join("ios/Generated");
    let mut sources: Vec<PathBuf> = fs::read_dir(remote.join("ios/Remote"))
        .expect("the shell's sources")
        .map(|entry| entry.expect("a source").path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name != "RemoteApp.swift")
        })
        .collect();
    sources.sort();
    run(Command::new("swiftc")
        .env("SDKROOT", &sdk)
        .args(["-swift-version", "6", "-parse-as-library"])
        .args([
            "-module-name",
            "Bench",
            "-target",
            SWIFT_TARGET,
            "-sdk",
            &sdk,
        ])
        .arg("-I")
        .arg(&generated)
        .arg("-Xcc")
        .arg(format!(
            "-fmodule-map-file={}",
            generated.join("module.modulemap").display()
        ))
        .arg(generated.join("remote_ffi.swift"))
        .args(&sources)
        .arg(bench_dir().join("app/BenchApp.swift"))
        .arg("-L")
        .arg(remote.join("target").join(SIM_TARGET).join("debug"))
        .args(["-lremote_ffi", "-o"])
        .arg(app.join("Bench")));
    run(Command::new("codesign")
        .args(["-s", "-", "--force"])
        .arg(&app));
    app
}

/// A booted iPhone: the one already running if there is one. One this bench
/// booted is shut down when the test binary exits, by a small watcher, since
/// `cargo test` has no hook for it.
fn booted_device() -> String {
    let listing = simctl(&["list", "devices", "available", "-j"]);
    let listing: serde_json::Value = serde_json::from_str(&listing).expect("simctl's JSON");
    let iphones: Vec<(String, String)> = listing["devices"]
        .as_object()
        .expect("devices by runtime")
        .values()
        .flat_map(|devices| devices.as_array().cloned().unwrap_or_default())
        .filter(|device| {
            device["name"]
                .as_str()
                .is_some_and(|n| n.starts_with("iPhone"))
        })
        .map(|device| {
            (
                device["udid"].as_str().unwrap_or_default().to_owned(),
                device["state"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    if let Some((udid, _)) = iphones.iter().find(|(_, state)| state == "Booted") {
        return udid.clone();
    }
    let (udid, _) = iphones
        .first()
        .expect("no iPhone simulator: the bench needs one");
    simctl(&["bootstatus", udid, "-b"]);
    let watcher = format!(
        "while kill -0 {} 2>/dev/null; do sleep 1; done; exec {SIMCTL} shutdown {udid}",
        std::process::id()
    );
    // Backgrounded by the shell: it outlives this process on purpose.
    run(Command::new("sh")
        .arg("-c")
        .arg(format!("({watcher}) >/dev/null 2>&1 </dev/null &")));
    udid.clone()
}

fn device() -> &'static str {
    static DEVICE: OnceLock<String> = OnceLock::new();
    DEVICE.get_or_init(|| {
        let app = build_app();
        let udid = booted_device();
        simctl(&["install", &udid, &app.display().to_string()]);
        // The page's permission is the question under test, not iOS's own.
        simctl(&["privacy", &udid, "grant", "microphone", APP]);
        udid
    })
}

/// One app at a time: a simulator shows one, and a test owns it while it runs.
fn exclusive() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The app, launched afresh for one test.
pub struct Shell {
    reader: BufReader<TcpStream>,
    log: PathBuf,
    _turn: MutexGuard<'static, ()>,
}

impl Shell {
    pub fn launch() -> Shell {
        let turn = exclusive();
        let udid = device();
        let listener = TcpListener::bind("127.0.0.1:0").expect("control port");
        listener.set_nonblocking(true).expect("control port");
        let port = listener.local_addr().expect("control port").port();
        let log = bench_dir().join(format!("target/app-{}.log", std::process::id()));
        let status = Command::new(SIMCTL)
            .env("SIMCTL_CHILD_BENCH_CONTROL", port.to_string())
            .arg("launch")
            .arg("--terminate-running-process")
            .arg(format!("--stdout={}", log.display()))
            .arg(format!("--stderr={}", log.display()))
            .args([udid, APP])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .expect("launching the app");
        assert!(status.success(), "launch: {status}");
        let start = Instant::now();
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                // The first launch after a boot can take this long.
                Err(_) if start.elapsed() < Duration::from_secs(90) => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(error) => panic!("the app never called: {error}"),
            }
        };
        stream.set_nonblocking(false).expect("control socket");
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .expect("control socket");
        Shell {
            reader: BufReader::new(stream),
            log,
            _turn: turn,
        }
    }

    /// One command, one line back. `eval` scripts stay on one line.
    pub fn ask(&mut self, command: &str) -> String {
        let stream = self.reader.get_mut();
        stream
            .write_all(format!("{command}\n").as_bytes())
            .expect("the app is gone");
        let mut line = String::new();
        let read = self.reader.read_line(&mut line);
        if read.is_err() || line.is_empty() {
            let log = fs::read_to_string(&self.log).unwrap_or_default();
            panic!("no answer to {command:?}: {read:?}\n{log}");
        }
        line.trim_end().to_owned()
    }

    /// The first two numbers of an answer: a point.
    pub fn point(&mut self, command: &str) -> (f64, f64) {
        let answer = self.ask(command);
        let numbers: Vec<f64> = answer
            .split(' ')
            .filter_map(|word| word.parse().ok())
            .collect();
        assert!(numbers.len() >= 2, "{command}: {answer}");
        (numbers[0], numbers[1])
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        let _: std::io::Result<std::process::ExitStatus> = Command::new(SIMCTL)
            .args(["terminate", device(), APP])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}
