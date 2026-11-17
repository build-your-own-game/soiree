//! Les règles de la console, embarquées dans l'outil, et `soiree rules` qui
//! les écrit dans un jeu.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

pub const TOOLCHAIN: &str = include_str!("../../regles/rust-toolchain.toml");
pub const CLIPPY: &str = include_str!("../../regles/clippy.toml");
const ADVICE: &str = include_str!("../../regles/conseils.toml");
const LINTS: &str = include_str!("../../regles/lints.toml");

/// Les extensions des langages que la console n'accepte pas : tout est en Rust.
pub const FOREIGN: [&str; 7] = ["js", "mjs", "cjs", "jsx", "ts", "tsx", "py"];

/// Le dossier où la console écrit le journal des parties et l'état gardé :
/// git ne le suit pas.
pub const IGNORED: &str = "/.soiree/";
const IGNORED_COMMENT: &str = "# Le journal des parties et l'état gardé, que la console écrit.";
const PROFILE: &str = "[profile.release]";
const OVERFLOW_CHECKS: &str = "overflow-checks = true";
const PROFILE_COMMENT: &str =
    "# Un dépassement d'entier arrête le banc au lieu de passer en silence, même en release.";

/// Ce que `cargo metadata` dit d'un jeu.
#[derive(Deserialize)]
pub struct Metadata {
    pub packages: Vec<Package>,
    pub workspace_root: PathBuf,
    pub target_directory: PathBuf,
}

#[derive(Deserialize)]
pub struct Package {
    pub name: String,
    pub manifest_path: PathBuf,
    pub targets: Vec<Target>,
    /// Ce que le crate emprunte, tel que son `Cargo.toml` le demande.
    pub dependencies: Vec<Dependency>,
    /// `[package.metadata]` du `Cargo.toml` du crate.
    metadata: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct Dependency {
    pub name: String,
    pub features: Vec<String>,
    /// `None` pour `[dependencies]`, `"dev"` pour `[dev-dependencies]`.
    pub kind: Option<String>,
}

#[derive(Deserialize)]
pub struct Target {
    pub name: String,
    pub kind: Vec<String>,
    pub crate_types: Vec<String>,
    pub src_path: PathBuf,
}

impl Target {
    fn is(&self, kind: &str) -> bool {
        self.kind.iter().any(|one| one == kind)
    }
}

/// Pourquoi la console pose une règle, et par où sortir.
#[derive(Deserialize)]
pub struct Advice {
    pub pourquoi: String,
    pub remede: String,
}

/// Les conseils de la console, par nom de lint (sans `clippy::`).
pub fn advice() -> std::collections::BTreeMap<String, Advice> {
    toml::from_str(ADVICE).unwrap_or_default()
}

impl Metadata {
    /// Les machines du jeu : les crates qui portent ses règles.
    pub fn machines(&self) -> impl Iterator<Item = &Package> {
        self.packages.iter().filter(|package| package.is_machine())
    }

    /// Les programmes du jeu : chaque cible binaire, avec son crate.
    pub fn binaries(&self) -> Vec<(&Package, &Target)> {
        self.packages
            .iter()
            .flat_map(|package| {
                package
                    .targets
                    .iter()
                    .filter(|target| target.is("bin"))
                    .map(move |target| (package, target))
            })
            .collect()
    }

    /// Le chemin d'un fichier du jeu, depuis sa racine.
    pub fn shown(&self, path: &Path) -> String {
        path.strip_prefix(&self.workspace_root)
            .unwrap_or(path)
            .display()
            .to_string()
    }

    /// Les `clippy.toml` posés ailleurs qu'à la racine. Clippy lit le plus
    /// proche d'un crate : un fichier en trop remplacerait, pour ce crate,
    /// les réglages de la console.
    pub fn stray_clippy_files(&self) -> Vec<PathBuf> {
        let mut stray = Vec::new();
        for package in &self.packages {
            for name in ["clippy.toml", ".clippy.toml"] {
                let path = package.dir().join(name);
                if path.exists() && path != self.workspace_root.join("clippy.toml") {
                    stray.push(path);
                }
            }
        }
        stray
    }
}

impl Package {
    /// Le dossier du crate.
    pub fn dir(&self) -> &Path {
        self.manifest_path.parent().unwrap_or(Path::new("."))
    }

    /// La machine du jeu : `[package.metadata.soiree] machine = true`.
    pub fn is_machine(&self) -> bool {
        self.metadata
            .as_ref()
            .and_then(|metadata| metadata.get("soiree")?.get("machine")?.as_bool())
            == Some(true)
    }

    /// Le nom de la bibliothèque du crate, tel qu'on l'écrit dans le code
    /// (`ma_machine::Jeu`) : il peut différer du nom du crate.
    pub fn library(&self) -> Option<String> {
        self.targets
            .iter()
            .find(|target| target.is("lib"))
            .map(|target| target.name.replace('-', "_"))
    }

    /// Le crate a une bibliothèque : ce que les autres crates lui empruntent.
    pub fn has_library(&self) -> bool {
        self.library().is_some()
    }

    /// Le crate a un exemple de ce nom (`examples/<nom>.rs`).
    pub fn has_example(&self, name: &str) -> bool {
        self.targets
            .iter()
            .any(|target| target.name == name && target.is("example"))
    }

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

/// Écrit le fichier s'il change, et le dit.
pub fn write(path: &Path, text: &str) -> Result<(), String> {
    if fs::read_to_string(path).ok().as_deref() == Some(text) {
        return Ok(());
    }
    fs::write(path, text).map_err(|error| format!("{} : {error}", path.display()))?;
    println!("  écrit {}", path.display());
    Ok(())
}

/// Une ligne de commentaire, ou vide.
fn is_blank_or_comment(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with('#') || line.is_empty()
}

/// Le manifeste sans ses sections de lints, ni les commentaires et les lignes
/// vides posés juste au-dessus d'elles. Les commentaires posés au-dessus de la
/// section suivante restent avec elle.
fn without_lints(manifest: &str, prefix: &str) -> String {
    let header = format!("[{prefix}");
    let mut kept: Vec<&str> = Vec::new();
    // Pendant qu'on saute les lints : les commentaires qui suivent, peut-être
    // ceux de la section d'après.
    let mut pending: Vec<&str> = Vec::new();
    let mut skipping = false;
    for line in manifest.lines() {
        if line.starts_with('[') {
            let lints = line.starts_with(&header);
            if lints {
                while kept.last().is_some_and(|last| is_blank_or_comment(last)) {
                    let _: Option<&str> = kept.pop();
                }
            } else if skipping {
                kept.append(&mut pending);
            }
            pending.clear();
            skipping = lints;
        }
        if !skipping {
            kept.push(line);
        } else if is_blank_or_comment(line) {
            pending.push(line);
        } else {
            pending.clear();
        }
    }
    while kept.last().is_some_and(|last| last.trim().is_empty()) {
        let _: Option<&str> = kept.pop();
    }
    kept.join("\n")
}

/// La clé d'une ligne `clé = valeur`.
fn key(line: &str) -> Option<&str> {
    let (key, _) = line.split_once('=')?;
    Some(key.trim().trim_matches('"'))
}

/// Le manifeste avec `overflow-checks = true` dans `[profile.release]` : la
/// clé est ajoutée ou corrigée, le reste de la section ne bouge pas. Sans
/// section, elle est ajoutée à la fin.
pub fn with_overflow_checks(manifest: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut found = false;
    let mut inside = false;
    for line in manifest.lines() {
        if line.trim_start().starts_with('[') {
            inside = line.split('#').next().map(str::trim) == Some(PROFILE);
            out.push(line);
            if inside {
                found = true;
                out.push(OVERFLOW_CHECKS);
            }
            continue;
        }
        if !(inside && key(line) == Some("overflow-checks")) {
            out.push(line);
        }
    }
    if found {
        out.join("\n")
    } else {
        format!(
            "{}\n\n{PROFILE_COMMENT}\n{PROFILE}\n{OVERFLOW_CHECKS}",
            manifest.trim_end()
        )
    }
}

/// Un dépassement d'entier arrête-t-il le programme, en release aussi ?
pub fn checks_overflow(manifest: &toml::Table) -> bool {
    manifest
        .get("profile")
        .and_then(|profile| profile.get("release"))
        .and_then(|release| release.get("overflow-checks"))
        .and_then(toml::Value::as_bool)
        == Some(true)
}

/// Le `.gitignore` laisse-t-il de côté le dossier de la console ?
pub fn ignores_soiree(gitignore: &str) -> bool {
    gitignore.lines().any(|line| line.trim() == IGNORED)
}

/// Le `.gitignore` avec la ligne du dossier de la console, ajoutée une fois.
pub fn with_soiree_ignored(gitignore: &str) -> String {
    if ignores_soiree(gitignore) {
        return gitignore.to_owned();
    }
    let mut text = gitignore.to_owned();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(IGNORED_COMMENT);
    text.push('\n');
    text.push_str(IGNORED);
    text.push('\n');
    text
}

/// Le `Cargo.toml` de la racine avec les règles de la console : la table des
/// lints, à la fin, et `overflow-checks`.
fn with_rules(manifest: &str, prefix: &str) -> Result<String, String> {
    let updated = format!(
        "{}\n\n{}",
        with_overflow_checks(&without_lints(manifest, prefix)),
        lints_block(prefix)
    );
    match updated.parse::<toml::Table>() {
        Ok(_) => Ok(updated),
        Err(error) => Err(format!(
            "Cargo.toml : une fois les règles ajoutées, il ne se lit plus ({error}).\n\
             Écris toi-même, à la racine, `{PROFILE}` avec `{OVERFLOW_CHECKS}`, et retire toute \
             autre façon d'écrire `overflow-checks`, puis relance `soiree rules`."
        )),
    }
}

/// `soiree rules` : écrit la version de Rust, les réglages de Clippy, la
/// table des lints et le profil de release, branche chaque crate sur cette
/// table, puis écrit les deux fichiers que tous les jeux partagent : le banc
/// de la machine et le programme.
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
    write(&manifest_path, &with_rules(&text, prefix)?)?;
    let gitignore = root.join(".gitignore");
    let ignored = match fs::read_to_string(&gitignore) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("{} : {error}", gitignore.display())),
    };
    write(&gitignore, &with_soiree_ignored(&ignored))?;

    let metadata = metadata(root)?;
    if prefix == "workspace.lints" {
        for package in &metadata.packages {
            if !read_toml(&package.manifest_path)?.contains_key("lints") {
                let text = fs::read_to_string(&package.manifest_path)
                    .map_err(|error| format!("{} : {error}", package.manifest_path.display()))?;
                let updated = format!("{}\n\n[lints]\nworkspace = true\n", text.trim_end());
                write(&package.manifest_path, &updated)?;
            }
        }
    }
    crate::cartridge::write(&metadata)?;
    // Les réglages de Clippy sont ceux de la console, à la racine seulement.
    for path in metadata.stray_clippy_files() {
        fs::remove_file(&path).map_err(|error| format!("{} : {error}", path.display()))?;
        println!("  supprimé {}", path.display());
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

    /// Chaque fichier de règles se lit, seul et assemblé.
    #[test]
    fn the_rule_files_parse() {
        let _: toml::Table = CLIPPY.parse().unwrap();
        let advice = advice();
        assert!(advice.contains_key("arithmetic_side_effects"));
        assert!(advice.values().all(|one| !one.remede.is_empty()));
    }

    /// `soiree rules` ne change rien à un jeu déjà en règle, et garde ce qui
    /// n'est pas à lui.
    #[test]
    fn writing_the_rules_twice_changes_nothing() {
        for manifest in ["Cargo.toml", "modele/Cargo.toml"] {
            let text = fs::read_to_string(repo(manifest)).unwrap();
            let once = with_rules(&text, "workspace.lints").unwrap();
            assert_eq!(with_rules(&once, "workspace.lints").unwrap(), once);
            assert!(checks_overflow(&once.parse().unwrap()));
        }
        let root = fs::read_to_string(repo("Cargo.toml")).unwrap();
        assert!(checks_overflow(&root.parse().unwrap()));
        // Le commentaire posé entre les lints et le profil reste avec le profil.
        let once = with_rules(&root, "workspace.lints").unwrap();
        assert!(
            once.contains(
                "# Un dépassement d'entier est un bug, même en release.\n[profile.release]"
            )
        );
    }

    #[test]
    fn the_release_profile_checks_overflows() {
        assert_eq!(
            with_overflow_checks("[package]\nname = \"jeu\"\n"),
            format!(
                "[package]\nname = \"jeu\"\n\n{PROFILE_COMMENT}\n[profile.release]\noverflow-checks = true"
            )
        );
        let text = "[profile.release]\nlto = true\noverflow-checks = false\n\n[profile.dev]\nopt-level = 1";
        let fixed = with_overflow_checks(text);
        assert_eq!(
            fixed,
            "[profile.release]\noverflow-checks = true\nlto = true\n\n[profile.dev]\nopt-level = 1"
        );
        assert_eq!(with_overflow_checks(&fixed), fixed);
        assert!(checks_overflow(&fixed.parse().unwrap()));
        assert!(!checks_overflow(&text.parse().unwrap()));
    }

    #[test]
    fn the_gitignore_leaves_out_the_console_s_folder_once() {
        let line = format!("{IGNORED_COMMENT}\n{IGNORED}\n");
        assert_eq!(with_soiree_ignored(""), line);
        let text = with_soiree_ignored("target/");
        assert_eq!(text, format!("target/\n{line}"));
        assert_eq!(with_soiree_ignored(&text), text);
        assert!(ignores_soiree(&text));
        assert!(!ignores_soiree("target/\n.soiree/journal\n"));
    }
}
