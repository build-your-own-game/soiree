//! Les règles de la console, embarquées dans l'outil, et `soiree rules` qui
//! les écrit dans un jeu.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

pub const TOOLCHAIN: &str = include_str!("../../regles/rust-toolchain.toml");
pub const CLIPPY: &str = include_str!("../../regles/clippy.toml");
const LINTS: &str = include_str!("../../regles/lints.toml");

/// Les extensions des langages que la console n'accepte pas : tout est en Rust.
pub const FOREIGN: [&str; 7] = ["js", "mjs", "cjs", "jsx", "ts", "tsx", "py"];

/// Ce que `cargo metadata` dit d'un jeu.
#[derive(Deserialize)]
pub struct Metadata {
    pub packages: Vec<Package>,
    pub workspace_root: PathBuf,
}

#[derive(Deserialize)]
pub struct Package {
    pub name: String,
    pub manifest_path: PathBuf,
    pub targets: Vec<Target>,
}

#[derive(Deserialize)]
pub struct Target {
    pub crate_types: Vec<String>,
}

impl Package {
    /// Un crate compilé pour le navigateur : une page.
    pub fn is_page(&self) -> bool {
        self.targets
            .iter()
            .any(|target| target.crate_types.iter().any(|kind| kind == "cdylib"))
    }
}

pub fn metadata(dir: &Path) -> Result<Metadata, String> {
    let output = Command::new("cargo")
        .current_dir(dir)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .map_err(|error| format!("impossible de lancer cargo : {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo ne lit pas ce dossier comme un projet Rust :\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| format!("cargo metadata : {error}"))
}

/// `[workspace.lints]` pour un workspace, `[lints]` pour un crate seul.
pub fn lints_prefix(manifest: &toml::Table) -> &'static str {
    if manifest.contains_key("workspace") {
        "workspace.lints"
    } else {
        "lints"
    }
}

/// La table des lints telle qu'elle s'écrit dans le `Cargo.toml` d'un jeu.
pub fn lints_block(prefix: &str) -> String {
    LINTS
        .replace("[rust]", &format!("[{prefix}.rust]"))
        .replace("[clippy]", &format!("[{prefix}.clippy]"))
}

/// La table des lints de la console, pour la comparer à celle d'un jeu.
pub fn lints_table() -> Result<toml::Table, String> {
    LINTS
        .parse()
        .map_err(|error| format!("regles/lints.toml : {error}"))
}

pub fn read_toml(path: &Path) -> Result<toml::Table, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("{} : {error}", path.display()))?;
    text.parse()
        .map_err(|error| format!("{} : {error}", path.display()))
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    fs::write(path, text).map_err(|error| format!("{} : {error}", path.display()))?;
    println!("  écrit {}", path.display());
    Ok(())
}

/// Le manifeste sans ses sections de lints, ni les commentaires et les lignes
/// vides posés juste au-dessus d'elles.
fn without_lints(manifest: &str, prefix: &str) -> String {
    let header = format!("[{prefix}");
    let mut kept: Vec<&str> = Vec::new();
    let mut skipping = false;
    for line in manifest.lines() {
        if line.starts_with('[') {
            skipping = line.starts_with(&header);
            if skipping {
                while kept
                    .last()
                    .is_some_and(|last| last.starts_with('#') || last.trim().is_empty())
                {
                    let _: Option<&str> = kept.pop();
                }
            }
        }
        if !skipping {
            kept.push(line);
        }
    }
    while kept.last().is_some_and(|last| last.trim().is_empty()) {
        let _: Option<&str> = kept.pop();
    }
    kept.join("\n")
}

/// `soiree rules` : écrit la version de Rust, les réglages de Clippy et la
/// table des lints, puis branche chaque crate sur cette table.
pub fn run(root: &Path) -> Result<(), String> {
    // La racine s'écrit avant d'interroger cargo : il refuse un workspace dont
    // les crates demandent une table de lints qui n'existe pas encore.
    let manifest_path = root.join("Cargo.toml");
    let text = fs::read_to_string(&manifest_path).map_err(|error| {
        format!(
            "{} : {error}\nLance `soiree rules` à la racine du jeu, là où est son Cargo.toml.",
            manifest_path.display()
        )
    })?;
    write(&root.join("rust-toolchain.toml"), TOOLCHAIN)?;
    write(&root.join("clippy.toml"), CLIPPY)?;

    let prefix = lints_prefix(&read_toml(&manifest_path)?);
    let updated = format!(
        "{}\n\n{}",
        without_lints(&text, prefix),
        lints_block(prefix)
    );
    write(&manifest_path, &updated)?;

    if prefix == "workspace.lints" {
        for package in &metadata(root)?.packages {
            if !read_toml(&package.manifest_path)?.contains_key("lints") {
                let text = fs::read_to_string(&package.manifest_path)
                    .map_err(|error| format!("{} : {error}", package.manifest_path.display()))?;
                let updated = format!("{}\n\n[lints]\nworkspace = true\n", text.trim_end());
                write(&package.manifest_path, &updated)?;
            }
        }
    }
    println!(
        "Les règles de soiree {} sont en place. Lance `soiree check`.",
        crate::VERSION
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(path: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(path)
    }

    /// La console s'applique ses propres règles, et le jeu de départ aussi.
    #[test]
    fn the_console_and_the_starter_game_follow_the_rules() {
        let rules = toml::Value::Table(lints_table().unwrap());
        for manifest in ["Cargo.toml", "modele/Cargo.toml"] {
            let manifest = read_toml(&repo(manifest)).unwrap();
            assert_eq!(manifest["workspace"]["lints"], rules);
        }
        for dir in ["", "modele"] {
            let dir = repo(dir);
            assert_eq!(
                fs::read_to_string(dir.join("rust-toolchain.toml")).unwrap(),
                TOOLCHAIN
            );
            assert_eq!(fs::read_to_string(dir.join("clippy.toml")).unwrap(), CLIPPY);
        }
    }

    /// Les textes qui disent quoi installer nomment la version courante.
    #[test]
    fn the_install_command_names_this_version() {
        let tag = format!("--tag v{} soiree", crate::VERSION);
        for file in ["README.md", "REJOINDRE.md"] {
            let text = fs::read_to_string(repo(file)).unwrap();
            assert!(text.contains(&tag), "{file} ne nomme pas {tag}");
        }
    }

    /// `soiree rules` ne change rien à un jeu déjà en règle.
    #[test]
    fn writing_the_rules_twice_changes_nothing() {
        let text = fs::read_to_string(repo("modele/Cargo.toml")).unwrap();
        let prefix = "workspace.lints";
        let once = format!(
            "{}\n\n{}",
            without_lints(&text, prefix),
            lints_block(prefix)
        );
        assert_eq!(once, text);
    }
}
