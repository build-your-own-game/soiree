//! Run the versioned observer and evaluate its scoped observations through the common policy.
//! Reports live in certification/verification.{json,md}; historical rapport files remain intact.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sha2::{Digest, Sha256};

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
    crate::verification::harness::run(dir, harness, extra)
}

/// Fingerprint the observer's executable bundle, not only its entry point.
#[cfg(test)]
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
#[cfg(test)]
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
