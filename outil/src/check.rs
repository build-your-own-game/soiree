//! `soiree check` : un jeu suit-il les règles de la console, et se
//! branche-t-il ? Chaque refus dit quoi corriger.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::io::{BufRead as _, BufReader, IsTerminal as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use serde::Deserialize;

use crate::rules::{self, Metadata};
use crate::{machine, plug};

/// Les dossiers qu'on ne fouille pas : ce qui se fabrique, et l'historique.
const SKIPPED: [&str; 3] = ["target", ".git", "node_modules"];
/// Une exception s'écrit sur quelques lignes : on ne lit pas plus loin.
const EXCEPTION_LINES: usize = 12;

struct Report {
    /// Les étapes refusées, pour les redire à la fin.
    failures: Vec<String>,
    /// La fin de l'étape précédente : chaque étape dit le temps qu'elle a pris.
    since: Instant,
}

impl Report {
    fn step(&mut self, name: &str, outcome: Result<(), String>) -> bool {
        let took = format!("{:.1} s", self.since.elapsed().as_secs_f32());
        self.since = Instant::now();
        match outcome {
            Ok(()) => {
                println!("✔ {name}  ({took})");
                true
            }
            Err(fix) => {
                println!("✘ {name}  ({took})\n{fix}\n");
                self.failures.push(name.to_owned());
                false
            }
        }
    }
}

pub fn run(dir: &Path) -> Result<(), String> {
    let metadata = rules::metadata(dir)?;
    let root = metadata.workspace_root.clone();
    println!("soiree {} vérifie {}\n", crate::VERSION, root.display());
    let mut report = Report {
        failures: Vec::new(),
        since: Instant::now(),
    };

    if !report.step("les règles sont en place", rule_files(&metadata)) {
        // Avec d'autres règles, les linteurs refuseraient ou laisseraient
        // passer à tort : on s'arrête là.
        return Err(format!(
            "\nLe reste n'est pas essayé : ce jeu n'a pas les règles de soiree {}. Lance \
             `soiree rules`, puis `soiree check`.",
            crate::VERSION
        ));
    }
    let has_machine = report.step("le jeu a une machine", machine::declared(&metadata));
    let _: bool = report.step("tout est en Rust", only_rust(&root));
    let _: bool = report.step(
        "le formatage",
        cargo(
            &root,
            &["fmt", "--all", "--check"],
            "Lance `cargo fmt --all`.",
        ),
    );
    let mut compiles = report.step(
        "le linteur, pour l'ordinateur",
        clippy(&root, &["--workspace", "--all-targets"]),
    );
    for page in metadata.packages.iter().filter(|package| package.is_page()) {
        compiles &= report.step(
            &format!("le linteur, pour le navigateur ({})", page.name),
            clippy(
                &root,
                &[
                    "--package",
                    &page.name,
                    "--target",
                    "wasm32-unknown-unknown",
                ],
            ),
        );
    }
    let pure = "la machine se passe du système";
    if has_machine {
        compiles &= report.step(pure, machine::no_system(&metadata));
    } else {
        println!("– {pure} : pas essayé tant que le jeu n'a pas de machine");
    }
    compiles &= report.step(
        "les tests du jeu",
        cargo(
            &root,
            &["test", "--workspace"],
            "Un test échoue : corrige le jeu, ou le test s'il se trompe.",
        ),
    );
    let replays = "la machine rejoue la même partie, en natif et en WebAssembly";
    if compiles && has_machine {
        let _: bool = report.step(replays, machine::replays(&metadata));
    } else {
        println!("– {replays} : pas essayé tant que le reste ne passe pas");
    }
    if compiles {
        let _: bool = report.step("le jeu se branche", plug::run(&root));
    } else {
        println!("– le jeu se branche : pas essayé tant que le reste ne passe pas");
    }
    print_exceptions(&metadata);

    if report.failures.is_empty() {
        println!(
            "\nCe jeu passe la vérification de soiree {}.",
            crate::VERSION
        );
        Ok(())
    } else {
        Err(format!(
            "\n{} point(s) à corriger : ce jeu ne passe pas la vérification de soiree {}.\n  ✘ {}\n\
             Chaque refus, plus haut, dit quoi corriger.",
            report.failures.len(),
            crate::VERSION,
            report.failures.join("\n  ✘ ")
        ))
    }
}

/// Lance cargo en laissant voir sa sortie : c'est elle qui dit quoi corriger.
fn cargo(root: &Path, args: &[&str], fix: &str) -> Result<(), String> {
    println!("\n── cargo {}", args.join(" "));
    let status = Command::new("cargo")
        .current_dir(root)
        .args(args)
        .status()
        .map_err(|error| format!("impossible de lancer cargo : {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("  {fix}"))
    }
}

/// Une ligne de `cargo --message-format=json`.
#[derive(Deserialize)]
struct Line {
    reason: String,
    message: Option<Diagnostic>,
}

#[derive(Deserialize)]
struct Diagnostic {
    rendered: Option<String>,
    code: Option<Code>,
}

#[derive(Deserialize)]
struct Code {
    code: String,
}

/// Lance Clippy, montre ses refus, puis dit pour chaque règle touchée
/// pourquoi la console la pose et par où sortir.
fn clippy(root: &Path, scope: &[&str]) -> Result<(), String> {
    println!("\n── cargo clippy {} -- -D warnings", scope.join(" "));
    let format = if std::io::stdout().is_terminal() {
        "--message-format=json-diagnostic-rendered-ansi"
    } else {
        "--message-format=json"
    };
    let mut child = Command::new("cargo")
        .current_dir(root)
        .arg("clippy")
        .args(scope)
        .args([format, "--", "-D", "warnings"])
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("impossible de lancer cargo : {error}"))?;
    let mut lints: BTreeSet<String> = BTreeSet::new();
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Ok(line) = serde_json::from_str::<Line>(&line) else {
                continue;
            };
            let Some(message) = line.message.filter(|_| line.reason == "compiler-message") else {
                continue;
            };
            if let Some(rendered) = message.rendered {
                // Clippy propose `#[allow]` : la console ne l'accepte pas.
                for row in rendered
                    .lines()
                    .filter(|row| !row.contains("= help: to override"))
                {
                    println!("{row}");
                }
            }
            // Un code `E0433` est une erreur de compilation, pas une règle.
            if let Some(code) = message.code.filter(|code| !is_compile_error(&code.code)) {
                let _: bool = lints.insert(code.code);
            }
        }
    }
    let status = child
        .wait()
        .map_err(|error| format!("cargo ne répond plus : {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(advice(&lints))
    }
}

/// Un code d'erreur du compilateur (`E0433`), par opposition à un lint.
fn is_compile_error(code: &str) -> bool {
    code.strip_prefix('E')
        .is_some_and(|digits| digits.chars().all(|c| c.is_ascii_digit()))
}

/// Ce que la console dit des règles qui viennent de refuser.
fn advice(lints: &BTreeSet<String>) -> String {
    let known = rules::advice();
    let mut text = String::new();
    let mut others: Vec<&str> = Vec::new();
    for lint in lints {
        let name = lint.strip_prefix("clippy::").unwrap_or(lint);
        match known.get(name) {
            Some(advice) => {
                let _: std::fmt::Result = writeln!(
                    text,
                    "  · {name} : {}\n    Par où sortir : {}",
                    advice.pourquoi, advice.remede
                );
            }
            None => others.push(name),
        }
    }
    if !others.is_empty() {
        let _: std::fmt::Result = writeln!(
            text,
            "  · {} : le message de chaque refus dit quoi changer.",
            others.join(", ")
        );
    }
    if text.is_empty() {
        text.push_str("  Corrige chaque erreur ci-dessus.\n");
    } else {
        text.insert_str(0, "  Ce que la console en dit :\n");
    }
    text.push_str(
        "  Une exception, `#[expect(lint, reason = \"…\")]`, est le dernier recours : au plus près \
         de la ligne, jamais sur un module entier, et sa raison dit pourquoi ce que la règle \
         protège n'est pas en jeu ici.",
    );
    text
}

/// La version de Rust, les réglages de Clippy et la table des lints sont ceux
/// de la console, et chaque crate du jeu s'y soumet.
fn rule_files(metadata: &Metadata) -> Result<(), String> {
    let root = &metadata.workspace_root;
    let shown = |path: &Path| {
        path.strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string()
    };
    let mut wrong: Vec<String> = Vec::new();
    for (file, expected) in [
        ("rust-toolchain.toml", rules::TOOLCHAIN),
        ("clippy.toml", rules::CLIPPY),
    ] {
        let expected: toml::Table = expected
            .parse()
            .map_err(|error| format!("regles/{file} : {error}"))?;
        if rules::read_toml(&root.join(file)).ok() != Some(expected) {
            wrong.push(format!("{file} n'est pas celui de la console"));
        }
    }
    let extra: Vec<String> = metadata
        .stray_clippy_files()
        .iter()
        .map(|path| shown(path))
        .collect();
    let manifest = rules::read_toml(&root.join("Cargo.toml"))?;
    let prefix = rules::lints_prefix(&manifest);
    let lints = prefix
        .split('.')
        .try_fold(&manifest, |table, key| table.get(key)?.as_table());
    if lints != Some(&rules::lints_table()?) {
        wrong.push(format!(
            "[{prefix}] de Cargo.toml n'est pas la table de la console"
        ));
    }
    if prefix == "workspace.lints" {
        for package in &metadata.packages {
            let follows = rules::read_toml(&package.manifest_path)?
                .get("lints")
                .and_then(|lints| lints.get("workspace"))
                .and_then(toml::Value::as_bool);
            if follows != Some(true) {
                wrong.push(format!(
                    "le crate {} n'a pas `[lints] workspace = true`",
                    package.name
                ));
            }
        }
    }
    let mut fixes: Vec<String> = Vec::new();
    if !wrong.is_empty() {
        fixes.push(format!(
            "  {}\n  Lance `soiree rules` : il écrit les règles de cette version dans le jeu.",
            wrong.join("\n  ")
        ));
    }
    if !extra.is_empty() {
        fixes.push(format!(
            "  {}\n  Supprime ces fichiers (`soiree rules` le fait) : les réglages de Clippy sont ceux \
             de la console, à la racine du jeu.",
            extra.join("\n  ")
        ));
    }
    if fixes.is_empty() {
        Ok(())
    } else {
        Err(fixes.join("\n"))
    }
}

/// Aucun fichier d'un autre langage : les pages aussi sont en Rust.
fn only_rust(root: &Path) -> Result<(), String> {
    let mut foreign = Vec::new();
    walk(root, &mut |path| {
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| rules::FOREIGN.contains(&extension))
        {
            foreign.push(
                path.strip_prefix(root)
                    .unwrap_or(path)
                    .display()
                    .to_string(),
            );
        }
    })
    .map_err(|error| format!("  {error}"))?;
    if foreign.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "  {}\n  Réécris ces fichiers en Rust. Dans une page, seul reste le chargeur de `index.html`.",
            foreign.join("\n  ")
        ))
    }
}

/// Visite chaque fichier du jeu, hors de ce qui se fabrique.
fn walk(dir: &Path, visit: &mut impl FnMut(&Path)) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path: PathBuf = entry.path();
        if entry.file_type()?.is_dir() {
            if !SKIPPED.iter().any(|skipped| entry.file_name() == *skipped) {
                walk(&path, visit)?;
            }
        } else {
            visit(&path);
        }
    }
    Ok(())
}

/// Une règle éteinte à un endroit du jeu.
struct Exception {
    place: String,
    lints: String,
    reason: String,
    whole_file: bool,
    in_machine: bool,
    /// Dans un fichier de tests, ou posée sur un module de tests.
    in_tests: bool,
}

/// Les `#[expect(…)]` d'un fichier.
fn exceptions_in(text: &str, place: &str, found: &mut Vec<Exception>) {
    let rows: Vec<&str> = text.lines().collect();
    for (index, line) in rows.iter().enumerate() {
        let start = line.trim_start();
        let whole_file = start.starts_with("#![expect(");
        if !whole_file && !start.starts_with("#[expect(") {
            continue;
        }
        let mut attribute = String::new();
        for part in rows.iter().skip(index).take(EXCEPTION_LINES) {
            attribute.push_str(part.trim());
            attribute.push(' ');
            if part.contains(")]") {
                break;
            }
        }
        let inside = attribute
            .split_once("expect(")
            .map_or("", |(_, inside)| inside);
        let (lints, reason) = inside.split_once("reason").unwrap_or((inside, ""));
        let reason = reason
            .split_once('"')
            .and_then(|(_, rest)| rest.rsplit_once('"'))
            .map_or("sans raison", |(reason, _)| reason);
        found.push(Exception {
            place: format!("{place}:{}", index.saturating_add(1)),
            lints: lints
                .trim_matches(|c: char| c.is_whitespace() || c == ',' || c == ')' || c == ']')
                .to_owned(),
            reason: reason.to_owned(),
            whole_file,
            in_machine: false,
            in_tests: place.contains("tests")
                || rows
                    .iter()
                    .skip(index.saturating_sub(1))
                    .take(EXCEPTION_LINES)
                    .take_while(|row| !row.trim_start().starts_with("fn "))
                    .any(|row| row.contains("cfg(test)") || row.contains("mod tests")),
        });
    }
}

/// Montre à l'hôte et à l'auteur les règles que le jeu a éteintes : ce n'est
/// pas un refus, mais chaque exception doit pouvoir se défendre.
fn print_exceptions(metadata: &Metadata) {
    let root = &metadata.workspace_root;
    let machines: Vec<&Path> = metadata.machines().map(rules::Package::dir).collect();
    let mut found: Vec<Exception> = Vec::new();
    let walked = walk(root, &mut |path| {
        if path.extension().is_some_and(|extension| extension == "rs")
            && let Ok(text) = fs::read_to_string(path)
        {
            let place = path
                .strip_prefix(root)
                .unwrap_or(path)
                .display()
                .to_string();
            let in_machine = machines.iter().any(|machine| path.starts_with(machine));
            let before = found.len();
            exceptions_in(&text, &place, &mut found);
            for exception in found.iter_mut().skip(before) {
                exception.in_machine = in_machine;
            }
        }
    });
    if walked.is_err() || found.is_empty() {
        return;
    }
    found.sort_by(|a, b| a.place.cmp(&b.place));
    println!(
        "\nLes exceptions de ce jeu ({}) : chacune éteint une règle à un endroit.",
        found.len()
    );
    for exception in &found {
        let mut notes = String::new();
        if exception.whole_file {
            notes.push_str("  [tout le fichier]");
        }
        if exception.in_machine {
            notes.push_str("  [dans la machine]");
        }
        if exception.in_tests {
            notes.push_str("  [des tests]");
        }
        println!(
            "  {}{notes}\n      {} : {}",
            exception.place, exception.lints, exception.reason
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exception_is_read_with_its_lints_and_its_reason() {
        let text = "#![expect(\n    clippy::arithmetic_side_effects,\n    reason = \"des \
                    compteurs\"\n)]\n\nfn f() {}\n\n#[expect(clippy::panic, reason = \"voulu\")]\nfn g() {}\n";
        let mut found = Vec::new();
        exceptions_in(text, "a.rs", &mut found);
        let seen: Vec<(&str, &str, &str, bool)> = found
            .iter()
            .map(|one| {
                (
                    one.place.as_str(),
                    one.lints.as_str(),
                    one.reason.as_str(),
                    one.whole_file,
                )
            })
            .collect();
        assert_eq!(
            seen,
            [
                (
                    "a.rs:1",
                    "clippy::arithmetic_side_effects",
                    "des compteurs",
                    true
                ),
                ("a.rs:8", "clippy::panic", "voulu", false),
            ]
        );
    }

    #[test]
    fn the_advice_names_the_reason_and_the_way_out() {
        let lints = BTreeSet::from([
            "clippy::arithmetic_side_effects".to_owned(),
            "clippy::needless_pass_by_value".to_owned(),
        ]);
        let text = advice(&lints);
        assert!(text.contains("Un dépassement d'entier arrête la partie."));
        assert!(text.contains("needless_pass_by_value : le message"));
    }
}
