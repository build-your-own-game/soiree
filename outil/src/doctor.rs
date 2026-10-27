//! `soiree doctor`: what this computer lacks to make and certify a game, each
//! with its official link. It installs nothing. No C compiler: salon is Rust
//! from end to end (salon/README.md, « Construire »).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::embedded;
use crate::report::Report;
use crate::util;

const RUSTUP: &str = "https://rustup.rs";
const WASM_BINDGEN: &str = "https://github.com/wasm-bindgen/wasm-bindgen";
const NODE: &str = "https://nodejs.org/en/download";
const CHROME: &str = "https://www.google.com/chrome/";
const TOXIPROXY: &str = "https://github.com/Shopify/toxiproxy#1-installing-toxiproxy";
const GIT: &str = "https://git-scm.com/downloads";
/// The oldest Node the harness is meant to run on (Playwright).
const NODE_MAJOR: u32 = 20;

pub fn run(dir: &Path) -> ExitCode {
    let mut report = Report::new();
    let here = util::absolute(dir);
    report.section("cet ordinateur");
    tool(&mut report, &here, "git", &["--version"], GIT);
    rust(&mut report, &here);
    wasm_bindgen(&mut report, &here);
    node(&mut report, &here);
    chrome(&mut report);
    tool(
        &mut report,
        &here,
        "toxiproxy-server",
        &["--version"],
        TOXIPROXY,
    );
    tool(
        &mut report,
        &here,
        if cfg!(windows) { "npm.cmd" } else { "npm" },
        &["--version"],
        NODE,
    );
    harness_dependencies(&mut report);
    webkit(&mut report);
    report.finish()
}

/// A program that answers `--version`.
fn tool(
    report: &mut Report,
    dir: &Path,
    program: &str,
    args: &[&str],
    link: &str,
) -> Option<String> {
    if let Some(version) = util::stdout_of(dir, program, args) {
        report.ok(version.lines().next().unwrap_or(program).to_owned());
        Some(version)
    } else {
        report.error(format!("{program} introuvable : {link}"));
        None
    }
}

fn rust(report: &mut Report, dir: &Path) {
    if tool(report, dir, "rustup", &["--version"], RUSTUP).is_none() {
        return;
    }
    let Ok(channel) = embedded::channel() else {
        report.error("règles illisibles : regles/rust-toolchain.toml");
        return;
    };
    let toolchains = util::stdout_of(dir, "rustup", &["toolchain", "list"]).unwrap_or_default();
    let installed = toolchains.lines().any(|l| {
        l.starts_with(&format!("{channel}-"))
            || l.split_whitespace().next() == Some(channel.as_str())
    });
    let install = format!(
        "rustup toolchain install {channel} --component rustfmt,clippy --target wasm32-unknown-unknown"
    );
    if !installed {
        report.error(format!(
            "Rust {channel} absent (la version de la soirée) : {install}"
        ));
        return;
    }
    report.ok(format!("Rust {channel}"));
    let list = |what: &str| {
        util::stdout_of(
            dir,
            "rustup",
            &[what, "list", "--installed", "--toolchain", &channel],
        )
        .unwrap_or_default()
    };
    let components = list("component");
    for component in ["rustfmt", "clippy"] {
        if !components.lines().any(|l| l.starts_with(component)) {
            report.error(format!(
                "composant {component} absent : rustup component add {component} --toolchain {channel}"
            ));
        }
    }
    if list("target")
        .lines()
        .any(|l| l.trim() == "wasm32-unknown-unknown")
    {
        report.ok("cible wasm32-unknown-unknown");
    } else {
        report.error(format!(
            "cible WebAssembly absente : rustup target add wasm32-unknown-unknown --toolchain {channel}"
        ));
    }
}

/// The CLI must be exactly the `wasm-bindgen` version of the games' locks.
fn wasm_bindgen(report: &mut Report, dir: &Path) {
    let mut wanted = locked_versions(dir, "wasm-bindgen");
    let from_model = wanted.is_empty();
    if from_model {
        wanted.extend(model_wasm_bindgen());
    }
    let installed = util::stdout_of(dir, "wasm-bindgen", &["--version"])
        .and_then(|v| v.split_whitespace().nth(1).map(str::to_owned));
    let origin = if from_model {
        "le modèle"
    } else {
        "les Cargo.lock d'ici"
    };
    if wanted.len() > 1 {
        report.warn(format!(
            "{origin} demandent plusieurs versions de wasm-bindgen : {}",
            wanted.iter().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    let Some(version) = wanted.iter().next_back() else {
        return;
    };
    let install =
        format!("cargo install --locked wasm-bindgen-cli --version {version} ({WASM_BINDGEN})");
    match installed {
        Some(installed) if wanted.contains(&installed) => {
            report.ok(format!(
                "wasm-bindgen {installed}, celle que demandent {origin}"
            ));
        }
        Some(installed) => report.error(format!(
            "wasm-bindgen {installed}, mais {origin} demandent {version} : {install}"
        )),
        None => report.error(format!("wasm-bindgen introuvable : {install}")),
    }
}

/// The versions of `package` in the `Cargo.lock` files of `dir`, its
/// subfolders and those of `games/`.
pub fn locked_versions(dir: &Path, package: &str) -> BTreeSet<String> {
    let mut locks: Vec<PathBuf> = vec![dir.join("Cargo.lock")];
    for parent in [dir.to_path_buf(), dir.join("games")] {
        if let Ok(entries) = std::fs::read_dir(parent) {
            locks.extend(
                entries
                    .filter_map(Result::ok)
                    .map(|e| e.path().join("Cargo.lock")),
            );
        }
    }
    let mut versions = BTreeSet::new();
    for lock in locks.iter().filter(|l| l.is_file()) {
        let Ok(table) = util::read(lock).and_then(|t| util::parse_toml(&t, "Cargo.lock")) else {
            continue;
        };
        let entries = table.get("package").and_then(toml::Value::as_array);
        for entry in entries.into_iter().flatten() {
            if entry.get("name").and_then(toml::Value::as_str) == Some(package)
                && let Some(version) = entry.get("version").and_then(toml::Value::as_str)
            {
                versions.insert(version.to_owned());
            }
        }
    }
    versions
}

/// `wasm-bindgen = "=0.2.129"` in the model's `Cargo.toml`.
fn model_wasm_bindgen() -> Option<String> {
    let (_, text) = embedded::MODEL
        .iter()
        .find(|(path, _)| *path == "Cargo.toml")?;
    let table = util::parse_toml(text, "modele/Cargo.toml").ok()?;
    let version = table
        .get("workspace")?
        .get("dependencies")?
        .get("wasm-bindgen")?
        .as_str()?;
    Some(version.trim_start_matches('=').to_owned())
}

fn node(report: &mut Report, dir: &Path) {
    let Some(version) = util::stdout_of(dir, "node", &["--version"]) else {
        report.error(format!(
            "Node introuvable (le harnais de certification) : {NODE}"
        ));
        return;
    };
    let major: Option<u32> = version
        .trim_start_matches('v')
        .split('.')
        .next()
        .and_then(|m| m.parse().ok());
    if major.is_some_and(|m| m >= NODE_MAJOR) {
        report.ok(format!("Node {version}"));
    } else {
        report.error(format!(
            "Node {version} : il faut Node {NODE_MAJOR} ou plus récent, {NODE}"
        ));
    }
}

/// The harness drives the installed Google Chrome (`CHROME` overrides it).
fn chrome(report: &mut Report) {
    if let Some(path) = std::env::var_os("CHROME") {
        if Path::new(&path).exists() {
            report.ok(format!("Chrome (CHROME={})", Path::new(&path).display()));
        } else {
            report.error(format!(
                "CHROME={} n'existe pas",
                Path::new(&path).display()
            ));
        }
        return;
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let mut candidates = vec![
        PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
        home.join("Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
        PathBuf::from("/usr/bin/google-chrome"),
        PathBuf::from("/usr/bin/google-chrome-stable"),
        PathBuf::from("/opt/google/chrome/chrome"),
    ];
    for variable in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Some(root) = std::env::var_os(variable) {
            candidates.push(PathBuf::from(root).join("Google/Chrome/Application/chrome.exe"));
        }
    }
    if let Some(found) = candidates.iter().find(|p| p.exists()) {
        report.ok(format!("Chrome ({})", found.display()));
    } else {
        report.error(format!(
            "Google Chrome introuvable (le harnais le pilote) : {CHROME}"
        ));
    }
}

/// The harness of this version, extracted and installed by `soiree certify`.
fn harness_dependencies(report: &mut Report) {
    let Some(dir) = crate::harness::cache_dir() else {
        report.warn("pas de dossier de cache pour le harnais (ni HOME, ni LOCALAPPDATA)");
        return;
    };
    if crate::harness::installed(&dir) {
        report.ok(format!(
            "harnais {} installé ({})",
            embedded::VERSION,
            dir.display()
        ));
    } else {
        report.info(format!(
            "harnais {} pas encore installé : le premier soiree certify l'écrit dans {} et y lance npm ci",
            embedded::VERSION,
            dir.display()
        ));
    }
}

/// Playwright pins the `WebKit` binary as precisely as its npm dependency.
/// An older installed `WebKit` is not evidence that this harness can launch it.
fn webkit(report: &mut Report) {
    let Some(dir) = crate::harness::cache_dir() else {
        return;
    };
    if !crate::harness::installed(&dir) {
        report.info(
            "WebKit : vérification après le premier soiree certify (installation du harnais)",
        );
        return;
    }
    let script = "const {createRequire}=require('node:module'); const r=createRequire(process.argv[1]); process.stdout.write(r('playwright-core').webkit.executablePath());";
    let package = dir.join("package.json");
    let path = util::stdout_of(&dir, "node", &["-e", script, &package.to_string_lossy()]);
    match path {
        Some(path) if Path::new(path.trim()).is_file() => {
            report.ok(format!("WebKit de Playwright ({})", path.trim()));
        }
        Some(_) | None => report.error(format!(
            "WebKit de cette version absent : dans {}, npx playwright-core install webkit (https://playwright.dev/docs/browsers)",
            dir.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_pins_a_wasm_bindgen() {
        assert_eq!(model_wasm_bindgen().as_deref(), Some("0.2.129"));
    }

    #[test]
    fn locked_versions_are_read_from_every_lock() {
        let dir = crate::testdir::fresh("doctor");
        let lock = |v: &str| {
            format!("version = 4\n\n[[package]]\nname = \"wasm-bindgen\"\nversion = \"{v}\"\n")
        };
        util::write(&dir.join("games/a/Cargo.lock"), &lock("0.2.129")).unwrap();
        util::write(&dir.join("soiree/Cargo.lock"), &lock("0.2.130")).unwrap();
        let found: Vec<_> = locked_versions(&dir, "wasm-bindgen").into_iter().collect();
        assert_eq!(found, vec!["0.2.129", "0.2.130"]);
    }
}
