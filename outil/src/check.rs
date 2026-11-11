//! `soiree check` : un jeu suit-il les règles de la console, et se
//! branche-t-il ? Chaque refus dit quoi corriger.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::plug;
use crate::rules::{self, Metadata};

/// Les dossiers qu'on ne fouille pas : ce qui se fabrique, et l'historique.
const SKIPPED: [&str; 3] = ["target", ".git", "node_modules"];

struct Report {
    failures: u32,
}

impl Report {
    fn step(&mut self, name: &str, outcome: Result<(), String>) -> bool {
        match outcome {
            Ok(()) => {
                println!("✔ {name}");
                true
            }
            Err(fix) => {
                println!("✘ {name}\n{fix}\n");
                self.failures = self.failures.saturating_add(1);
                false
            }
        }
    }
}

pub fn run(dir: &Path) -> Result<(), String> {
    let metadata = rules::metadata(dir)?;
    let root = metadata.workspace_root.clone();
    println!("soiree {} vérifie {}\n", crate::VERSION, root.display());
    let mut report = Report { failures: 0 };

    let _: bool = report.step("les règles sont en place", rule_files(&metadata));
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
        cargo(
            &root,
            &["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
            "Corrige chaque erreur ci-dessus. Une exception se justifie sur place : #[expect(lint, reason = \"…\")].",
        ),
    );
    for page in metadata.packages.iter().filter(|package| package.is_page()) {
        compiles &= report.step(
            &format!("le linteur, pour le navigateur ({})", page.name),
            cargo(
                &root,
                &[
                    "clippy",
                    "--package",
                    &page.name,
                    "--target",
                    "wasm32-unknown-unknown",
                    "--",
                    "-D",
                    "warnings",
                ],
                "Corrige chaque erreur ci-dessus. Une exception se justifie sur place : #[expect(lint, reason = \"…\")].",
            ),
        );
    }
    compiles &= report.step(
        "les tests du jeu",
        cargo(
            &root,
            &["test", "--workspace"],
            "Un test échoue : corrige le jeu, ou le test s'il se trompe.",
        ),
    );
    if compiles {
        let _: bool = report.step("le jeu se branche", plug::run(&root));
    } else {
        println!("– le jeu se branche : pas essayé tant que le reste ne passe pas");
    }

    if report.failures == 0 {
        println!(
            "\nCe jeu passe la vérification de soiree {}.",
            crate::VERSION
        );
        Ok(())
    } else {
        Err(format!(
            "\n{} point(s) à corriger : ce jeu ne passe pas la vérification de soiree {}.",
            report.failures,
            crate::VERSION
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

/// La version de Rust, les réglages de Clippy et la table des lints sont ceux
/// de la console, et chaque crate du jeu s'y soumet.
fn rule_files(metadata: &Metadata) -> Result<(), String> {
    let root = &metadata.workspace_root;
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
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "  {}\n  Lance `soiree rules` : il écrit les règles de cette version dans le jeu.",
            wrong.join("\n  ")
        ))
    }
}

/// Aucun fichier d'un autre langage : les pages aussi sont en Rust.
fn only_rust(root: &Path) -> Result<(), String> {
    let mut foreign = Vec::new();
    collect_foreign(root, root, &mut foreign).map_err(|error| format!("  {error}"))?;
    if foreign.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "  {}\n  Réécris ces fichiers en Rust. Dans une page, seul reste le chargeur de `index.html`.",
            foreign.join("\n  ")
        ))
    }
}

fn collect_foreign(root: &Path, dir: &Path, foreign: &mut Vec<String>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path: PathBuf = entry.path();
        if entry.file_type()?.is_dir() {
            if !SKIPPED.iter().any(|skipped| entry.file_name() == *skipped) {
                collect_foreign(root, &path, foreign)?;
            }
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| rules::FOREIGN.contains(&extension))
        {
            foreign.push(
                path.strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string(),
            );
        }
    }
    Ok(())
}
