//! `soiree doctor`: what this computer lacks to make and certify a game, each
//! with its official link. It installs nothing. Salon compiles no C
//! (salon/README.md, « Construire »), but Rust links with the system's linker,
//! which rustup does not install.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::embedded;
use crate::report::{Level, Report};
use crate::util;

const RUSTUP: &str = "https://rustup.rs";
const WASM_BINDGEN: &str = "https://github.com/wasm-bindgen/wasm-bindgen";
const NODE: &str = "https://nodejs.org/en/download";
const CHROME: &str = "https://www.google.com/chrome/";
const TOXIPROXY: &str = "https://github.com/Shopify/toxiproxy#1-installing-toxiproxy";
const GIT: &str = "https://git-scm.com/downloads";
const GITHUB_CLI: &str = "https://cli.github.com";
const WINDOWS_LINKER: &str = "https://rust-lang.github.io/rustup/installation/windows-msvc.html";
/// The oldest Node the harness is meant to run on (Playwright).
const NODE_MAJOR: u32 = 20;
/// Installed Chromium builds, which the harness reaches only through `CHROME`.
const CHROMIUM: [&str; 3] = [
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
    "/snap/bin/chromium",
];

#[derive(Clone, Copy)]
enum System {
    Mac,
    Windows,
    Other,
}

#[derive(Clone, Copy)]
enum GithubCli {
    Absent,
    Disconnected,
    Connected,
}

const SYSTEM: System = if cfg!(target_os = "macos") {
    System::Mac
} else if cfg!(windows) {
    System::Windows
} else {
    System::Other
};

pub fn run(dir: &Path) -> ExitCode {
    let mut report = Report::new();
    let here = util::absolute(dir);
    report.section("cet ordinateur");
    tool(&mut report, &here, "git", &["--version"], GIT);
    github_cli(&mut report, &here);
    linker(&mut report, &here);
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
    if cfg!(windows) {
        report.warn(format!(
            "{} Node, Chrome, Toxiproxy et WebKit ne servent qu'à lui.",
            crate::certify::WINDOWS
        ));
    }
    report.finish()
}

fn say(report: &mut Report, (level, message): (Level, String)) {
    match level {
        Level::Ok => report.ok(message),
        Level::Error => report.error(message),
        Level::Warning => report.warn(message),
        Level::Info => report.info(message),
    }
}

/// On a Mac, `cc` is a stub that exists without Apple's tools, and running it
/// opens their installer: `xcode-select -p` is asked first.
fn linker(report: &mut Report, dir: &Path) {
    let developer = match SYSTEM {
        System::Mac => {
            util::stdout_of(dir, "xcode-select", &["-p"]).filter(|path| Path::new(path).is_dir())
        }
        System::Windows | System::Other => None,
    };
    let cc = match (SYSTEM, &developer) {
        (System::Mac, None) => None,
        _ => util::stdout_of(dir, "cc", &["--version"]),
    };
    say(
        report,
        linker_verdict(SYSTEM, developer.as_deref(), cc.as_deref()),
    );
}

fn linker_verdict(system: System, developer: Option<&str>, cc: Option<&str>) -> (Level, String) {
    match (system, developer, cc) {
        (System::Mac, None, _) => (
            Level::Error,
            "outils de développement d'Apple absents (Rust y prend son éditeur de liens) : xcode-select --install, puis valide la fenêtre qui s'ouvre".to_owned(),
        ),
        (_, _, Some(version)) => (
            Level::Ok,
            format!(
                "éditeur de liens : {}",
                version.lines().next().unwrap_or("cc")
            ),
        ),
        (System::Windows, _, None) => (
            Level::Info,
            format!("éditeur de liens : sous Windows, Rust prend celui de Visual Studio, que soiree doctor ne vérifie pas : {WINDOWS_LINKER}"),
        ),
        (System::Mac, Some(_), None) => (
            Level::Error,
            "cc ne répond pas (l'éditeur de liens de Rust) : réinstalle les outils d'Apple avec xcode-select --install".to_owned(),
        ),
        (System::Other, _, None) => (
            Level::Error,
            "éditeur de liens cc introuvable : installe build-essential, ou son équivalent sur ta distribution".to_owned(),
        ),
    }
}

/// Optional: the README also says how to create the repository without it.
fn github_cli(report: &mut Report, dir: &Path) {
    let state = if util::stdout_of(dir, "gh", &["--version"]).is_none() {
        GithubCli::Absent
    } else if util::run(dir, "gh", &["auth", "status"]).is_ok_and(|o| o.status.success()) {
        GithubCli::Connected
    } else {
        GithubCli::Disconnected
    };
    say(report, github_verdict(state));
}

fn github_verdict(state: GithubCli) -> (Level, String) {
    match state {
        GithubCli::Absent => (
            Level::Info,
            format!("gh absent (facultatif, il crée le dépôt du jeu) : {GITHUB_CLI}"),
        ),
        GithubCli::Connected => (Level::Ok, "gh connecté à GitHub".to_owned()),
        GithubCli::Disconnected => (
            Level::Info,
            "gh n'est pas connecté à GitHub : lance gh auth login toi-même, il ouvre le navigateur"
                .to_owned(),
        ),
    }
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
            "Node introuvable (soiree certify en a besoin ; soiree check seulement si cert.json déclare ui.checks[].success) : {NODE}"
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

/// The harness launches Playwright's `chrome` channel, the installed Google
/// Chrome, unless `CHROME` names a browser (tools/cert/browser.mjs): a Chromium
/// counts through that variable and no other way.
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
        let chromium = CHROMIUM
            .iter()
            .map(Path::new)
            .find(|path| cfg!(target_os = "linux") && path.exists());
        report.error(missing_chrome(chromium));
    }
}

fn missing_chrome(chromium: Option<&Path>) -> String {
    chromium.map_or_else(
        || format!("Google Chrome introuvable (le harnais le pilote) : {CHROME}"),
        |path| {
            format!(
                "Google Chrome introuvable, mais Chromium est là : le harnais ne le prend que par sa variable, export CHROME={}",
                path.display()
            )
        },
    )
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
    fn a_mac_without_apple_tools_is_told_before_cc_is_run() {
        let (level, message) = linker_verdict(System::Mac, None, Some("the stub answered"));
        assert_eq!(level, Level::Error);
        assert!(message.contains("xcode-select --install"), "{message}");
        let (level, message) = linker_verdict(
            System::Mac,
            Some("/Library/Developer"),
            Some("Apple clang\nx"),
        );
        assert_eq!(
            (level, message.as_str()),
            (Level::Ok, "éditeur de liens : Apple clang")
        );
        let (level, _) = linker_verdict(System::Mac, Some("/Library/Developer"), None);
        assert_eq!(level, Level::Error);
    }

    #[test]
    fn linux_needs_cc_and_windows_links_with_visual_studio() {
        assert_eq!(
            linker_verdict(System::Other, None, Some("cc 14")).0,
            Level::Ok
        );
        let (level, message) = linker_verdict(System::Other, None, None);
        assert_eq!(level, Level::Error);
        assert!(message.contains("build-essential"), "{message}");
        assert_eq!(linker_verdict(System::Windows, None, None).0, Level::Info);
    }

    #[test]
    fn github_cli_is_only_information_until_it_is_connected() {
        let (level, message) = github_verdict(GithubCli::Absent);
        assert_eq!(level, Level::Info);
        assert!(message.contains(GITHUB_CLI), "{message}");
        let (level, message) = github_verdict(GithubCli::Disconnected);
        assert_eq!(level, Level::Info);
        assert!(message.contains("gh auth login"), "{message}");
        assert_eq!(github_verdict(GithubCli::Connected).0, Level::Ok);
    }

    #[test]
    fn a_chromium_the_harness_would_not_find_gets_its_variable() {
        let message = missing_chrome(Some(Path::new("/usr/bin/chromium")));
        assert!(message.contains("CHROME=/usr/bin/chromium"), "{message}");
        assert!(missing_chrome(None).contains(CHROME));
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
