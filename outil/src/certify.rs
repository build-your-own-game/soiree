//! `soiree certify`: runs the harness embedded in this version
//! (`node <cache>/cert.mjs <game>`, see `harness.rs`), then writes `certification/`: the
//! harness's `rapport.md`, and `rapport.json` with the versions, the seed, the
//! date, the machine and the fingerprint of the code.
//!
//! Nothing proves the harness really ran: the report is trusted. What anyone
//! can check is that it matches the code, through the fingerprint.

use std::fmt::Write as _;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use serde_json::json;
use sha2::{Digest, Sha256};

use crate::embedded::VERSION;
use crate::util::{self, Result};

pub fn run(dir: &Path, harness: Option<&Path>, extra: &[String]) -> ExitCode {
    let harness = match crate::harness::locate(harness) {
        Ok(harness) => harness,
        Err(error) => {
            eprintln!("soiree certify : {error}");
            return ExitCode::FAILURE;
        }
    };
    match certify(&util::absolute(dir), &harness, extra) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("soiree certify : {error}");
            ExitCode::FAILURE
        }
    }
}

/// `Ok(passed)`.
fn certify(dir: &Path, harness: &Path, extra: &[String]) -> Result<bool> {
    let script = harness.join("cert.mjs");
    if !script.is_file() {
        return Err(format!(
            "harnais introuvable : {} (--harness <dossier> ou SOIREE_HARNESS)",
            script.display()
        ));
    }
    let started = util::now_utc();
    let harness_before = harness_fingerprint(harness)?;
    let source_before = fingerprint(dir).ok();
    let mut child = Command::new("node")
        .arg(&script)
        .arg(dir)
        .args(extra)
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("node : {error}"))?;
    // The harness talks for minutes: echo it as it comes, and keep it.
    let mut output = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout)
            .lines()
            .map_while(std::result::Result::ok)
        {
            println!("{line}");
            output.push(line);
        }
    }
    let status = child.wait().map_err(|error| format!("node : {error}"))?;

    let harness_unchanged = harness_before == harness_fingerprint(harness)?;
    let source_after = fingerprint(dir);
    let source_unchanged = source_before
        .as_ref()
        .zip(source_after.as_ref().ok())
        .map(|(before, after)| before.value == after.value);
    let passed = status.success() && harness_unchanged && source_unchanged.unwrap_or(true);

    let out = dir.join("certification");
    let captures = captures(&output);
    if let Some(report) = captures.as_ref().map(|c| c.join("rapport.md"))
        && report.is_file()
    {
        let text = util::read(&report)?;
        util::write(&out.join("rapport.md"), &text)?;
    }
    let performance = captures
        .as_ref()
        .map(|c| c.join("performance.json"))
        .filter(|path| path.is_file())
        .map(|path| {
            util::read(&path).and_then(|text| {
                let value: serde_json::Value =
                    serde_json::from_str(&text).map_err(|error| error.to_string())?;
                util::write(&out.join("performance.json"), &text)?;
                Ok(value)
            })
        })
        .transpose()?;
    let report = json!({
        "performance": performance,
        "soiree": VERSION,
        "salon": salon_version(dir),
        // The observer includes imported modules, configuration and resources.
        "harness_sha256": harness_before,
        "harness_unchanged": harness_unchanged,
        "source_unchanged": source_unchanged,
        "arguments": extra,
        "seed": seed(&output),
        "started": started,
        "finished": util::now_utc(),
        "passed": passed,
        "exit_code": status.code(),
        "machine": machine(dir),
        "fingerprint": source_after.map_or_else(
            |error| json!({ "error": error }),
            |f| json!({
                "algorithm": "sha256 de « chemin\\0sha256(contenu)\\n » des fichiers suivis ou non ignorés par git, triés, hors certification/",
                "value": f.value,
                "files": f.files,
                "uncommitted": f.uncommitted,
            }),
        ),
    });
    let text = serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?;
    util::write(&out.join("rapport.json"), &format!("{text}\n"))?;
    println!("\n{} écrit.", util::shown(&out, dir));
    if !harness_unchanged {
        eprintln!("le harnais a changé pendant la certification : passage à relancer");
    }
    if source_unchanged == Some(false) {
        eprintln!("les sources ont changé pendant la certification : passage à relancer");
    }
    Ok(passed)
}

/// Fingerprint the observer's executable bundle, not only its entry point.
fn harness_fingerprint(dir: &Path) -> Result<String> {
    let mut hash = Sha256::new();
    for file in util::files_under(
        dir,
        &[".mjs", ".js", ".json", ".css", ".html", ".wasm"],
        &["node_modules", ".git"],
    ) {
        let relative = file.strip_prefix(dir).map_err(|error| error.to_string())?;
        let bytes = std::fs::read(&file).map_err(|error| error.to_string())?;
        hash.update(relative.to_string_lossy().as_bytes());
        hash.update(b"\0");
        hash.update(Sha256::digest(&bytes));
        hash.update(b"\n");
    }
    Ok(hex(&hash.finalize()))
}

/// The harness prints `graine 12` when it starts and `Graine 12` in its report.
pub fn seed(output: &[String]) -> Option<u64> {
    output.iter().rev().find_map(|line| {
        let lower = line.to_lowercase();
        let rest = lower.split("graine ").nth(1)?;
        rest.chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .ok()
    })
}

/// `Captures : /tmp/cert-x-…` in the harness's report.
pub fn captures(output: &[String]) -> Option<PathBuf> {
    output.iter().rev().find_map(|line| {
        let rest = line.split("Captures : ").nth(1)?;
        Some(PathBuf::from(rest.trim()))
    })
}

/// Salon as the game's lock pins it.
fn salon_version(dir: &Path) -> serde_json::Value {
    let Ok(lock) =
        util::read(&dir.join("Cargo.lock")).and_then(|t| util::parse_toml(&t, "Cargo.lock"))
    else {
        return serde_json::Value::Null;
    };
    let entries = lock.get("package").and_then(toml::Value::as_array);
    entries
        .into_iter()
        .flatten()
        .find(|p| p.get("name").and_then(toml::Value::as_str) == Some("salon-core"))
        .map_or(serde_json::Value::Null, |p| {
            json!({
                "version": p.get("version").and_then(toml::Value::as_str),
                "source": p.get("source").and_then(toml::Value::as_str).unwrap_or("chemin local"),
            })
        })
}

/// What ran the harness. No host name: the report is committed, and a host
/// name often carries its owner's name.
fn machine(dir: &Path) -> serde_json::Value {
    json!({
        "os": std::env::consts::OS,
        "os_version": util::stdout_of(dir, "uname", &["-r"]),
        "arch": std::env::consts::ARCH,
        "cpus": std::thread::available_parallelism().map(std::num::NonZero::get).ok(),
        "rustc": util::stdout_of(dir, "rustc", &["--version"]),
        "node": util::stdout_of(dir, "node", &["--version"]),
    })
}

pub struct Fingerprint {
    pub value: String,
    pub files: usize,
    /// Something outside `certification/` differs from the last commit.
    pub uncommitted: bool,
}

/// The tracked and untracked, nonignored source files in `dir`, outside `certification/`, as they are on
/// disk now. A tracked file deleted from the disk counts as « deleted ».
pub fn fingerprint(dir: &Path) -> Result<Fingerprint> {
    let listed = util::run(
        dir,
        "git",
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            ".",
            ":!certification",
        ],
    )?;
    if !listed.status.success() {
        return Err(format!(
            "pas un dépôt git : {}",
            String::from_utf8_lossy(&listed.stderr).trim()
        ));
    }
    let mut files: Vec<String> = String::from_utf8_lossy(&listed.stdout)
        .split('\0')
        .filter(|f| !f.is_empty())
        .map(str::to_owned)
        .collect();
    files.sort();
    let mut all = Sha256::new();
    for file in &files {
        let content = std::fs::read(dir.join(file)).map_or_else(
            |_| "deleted".to_owned(),
            |bytes| hex(&Sha256::digest(&bytes)),
        );
        all.update(file.as_bytes());
        all.update(b"\0");
        all.update(content.as_bytes());
        all.update(b"\n");
    }
    let uncommitted = util::stdout_of(
        dir,
        "git",
        &["status", "--porcelain", "--", ".", ":!certification"],
    )
    .is_some_and(|s| !s.is_empty());
    Ok(Fingerprint {
        value: hex(&all.finalize()),
        files: files.len(),
        uncommitted,
    })
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_and_captures_are_read_from_the_harness() {
        let output = [
            "[0.0s] /x/games/a : lancement (cargo run) sur le port 4000, graine 4242".to_owned(),
            "# Certification de games/a".to_owned(),
            String::new(),
            "Graine 4242, 5.1 min. Captures : /tmp/cert-a-2026".to_owned(),
        ];
        assert_eq!(seed(&output), Some(4242));
        assert_eq!(captures(&output), Some(PathBuf::from("/tmp/cert-a-2026")));
        assert_eq!(seed(&[]), None);
    }

    #[test]
    fn imported_observer_modules_change_the_harness_fingerprint() {
        let dir = crate::testdir::fresh("harness-fingerprint");
        util::write(&dir.join("cert.mjs"), "import './browser.mjs';").unwrap();
        util::write(&dir.join("browser.mjs"), "export const version = 1;").unwrap();
        let before = harness_fingerprint(&dir).unwrap();
        util::write(&dir.join("browser.mjs"), "export const version = 2;").unwrap();
        assert_ne!(harness_fingerprint(&dir).unwrap(), before);
        let before = harness_fingerprint(&dir).unwrap();
        util::write(
            &dir.join("node_modules/ignored.js"),
            "external pinned dependency",
        )
        .unwrap();
        assert_eq!(harness_fingerprint(&dir).unwrap(), before);
    }

    #[test]
    fn the_fingerprint_follows_the_code_and_ignores_the_report() {
        let dir = crate::testdir::fresh("certify");
        let git = |args: &[&str]| {
            assert!(
                util::run(&dir, "git", args).unwrap().status.success(),
                "{args:?}"
            );
        };
        git(&["init", "-q"]);
        util::write(&dir.join("src/lib.rs"), "pub fn a() {}\n").unwrap();
        util::write(&dir.join("certification/rapport.json"), "{}\n").unwrap();
        git(&["add", "-A"]);
        git(&[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "x",
        ]);

        let first = fingerprint(&dir).unwrap();
        assert_eq!(first.files, 1);
        assert!(!first.uncommitted);

        util::write(&dir.join("certification/rapport.json"), "{\"seed\": 1}\n").unwrap();
        let after_report = fingerprint(&dir).unwrap();
        assert_eq!(after_report.value, first.value);
        assert!(!after_report.uncommitted);

        util::write(&dir.join("src/lib.rs"), "pub fn b() {}\n").unwrap();
        let after_code = fingerprint(&dir).unwrap();
        assert_ne!(after_code.value, first.value);
        assert!(after_code.uncommitted);
        util::write(&dir.join("src/new.rs"), "pub fn untracked() {}\n").unwrap();
        let untracked = fingerprint(&dir).unwrap();
        assert_ne!(untracked.value, after_code.value);
        assert_eq!(untracked.files, 2);
    }
}
