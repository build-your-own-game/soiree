//! `soiree check`: what a machine can verify of the manifesto, with the rules
//! embedded in this binary (`regles/`), where a shell script once read them
//! from `MANIFESTE.md`. On top of it: the version in `soiree.toml`, `.soiree/`,
//! where salon comes from, and the certification files.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::Value as Json;

use crate::dom;
use crate::embedded::{self, VERSION};
use crate::report::Report;
use crate::util::{self, Result};
use crate::{catalogue, gaps, items, lints};

#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    /// Skips fmt, both Clippy runs and the tests.
    pub fast: bool,
    /// Passes `--locked` to Cargo: never rewrites a `Cargo.lock`.
    pub locked: bool,
}

/// Crates that have nothing to do in a simulation that runs without a screen.
const SCREEN_CRATES: [&str; 7] = [
    "web-sys",
    "js-sys",
    "wasm-bindgen",
    "wasm-bindgen-futures",
    "wgpu",
    "winit",
    "web-time",
];

pub fn run(dirs: &[PathBuf], options: Options) -> ExitCode {
    let mut report = Report::new();
    for dir in dirs {
        check_project(&util::absolute(dir), options, &mut report);
    }
    report.finish()
}

/// What `soiree.toml` says about this folder.
/// What a checked folder says it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Folder {
    /// It has a `soiree.toml`: a game of the soirée, which owes its own files.
    Game,
    /// Anything else (salon, the remote): a file it lacks is no fault.
    Other,
}

struct Game {
    folder: Folder,
    version: Option<String>,
}

pub fn check_project(dir: &Path, options: Options, report: &mut Report) {
    report.section(&dir.display().to_string());
    let has_catalogue = dir.join("certifications").join("README.md").is_file();
    if !dir.join("Cargo.toml").is_file() {
        if has_catalogue {
            report.info("pas de Cargo.toml : seulement le catalogue des certifications");
            catalogue::check(dir, report);
        } else {
            report.error("pas de Cargo.toml");
        }
        return;
    }
    let Some(game) = soiree_toml(dir, report) else {
        return;
    };
    lints::check_toolchain(dir, report);
    let project =
        util::read(&dir.join("Cargo.toml")).and_then(|t| util::parse_toml(&t, "Cargo.toml"));
    let metadata = cargo_metadata(dir, options);
    match &project {
        Ok(project) => {
            lints::check_cargo(project, report);
            check_members(dir, project, metadata.as_ref().ok(), report);
        }
        Err(error) => report.error(error.clone()),
    }
    lints::check_clippy(dir, report);
    check_headless_core(dir, options, report);
    match &metadata {
        Ok(metadata) => check_sources(dir, metadata, game.version.as_deref(), report),
        Err(error) => report.warn(format!(
            "cargo metadata a échoué, provenance de salon non vérifiée : {error}"
        )),
    }
    check_javascript(dir, report);
    dom::check(dir, report);
    check_doc_paths(dir, report);
    gaps::check(dir, report);
    if has_catalogue {
        catalogue::check(dir, report);
    } else {
        items::check(dir, game.folder, report);
    }
    check_cert_json(dir, game.folder, report);
    if !options.fast {
        check_commands(dir, options, report);
    }
}

/// Reads `soiree.toml`, refuses another version, and writes `.soiree/`.
/// `None`: stop here.
fn soiree_toml(dir: &Path, report: &mut Report) -> Option<Game> {
    let path = dir.join("soiree.toml");
    if !path.is_file() {
        report.info("pas de soiree.toml : pas encore un jeu de soiree (version non vérifiée, .soiree/ non écrit)");
        return Some(Game {
            folder: Folder::Other,
            version: None,
        });
    }
    let table = match util::read(&path).and_then(|t| util::parse_toml(&t, "soiree.toml")) {
        Ok(table) => table,
        Err(error) => {
            report.error(error);
            return None;
        }
    };
    let Some(version) = table.get("version").and_then(toml::Value::as_str) else {
        report.error("soiree.toml : pas de version (attendu : version = \"0.1.0\")");
        return None;
    };
    if version != VERSION {
        report.error(format!(
            "soiree.toml demande la version {version} ; ce binaire est la {VERSION}. Installe la bonne : {}",
            embedded::install_command(version)
        ));
        return None;
    }
    match embedded::write_docs(dir) {
        Ok(()) => report.ok(format!(
            ".soiree/ écrit : les documents de la version {VERSION}"
        )),
        Err(error) => report.error(format!(".soiree/ : {error}")),
    }
    if let Ok(output) = util::run(dir, "git", &["check-ignore", "-q", ".soiree"])
        && output.status.code() == Some(1)
    {
        report.warn(".soiree/ n'est pas ignoré par git : ajoute /.soiree à .gitignore");
    }
    Some(Game {
        folder: Folder::Game,
        version: Some(version.to_owned()),
    })
}

fn cargo_metadata(dir: &Path, options: Options) -> Result<Json> {
    let mut args = vec!["metadata", "--format-version", "1"];
    if options.locked {
        args.push("--locked");
    }
    let output = util::run(dir, "cargo", &args)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr
            .lines()
            .find(|l| l.starts_with("error"))
            .unwrap_or("")
            .to_owned());
    }
    serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())
}

fn workspace_member_ids(metadata: &Json) -> BTreeSet<&str> {
    metadata["workspace_members"]
        .as_array()
        .map(|ids| ids.iter().filter_map(Json::as_str).collect())
        .unwrap_or_default()
}

fn packages(metadata: &Json) -> &[Json] {
    metadata["packages"].as_array().map_or(&[], Vec::as_slice)
}

/// Every member of a workspace inherits its lints.
fn check_members(dir: &Path, project: &toml::Table, metadata: Option<&Json>, report: &mut Report) {
    if !project.contains_key("workspace") {
        return;
    }
    let root = dir.join("Cargo.toml");
    let manifests: Vec<PathBuf> = if let Some(metadata) = metadata {
        let members = workspace_member_ids(metadata);
        packages(metadata)
            .iter()
            .filter(|p| p["id"].as_str().is_some_and(|id| members.contains(id)))
            .filter_map(|p| p["manifest_path"].as_str().map(PathBuf::from))
            .filter(|path| util::absolute(path) != util::absolute(&root))
            .collect()
    } else {
        let mut found: Vec<PathBuf> = std::fs::read_dir(dir.join("crates"))
            .map(|e| {
                e.filter_map(std::result::Result::ok)
                    .map(|e| e.path().join("Cargo.toml"))
                    .collect()
            })
            .unwrap_or_default();
        found.push(dir.join("xtask").join("Cargo.toml"));
        found.retain(|p| p.is_file());
        found
    };
    for manifest in manifests {
        let inherits = util::read(&manifest)
            .and_then(|t| util::parse_toml(&t, "Cargo.toml"))
            .is_ok_and(|member| lints::inherits_lints(&member));
        if !inherits {
            report.error(format!(
                "{} n'hérite pas des lints du workspace ([lints] workspace = true)",
                util::shown(&manifest, dir)
            ));
        }
    }
}

/// Principle 4: the simulation compiles without GPU or browser.
fn check_headless_core(dir: &Path, options: Options, report: &mut Report) {
    let manifest = dir.join("crates").join("core").join("Cargo.toml");
    let Some(name) = util::read(&manifest)
        .and_then(|t| util::parse_toml(&t, "crates/core/Cargo.toml"))
        .ok()
        .and_then(|t| t.get("package")?.get("name")?.as_str().map(str::to_owned))
    else {
        report.warn(
            "pas de crates/core : impossible de vérifier que la simulation tourne sans écran",
        );
        return;
    };
    let mut args = vec![
        "tree", "-q", "-p", &name, "-e", "normal", "--target", "all", "--prefix", "none",
    ];
    if options.locked {
        args.push("--locked");
    }
    let output = match util::run(dir, "cargo", &args) {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            report.warn(format!(
                "cargo tree a échoué, simulation sans écran non vérifiée : {}",
                stderr.lines().next().unwrap_or("")
            ));
            return;
        }
        Err(error) => {
            report.warn(error);
            return;
        }
    };
    let found: BTreeSet<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let crate_name = words.next()?;
            let screen = SCREEN_CRATES.contains(&crate_name)
                || crate_name
                    .strip_prefix("gloo")
                    .is_some_and(|rest| rest.chars().all(|c| c.is_ascii_lowercase() || c == '-'));
            (screen && words.next()?.starts_with('v')).then(|| line.to_owned())
        })
        .collect();
    if found.is_empty() {
        report.ok(format!("{name} ne dépend ni du navigateur ni du GPU"));
    } else {
        report.error(format!(
            "{name} dépend de {} : la simulation doit compiler sans navigateur ni GPU",
            found.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
}

/// Where salon and the remote come from. A local path outside this game's
/// repository is a salon nobody else has: warn before it gets pushed.
fn check_sources(dir: &Path, metadata: &Json, version: Option<&str>, report: &mut Report) {
    let members = workspace_member_ids(metadata);
    let repository =
        util::stdout_of(dir, "git", &["rev-parse", "--show-toplevel"]).map(PathBuf::from);
    let mut same_repo = Vec::new();
    let mut outside = Vec::new();
    let mut tagged = Vec::new();
    for package in packages(metadata) {
        let (Some(name), Some(id)) = (package["name"].as_str(), package["id"].as_str()) else {
            continue;
        };
        if !(name.starts_with("salon-") || name.starts_with("remote-")) || members.contains(id) {
            continue;
        }
        match package["source"].as_str() {
            None => {
                let path = package["manifest_path"]
                    .as_str()
                    .map(|p| util::absolute(Path::new(p)))
                    .and_then(|p| p.parent().map(Path::to_path_buf))
                    .unwrap_or_default();
                let inside = repository.as_ref().map_or_else(
                    || path.starts_with(dir),
                    |repo| path.starts_with(util::absolute(repo)),
                );
                if inside {
                    same_repo.push(name.to_owned());
                } else {
                    outside.push(format!("{name} ({})", path.display()));
                }
            }
            Some(source) if source.starts_with("git+") => {
                let tag = source
                    .split_once("tag=")
                    .map(|(_, rest)| rest.split(['#', '&']).next().unwrap_or(rest));
                match (tag, version) {
                    (None, _) => report.warn(format!("{name} vient de git sans étiquette ({source}) : attendu tag = \"v…\"")),
                    (Some(tag), Some(version)) if tag != format!("v{version}") => report.warn(format!(
                        "{name} est à l'étiquette {tag}, mais soiree.toml dit {version} : `soiree upgrade` les accorde"
                    )),
                    (Some(tag), _) => tagged.push(format!("{name} {tag}")),
                }
            }
            Some(source) => report.warn(format!(
                "{name} vient de {source} : attendu une étiquette git du dépôt commun"
            )),
        }
    }
    if !outside.is_empty() {
        report.warn(format!(
            "viennent d'un chemin local hors de ce dépôt, pas d'une étiquette git : {}. Ce jeu ne compile qu'ici ; ne le pousse pas tant que ce salon n'est pas étiqueté",
            outside.join(", ")
        ));
    }
    if !same_repo.is_empty() {
        report.info(format!(
            "{} : chemins locaux dans ce même dépôt",
            same_repo.join(", ")
        ));
    }
    if !tagged.is_empty() {
        report.ok(format!("depuis git : {}", tagged.join(", ")));
    }
}

/// The JavaScript is the generated glue and nothing else.
fn check_javascript(dir: &Path, report: &mut Report) {
    let web = dir.join("web");
    if !web.is_dir() {
        return;
    }
    for file in util::files_under(&web, &[".js", ".mjs", ".ts"], &["pkg"]) {
        report.error(format!(
            "JavaScript écrit à la main dans {} : passe par wasm-bindgen",
            util::shown(&file, dir)
        ));
    }
}

/// The paths quoted in `AGENTS.md` and `README.md` exist, in the folder or in
/// `.soiree/` (where the shared documents are).
fn check_doc_paths(dir: &Path, report: &mut Report) {
    for doc in ["AGENTS.md", "README.md"] {
        let Ok(text) = util::read(&dir.join(doc)) else {
            continue;
        };
        for path in cited_paths(&text) {
            if !dir.join(&path).exists() && !dir.join(".soiree").join(&path).exists() {
                report.error(format!("{doc} cite `{path}`, qui n'existe pas"));
            }
        }
    }
}

/// The code spans that look like a path with a `/`, like the `grep -o` of
/// the former shell check: `` `[A-Za-z0-9_.][A-Za-z0-9_./-]*/[A-Za-z0-9_./-]*` ``, with a
/// letter, sorted and without duplicates.
fn cited_paths(text: &str) -> BTreeSet<String> {
    let allowed = |c: char| c.is_ascii_alphanumeric() || "_./-".contains(c);
    let mut found = BTreeSet::new();
    for line in text.lines() {
        let mut rest = line;
        while let Some((_, after)) = rest.split_once('`') {
            let span: String = after.chars().take_while(|c| allowed(*c)).collect();
            let closed = after.get(span.len()..).and_then(|r| r.strip_prefix('`'));
            let first_ok = span
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
            if let Some(closed) = closed
                && first_ok
                && span.contains('/')
            {
                if span.chars().any(|c| c.is_ascii_alphabetic()) {
                    found.insert(span.clone());
                }
                rest = closed;
            } else {
                rest = after;
            }
        }
    }
    found
}

/// `cert.json`: how the harness launches and joins the game.
fn check_cert_json(dir: &Path, folder: Folder, report: &mut Report) {
    let path = dir.join("cert.json");
    let Ok(text) = util::read(&path) else {
        let under_games = dir
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|n| n == "games");
        if folder == Folder::Game || under_games {
            report.warn(
                "cert.json absent : le harnais de CERTIFICATION.md ne sait pas lancer ce jeu",
            );
        }
        return;
    };
    match serde_json::from_str::<Json>(&text) {
        Ok(config) => {
            let missing = cert_json_missing(&config);
            if missing.is_empty() {
                report.ok("cert.json lisible par le harnais (soiree certify)");
            } else {
                report.error(format!("cert.json : il manque {}", missing.join(", ")));
            }
        }
        Err(error) => report.error(format!("cert.json : {error}")),
    }
}

/// The keys the harness needs, with JavaScript's idea of an empty value.
fn cert_json_missing(config: &Json) -> Vec<&'static str> {
    let truthy = |key: &str| match config.get(key) {
        None | Some(Json::Null | Json::Bool(false)) => false,
        Some(Json::String(s)) => !s.is_empty(),
        Some(Json::Number(n)) => n.as_f64().is_some_and(|n| n != 0.0),
        Some(_) => true,
    };
    let mut missing: Vec<&'static str> = ["launch", "ready", "tv"]
        .into_iter()
        .filter(|key| !truthy(key))
        .collect();
    if config.get("phone").is_none() {
        missing.push("phone (null sans téléphones)");
    }
    if truthy("phone") {
        if !truthy("joined") {
            missing.push("joined");
        }
        if !config.get("join").is_some_and(Json::is_array) {
            missing.push("join");
        }
    }
    missing
}

/// The four commands the manifesto requires before a step is done.
fn check_commands(dir: &Path, options: Options, report: &mut Report) {
    let locked: &[&str] = if options.locked { &["--locked"] } else { &[] };
    let runs: [(&str, Vec<&str>); 4] = [
        ("cargo fmt --check", vec!["fmt", "--check"]),
        (
            "cargo clippy --all-targets",
            [
                &["clippy", "--all-targets", "--message-format=short"],
                locked,
                &["--", "-D", "warnings"],
            ]
            .concat(),
        ),
        (
            "cargo clippy --target wasm32-unknown-unknown",
            [
                &[
                    "clippy",
                    "--target",
                    "wasm32-unknown-unknown",
                    "--message-format=short",
                ],
                locked,
                &["--", "-D", "warnings"],
            ]
            .concat(),
        ),
        ("cargo test", [&["test"], locked].concat()),
    ];
    for (label, args) in runs {
        match util::run(dir, "cargo", &args) {
            Ok(output) if output.status.success() => report.ok(label),
            Ok(output) => {
                report.error(label);
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                for line in text.lines().filter(|l| notable(l)).take(8) {
                    report.detail(line);
                }
            }
            Err(error) => report.error(format!("{label} : {error}")),
        }
    }
}

/// The lines of a failed command worth showing (the filter of the former shell check).
fn notable(line: &str) -> bool {
    if line.contains("could not compile") {
        return false;
    }
    let diagnostic = ["error:", "error[", "warning:", "warning["]
        .iter()
        .any(|d| line.starts_with(d) || line.contains(&format!(": {d}")));
    diagnostic
        || line.starts_with("Diff in")
        || (line.starts_with("test ") && line.contains(" FAILED"))
        || (line.starts_with("thread ") && line.contains(" panicked"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cited_paths_are_code_spans_with_a_slash() {
        let text = "Voir `crates/core/src/lib.rs`, `../../salon/README.md` et `a b/c`.\n`cargo run -p x` `/play?seat=1` `1/2` `-x/y` `.soiree/`\n";
        let found: Vec<_> = cited_paths(text).into_iter().collect();
        assert_eq!(
            found,
            vec![
                "../../salon/README.md",
                ".soiree/",
                "crates/core/src/lib.rs"
            ]
        );
    }

    #[test]
    fn cert_json_needs_what_the_harness_reads() {
        let tier: Json = serde_json::from_str(
            r#"{"launch":["cargo"],"ready":"Tier list ready","tv":"/","phone":null}"#,
        )
        .unwrap();
        assert_eq!(cert_json_missing(&tier), Vec::<&str>::new());
        let phone: Json =
            serde_json::from_str(r#"{"launch":[],"ready":"","tv":"/","phone":"/play"}"#).unwrap();
        assert_eq!(cert_json_missing(&phone), vec!["ready", "joined", "join"]);
        let none: Json = serde_json::from_str("{}").unwrap();
        assert_eq!(
            cert_json_missing(&none),
            vec!["launch", "ready", "tv", "phone (null sans téléphones)"]
        );
    }

    #[test]
    fn notable_lines_are_diagnostics_and_failures() {
        assert!(notable("src/lib.rs:3:5: error[E0425]: cannot find value"));
        assert!(notable("warning: unused import"));
        assert!(notable("Diff in /x/src/lib.rs:1:"));
        assert!(notable("test tests::a ... FAILED"));
        assert!(notable("thread 'main' panicked at src/x.rs"));
        assert!(!notable("error: could not compile `x`"));
        assert!(!notable("   Compiling x v0.1.0"));
    }
}
