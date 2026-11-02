//! Conservative snapshot of declared inputs, including ignored local source files.
//!
//! This is not a hermetic execution environment. The recorded assumptions state
//! which external reads and dependency caches remain outside this snapshot.
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::model::Plan;
use crate::util::{self, Result};

const ARTIFACT_DIRECTORIES: &[&str] =
    &[".git", "target", "node_modules", ".soiree", "certification"];
type Environment = BTreeMap<OsString, OsString>;

/// Whether a passage cannot go without Node.
#[derive(Clone, Copy)]
enum Node {
    Required,
    Optional,
}

/// How a system spells programs and variables.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Spelling {
    Posix,
    Windows,
}

const SPELLING: Spelling = if cfg!(windows) {
    Spelling::Windows
} else {
    Spelling::Posix
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Identity {
    pub value: String,
    pub source_roots: Vec<String>,
    pub files: usize,
    /// Entries left out of the value by the rules of `assumptions`, each named.
    #[serde(default)]
    pub excluded: Vec<String>,
    /// Each hashed source, to name what changed between two captures.
    #[serde(skip)]
    digests: BTreeMap<String, String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
}

/// The files of a source root: what is hashed, and what is left out and why.
#[derive(Debug, Default)]
struct Sources {
    files: Vec<PathBuf>,
    excluded: Vec<String>,
    digests: BTreeMap<String, String>,
}

/// Cargo refreshes a stale `Cargo.lock` at its first command. Doing it here puts
/// that rewrite before the snapshot, where it is not a change during the run;
/// otherwise the snapshot's own `--locked` metadata refuses the run. A locked
/// run never comes here: it must still refuse a stale lock, and rewrites nothing.
pub fn settle_lock(dir: &Path) {
    if dir.join("Cargo.toml").is_file() {
        let _: Result<std::process::Output> =
            util::run(dir, "cargo", &["metadata", "--format-version", "1"]);
    }
}

/// The sentence for two captures that differ, `what` being what changed.
pub fn drift(
    before: &Result<Identity>,
    after: &Result<Identity>,
    dir: &Path,
    what: &str,
) -> Option<String> {
    let value = |capture: &Result<Identity>| capture.as_ref().ok().map(|i| i.value.clone());
    (value(before) != value(after)).then(|| match (before, after) {
        (Ok(before), Ok(after)) => format!("{what}{}", changes(before, after, dir)),
        _ => what.to_owned(),
    })
}

/// The sources that differ between two captures, for the reader of a failed
/// passage: ` : a, b et 3 autres`, or nothing when the sources did not change.
fn changes(before: &Identity, after: &Identity, dir: &Path) -> String {
    const SHOWN: usize = 5;
    let changed: Vec<String> = before
        .digests
        .iter()
        .filter(|(path, digest)| after.digests.get(*path) != Some(digest))
        .map(|(path, _)| path)
        .chain(
            after
                .digests
                .keys()
                .filter(|path| !before.digests.contains_key(*path)),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|path| util::shown(Path::new(path), dir))
        .collect();
    let named = changed
        .iter()
        .take(SHOWN)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    match changed.len() {
        0 => String::new(),
        count if count <= SHOWN => format!(" : {named}"),
        count => format!(" : {named} et {} autres", count.saturating_sub(SHOWN)),
    }
}

pub fn capture(
    dir: &Path,
    plan: &Plan,
    selected: &BTreeSet<String>,
    observer: Option<&Path>,
    parameters: &[String],
) -> Result<Identity> {
    let environment = environment();
    validate_campaign(&environment)?;
    let dir = source_root(dir)?;
    let metadata = metadata(&dir)?;
    let roots = source_roots(&dir, &metadata, observer)?;
    let graph = metadata
        .get("resolve")
        .filter(|value| !value.is_null())
        .ok_or("graphe Cargo absent de l’identité")?;
    let mut hash = Sha256::new();
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    add_file(&mut hash, "controller", &executable)?;
    add(
        &mut hash,
        "plan",
        &serde_json::to_vec(plan).map_err(|e| e.to_string())?,
    );
    add(
        &mut hash,
        "selection",
        &serde_json::to_vec(selected).map_err(|e| e.to_string())?,
    );
    add(
        &mut hash,
        "parameters",
        &serde_json::to_vec(parameters).map_err(|e| e.to_string())?,
    );
    add(
        &mut hash,
        "dependency-graph",
        &serde_json::to_vec(graph).map_err(|e| e.to_string())?,
    );
    let sources = hash_sources(&mut hash, &roots)?;
    hash_configs(&mut hash, &dir, &environment)?;
    hash_environment(&mut hash, &dir, &environment)?;
    hash_tools(&mut hash, &dir, &environment)?;
    let node = if observer.is_some() {
        Node::Required
    } else {
        Node::Optional
    };
    hash_node(&mut hash, &dir, &environment, node)?;
    Ok(Identity {
        value: hex(&hash.finalize()),
        source_roots: roots
            .iter()
            .map(|root| root.display().to_string())
            .collect(),
        files: sources.digests.len(),
        excluded: sources.excluded,
        digests: sources.digests,
        assumptions: assumptions(),
    })
}

fn metadata(dir: &Path) -> Result<Value> {
    if !dir.join("Cargo.toml").is_file() {
        return Ok(
            serde_json::json!({"workspace_root":dir, "packages":[], "resolve":{"kind":"static-tree-without-Cargo"}}),
        );
    }
    let metadata = util::run(
        dir,
        "cargo",
        &[
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--all-features",
        ],
    )?;
    if !metadata.status.success() {
        return Err(format!(
            "identité des dépendances non établie : {}",
            String::from_utf8_lossy(&metadata.stderr)
        ));
    }
    serde_json::from_slice(&metadata.stdout).map_err(|e| e.to_string())
}

fn assumptions() -> Vec<String> {
    [
        "L’empreinte couvre tous les fichiers des racines du projet, des dépendances path/git et du contrôleur déclaré, y compris les fichiers ignorés par Git et dist. Les répertoires .git, target, node_modules, .soiree et certification sont exclus ; leurs contenus ne doivent pas servir d’entrées source non déclarées.",
        "Sont aussi exclus, et nommés dans cette provenance : les fichiers que dépose la machine (.DS_Store, ._*, Thumbs.db, desktop.ini, sauvegardes et verrous d’éditeur), les notes personnelles que Git ne suit pas (.md ou .txt nommés notes, todo, scratch ou brouillon, ou placés dans notes/) et un dossier data que Git ignore, état local du jeu en marche. Un fichier suivi par Git reste toujours une entrée, et hors de Git rien de cela n’est présumé. Un test qui lirait ces fichiers ne serait pas couvert.",
        "L’exécution n’est pas hermétique : les lectures hors de ces racines, les autres variables d’environnement, les services réseau, les bibliothèques dynamiques et les commandes invoquées indirectement par les scripts restent des hypothèses du contrôle.",
        "Les dépendances de registre et les paquets node_modules sont identifiés par leurs verrous et la chaîne d’outillage ; l’intégrité de leurs caches locaux est une hypothèse, leur contenu n’est pas entièrement haché.",
        "Les empreintes avant/après détectent les changements persistants des entrées couvertes ; elles ne constituent pas une isolation contre une modification temporaire restaurée pendant le passage.",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn source_root(path: &Path) -> Result<PathBuf> {
    let root = std::fs::canonicalize(path).map_err(|e| format!("{} : {e}", path.display()))?;
    if !root.is_dir() {
        return Err(format!("racine source non répertoire : {}", path.display()));
    }
    Ok(root)
}

fn source_roots(
    dir: &Path,
    metadata: &Value,
    observer: Option<&Path>,
) -> Result<BTreeSet<PathBuf>> {
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or("paquets Cargo absents de l’identité")?;
    let workspace = metadata
        .get("workspace_root")
        .and_then(Value::as_str)
        .ok_or("racine du workspace Cargo absente de l’identité")?;
    let mut roots = BTreeSet::from([dir.to_owned(), source_root(Path::new(workspace))?]);
    for package in packages {
        let registry = package
            .get("source")
            .and_then(Value::as_str)
            .is_some_and(|source| source.starts_with("registry+"));
        if registry {
            continue;
        }
        let path = package
            .get("manifest_path")
            .and_then(Value::as_str)
            .and_then(|path| Path::new(path).parent())
            .ok_or("racine d’un paquet local/git inconnue")?;
        roots.insert(source_root(path)?);
    }
    if let Some(observer) = observer {
        roots.insert(source_root(observer)?);
    }
    Ok(roots)
}

fn hash_sources(hash: &mut Sha256, roots: &BTreeSet<PathBuf>) -> Result<Sources> {
    let mut sources = Sources::default();
    for root in roots {
        add(hash, "root", path_text(root)?.as_bytes());
        let found = source_files(root)?;
        for path in found.files {
            let name = path.strip_prefix(root).map_err(|e| e.to_string())?;
            let digest = add_file(hash, path_text(name)?, &path)?;
            sources.digests.insert(path_text(&path)?.to_owned(), digest);
        }
        sources.excluded.extend(found.excluded);
    }
    sources.excluded.sort();
    sources.excluded.dedup();
    Ok(sources)
}

fn add(hash: &mut Sha256, name: &str, bytes: &[u8]) {
    add_digest(hash, name, &Sha256::digest(bytes));
}

fn add_digest(hash: &mut Sha256, name: &str, digest: &[u8]) {
    hash.update(name.as_bytes());
    hash.update(b"\0");
    hash.update(digest);
    hash.update(b"\n");
}

fn add_file(hash: &mut Sha256, name: &str, path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(|e| format!("{} : {e}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0u8; 65536];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| format!("{} : {e}", path.display()))?;
        if read == 0 {
            break;
        }
        let bytes = buffer.get(..read).ok_or("lecture de fichier hors tampon")?;
        digest.update(bytes);
    }
    let digest = digest.finalize();
    add_digest(hash, name, &digest);
    Ok(hex(&digest))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _: std::fmt::Result = write!(text, "{byte:02x}");
        text
    })
}

fn path_text(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| format!("chemin non UTF-8 non pris en charge : {}", path.display()))
}

fn source_files(root: &Path) -> Result<Sources> {
    let tracked = tracked_files(root)?;
    let mut found = Sources::default();
    walk(root, root, tracked.as_ref(), &mut found)?;
    // A tracked file missing from the working tree is never silently omitted.
    for relative in tracked.iter().flatten() {
        if machine_noise(relative)
            || relative.parent().is_some_and(|parent| {
                parent.components().any(|part| {
                    ARTIFACT_DIRECTORIES
                        .iter()
                        .any(|skip| part.as_os_str() == *skip)
                })
            })
        {
            continue;
        }
        let path = root.join(relative);
        let kind = std::fs::symlink_metadata(&path)
            .map_err(|e| {
                format!(
                    "source suivie absente ou illisible {} : {e}",
                    path.display()
                )
            })?
            .file_type();
        if kind.is_symlink() {
            return Err(symbolic_source(&path));
        }
        if !kind.is_file() {
            return Err(format!("source suivie non fichier : {}", path.display()));
        }
        found.files.push(path);
    }
    found.files.sort();
    found.files.dedup();
    Ok(found)
}

fn tracked_files(root: &Path) -> Result<Option<BTreeSet<PathBuf>>> {
    if !has_git_ancestor(root)? {
        return Ok(None);
    }
    let tracked = util::run(root, "git", &["ls-files", "-z", "--cached", "--", "."])?;
    if !tracked.status.success() {
        return Err(format!("inventaire Git impossible dans {}", root.display()));
    }
    let tracked = String::from_utf8(tracked.stdout).map_err(|e| e.to_string())?;
    let mut files = BTreeSet::new();
    for name in tracked.split('\0').filter(|name| !name.is_empty()) {
        let relative = Path::new(name);
        if relative.is_absolute()
            || relative
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err("chemin Git hors racine source".to_owned());
        }
        files.insert(relative.to_owned());
    }
    Ok(Some(files))
}

/// What the Finder, Windows and editors leave beside the sources. No build
/// reads these, and they come and go during a passage.
fn machine_noise(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    matches!(name, ".DS_Store" | "Thumbs.db" | "desktop.ini")
        || name.starts_with("._")
        || name.starts_with(".#")
        || name.ends_with('~')
        || path
            .extension()
            .is_some_and(|extension| extension == "swp" || extension == "swo")
}

/// A document someone writes for themselves. Only its name tells it from a
/// document the game reads, so the caller keeps every file Git follows, and
/// `ideas.md` remains an input: a false alarm costs less than a blind spot.
fn personal_note(relative: &Path) -> bool {
    let document = relative
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| matches!(extension.to_lowercase().as_str(), "md" | "txt"));
    let stem = relative
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_lowercase();
    let in_notes = relative
        .parent()
        .is_some_and(|parent| parent.components().any(|part| part.as_os_str() == "notes"));
    document
        && (in_notes
            || stem.contains("note")
            || ["todo", "scratch", "brouillon"]
                .iter()
                .any(|name| stem.starts_with(name)))
}

/// `--no-index`: a followed file inside must not hide the folder's own rule.
/// Any failure, such as a submodule, leaves the folder an input.
fn ignored_by_git(root: &Path, relative: &Path) -> Result<bool> {
    let path = path_text(relative)?;
    let args = ["check-ignore", "--quiet", "--no-index", "--", path];
    Ok(util::run(root, "git", &args)?.status.success())
}

fn has_git_ancestor(root: &Path) -> Result<bool> {
    for ancestor in root.ancestors() {
        match std::fs::symlink_metadata(ancestor.join(".git")) {
            Ok(_) => return Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("métadonnées Git illisibles : {error}")),
        }
    }
    Ok(false)
}

fn walk(
    root: &Path,
    dir: &Path,
    tracked: Option<&BTreeSet<PathBuf>>,
    found: &mut Sources,
) -> Result<()> {
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{} : {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let path = entry.path();
        let relative = path.strip_prefix(root).map_err(|e| e.to_string())?;
        if kind.is_dir() && ARTIFACT_DIRECTORIES.iter().any(|skip| name == *skip) {
            continue;
        }
        if !kind.is_dir() && machine_noise(relative) {
            found.excluded.push(path_text(relative)?.to_owned());
            continue;
        }
        if kind.is_symlink() {
            return Err(symbolic_source(&path));
        }
        if kind.is_dir() {
            // `source_files` puts back any file Git follows inside.
            if name == "data" && tracked.is_some() && ignored_by_git(root, relative)? {
                found.excluded.push(format!("{}/", path_text(relative)?));
            } else {
                walk(root, &path, tracked, found)?;
            }
        } else if kind.is_file() {
            if tracked.is_some_and(|tracked| !tracked.contains(relative)) && personal_note(relative)
            {
                found.excluded.push(path_text(relative)?.to_owned());
            } else {
                found.files.push(path);
            }
        } else {
            return Err(format!(
                "entrée source spéciale non prise en charge : {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn symbolic_source(path: &Path) -> String {
    format!(
        "source symbolique à résoudre explicitement : {}",
        path.display()
    )
}

fn hash_configs(hash: &mut Sha256, dir: &Path, environment: &Environment) -> Result<()> {
    let mut paths = BTreeSet::new();
    for ancestor in dir.ancestors() {
        paths.insert(ancestor.join(".cargo/config"));
        paths.insert(ancestor.join(".cargo/config.toml"));
    }
    let home = environment
        .get(OsStr::new("CARGO_HOME"))
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            environment
                .get(OsStr::new("USERPROFILE"))
                .or_else(|| environment.get(OsStr::new("HOME")))
                .map(|home| Path::new(home).join(".cargo"))
        })
        .ok_or("CARGO_HOME et HOME absents : configuration Cargo globale inconnue")?;
    let home = if home.is_absolute() {
        home
    } else {
        dir.join(home)
    };
    paths.insert(home.join("config"));
    paths.insert(home.join("config.toml"));
    for path in paths {
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(symbolic_source(&path));
            }
            Ok(metadata) if metadata.is_file() => {
                add_file(hash, path_text(&path)?, &path)?;
            }
            Ok(_) => {
                return Err(format!(
                    "configuration Cargo non fichier : {}",
                    path.display()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                add(hash, path_text(&path)?, b"absent");
            }
            Err(error) => return Err(format!("{} : {error}", path.display())),
        }
    }
    Ok(())
}

fn environment() -> Environment {
    kept_variables(std::env::vars_os(), SPELLING)
}

/// Windows spells `Path` as it likes and has `USERPROFILE` for a home: its
/// variable names are folded here, so that the rest reads one spelling.
fn kept_variables(
    variables: impl Iterator<Item = (OsString, OsString)>,
    spelling: Spelling,
) -> Environment {
    let windows = spelling == Spelling::Windows;
    variables
        .map(|(key, value)| {
            (
                if windows {
                    key.to_ascii_uppercase()
                } else {
                    key
                },
                value,
            )
        })
        .filter(|(key, _)| {
            key.to_str().is_some_and(|key| {
                (windows && key == "USERPROFILE")
                    || [
                        "CARGO_",
                        "RUSTC_",
                        "RUSTDOC_",
                        "RUSTUP_",
                        "SOIREE_",
                        "PROPTEST_",
                    ]
                    .iter()
                    .any(|prefix| key.starts_with(prefix))
                    || [
                        "HOME",
                        "PATH",
                        "RUSTC",
                        "RUSTDOC",
                        "RUSTFLAGS",
                        "RUSTDOCFLAGS",
                        "RUST_TEST_THREADS",
                        "SIDEQUEST_BLESS",
                    ]
                    .contains(&key)
            })
        })
        .collect()
}

fn validate_campaign(environment: &Environment) -> Result<()> {
    for key in ["SOIREE_SIM_SEEDS", "PROPTEST_CASES"] {
        if environment
            .get(OsStr::new(key))
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.parse::<u64>().is_ok_and(|count| count == 0))
        {
            return Err("une campagne de zéro cas ne peut pas attester un engagement".to_owned());
        }
    }
    if environment.contains_key(OsStr::new("SIDEQUEST_BLESS")) {
        return Err("SIDEQUEST_BLESS doit être absent pendant une vérification".to_owned());
    }
    Ok(())
}

fn hash_environment(hash: &mut Sha256, dir: &Path, environment: &Environment) -> Result<()> {
    for (key, value) in environment {
        let name = key
            .to_str()
            .ok_or("nom de paramètre d’environnement non UTF-8")?;
        add(hash, name, value.as_encoded_bytes());
    }
    for key in [
        "RUSTC",
        "RUSTDOC",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
    ] {
        if let Some(program) = environment
            .get(OsStr::new(key))
            .filter(|value| !value.is_empty())
        {
            hash_executable(hash, dir, key, program, environment)?;
        }
    }
    Ok(())
}

fn hash_tools(hash: &mut Sha256, dir: &Path, environment: &Environment) -> Result<()> {
    for (program, args) in [("rustc", "-Vv"), ("cargo", "-V")] {
        let resolved = hash_executable(hash, dir, program, OsStr::new(program), environment)?;
        hash_version(hash, dir, program, args)?;
        // rustup proxies share one executable; also identify the selected tool.
        if resolved.file_stem().is_some_and(|name| name == "rustup") {
            let output = util::run(dir, path_text(&resolved)?, &["which", program])?;
            if !output.status.success() {
                return Err(format!("binaire effectif de {program} inconnu"));
            }
            let actual = std::str::from_utf8(&output.stdout)
                .map_err(|e| e.to_string())?
                .trim();
            if actual.is_empty() {
                return Err(format!("binaire effectif de {program} absent"));
            }
            hash_executable(
                hash,
                dir,
                &format!("rustup:{program}"),
                OsStr::new(actual),
                environment,
            )?;
        }
    }
    Ok(())
}

fn hash_version(hash: &mut Sha256, dir: &Path, program: &str, flag: &str) -> Result<()> {
    let output = util::run(dir, program, &[flag])?;
    if !output.status.success() {
        return Err(format!("identité de {program} inconnue"));
    }
    add(hash, program, &output.stdout);
    Ok(())
}

/// The harness runs on Node, so an observed passage cannot go without it. A
/// check hashes the Node it finds exactly as a harness does: what it can call
/// stays an input. Its absence is an input too, as for a missing Cargo config:
/// the identity of a passage without Node is not that of a passage with it,
/// and Node leaving or arriving during a passage is a change of the inputs.
fn hash_node(hash: &mut Sha256, dir: &Path, environment: &Environment, node: Node) -> Result<()> {
    if matches!(node, Node::Optional)
        && find_on_path(dir, OsStr::new("node"), environment, SPELLING)?.is_none()
    {
        add(hash, "node", b"absent");
        return Ok(());
    }
    hash_executable(hash, dir, "node", OsStr::new("node"), environment)?;
    hash_version(hash, dir, "node", "--version")
}

fn hash_executable(
    hash: &mut Sha256,
    dir: &Path,
    name: &str,
    program: &OsStr,
    environment: &Environment,
) -> Result<PathBuf> {
    let path = executable_path(dir, program, environment)?;
    add(hash, &format!("{name}:path"), path_text(&path)?.as_bytes());
    add_file(hash, name, &path)?;
    Ok(path)
}

fn executable_path(dir: &Path, program: &OsStr, environment: &Environment) -> Result<PathBuf> {
    let path = Path::new(program);
    if path.is_absolute() || path.components().count() > 1 {
        let path = if path.is_absolute() {
            path.to_owned()
        } else {
            dir.join(path)
        };
        if !is_executable(&path)? {
            return Err(format!("programme non exécutable : {}", path.display()));
        }
        return std::fs::canonicalize(&path).map_err(|e| e.to_string());
    }
    find_on_path(dir, program, environment, SPELLING)?.ok_or_else(|| {
        format!(
            "programme introuvable pour l’identité : {}",
            program.to_string_lossy()
        )
    })
}

/// What `Command` would start for a bare `program`. On Windows it completes
/// the name with `.exe`, and with no other extension.
fn find_on_path(
    dir: &Path,
    program: &OsStr,
    environment: &Environment,
    spelling: Spelling,
) -> Result<Option<PathBuf>> {
    let search = environment
        .get(OsStr::new("PATH"))
        .ok_or("PATH absent : exécutables non identifiés")?;
    let mut name = program.to_owned();
    if spelling == Spelling::Windows && Path::new(program).extension().is_none() {
        name.push(".exe");
    }
    for root in std::env::split_paths(search) {
        let root = if root.is_absolute() {
            root
        } else {
            dir.join(root)
        };
        let candidate = root.join(&name);
        if is_executable(&candidate)? {
            return std::fs::canonicalize(candidate)
                .map(Some)
                .map_err(|e| e.to_string());
        }
    }
    Ok(None)
}

fn is_executable(path: &Path) -> Result<bool> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("{} : {error}", path.display())),
    };
    if !metadata.is_file() {
        return Ok(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        Ok(metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "soiree-identity-{}-{}",
                std::process::id(),
                FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(std::fs::canonicalize(path).unwrap())
        }
        fn write(&self, name: &str, value: &str) {
            let path = self.0.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, value).unwrap();
        }
        fn git(&self, args: &[&str]) {
            assert!(util::run(&self.0, "git", args).unwrap().status.success());
        }
        fn digest(&self) -> String {
            let mut hash = Sha256::new();
            hash_sources(&mut hash, &BTreeSet::from([self.0.clone()])).unwrap();
            hex(&hash.finalize())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _: std::io::Result<()> = std::fs::remove_dir_all(&self.0);
        }
    }
    fn plan() -> Plan {
        Plan {
            schema: 1,
            policy_version: "test".to_owned(),
            obligations: vec![],
            controls: vec![],
            catalogue: vec![],
            diagnostics: vec![],
        }
    }
    #[test]
    fn ignored_inputs_and_dist_are_real_inputs() {
        let fixture = Fixture::new();
        fixture.git(&["init", "-q"]);
        fixture.write(".gitignore", "/inputs/\n/dist/\n");
        fixture.write(
            "src/lib.rs",
            "pub const INPUT: &str = include_str!(\"../inputs/value.txt\");",
        );
        fixture.write("inputs/value.txt", "before");
        fixture.write("dist/game.wasm", "wasm");
        for name in ["inputs/value.txt", "dist/game.wasm"] {
            let before = fixture.digest();
            fixture.write(name, "changed");
            assert_ne!(before, fixture.digest(), "{name} was omitted");
        }
    }
    #[test]
    fn what_the_machine_drops_beside_the_sources_is_not_an_input() {
        let fixture = Fixture::new();
        fixture.git(&["init", "-q"]);
        fixture.write("src/lib.rs", "pub fn source() {}");
        let before = fixture.digest();
        for name in [
            ".DS_Store",
            "src/.DS_Store",
            "src/._lib.rs",
            "Thumbs.db",
            "desktop.ini",
            "src/lib.rs~",
            "src/.lib.rs.swp",
            "src/.#lib.rs",
        ] {
            fixture.write(name, "noise");
            assert_eq!(before, fixture.digest(), "{name} changed the identity");
        }
        assert!(
            source_files(&fixture.0)
                .unwrap()
                .excluded
                .contains(&".DS_Store".to_owned())
        );
        fixture.write("src/lib.rs", "pub fn changed() {}");
        assert_ne!(before, fixture.digest());
    }
    #[cfg(unix)]
    #[test]
    fn an_editor_lock_symlink_is_noise_and_any_other_symlink_is_still_refused() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        fixture.write("src/lib.rs", "pub fn source() {}");
        symlink("user@host.1234", fixture.0.join("src/.#lib.rs")).unwrap();
        assert!(source_files(&fixture.0).is_ok());
        symlink("lib.rs", fixture.0.join("src/other.rs")).unwrap();
        assert!(source_files(&fixture.0).is_err());
    }
    #[test]
    fn a_personal_note_git_does_not_follow_is_not_an_input() {
        let fixture = Fixture::new();
        fixture.git(&["init", "-q"]);
        fixture.write("src/lib.rs", "pub fn source() {}");
        let before = fixture.digest();
        for name in [
            "notes.md",
            "NOTES.txt",
            "mes-notes.md",
            "TODO.md",
            "scratch.txt",
            "notes/2026-10-31.md",
            "crates/core/notes.md",
        ] {
            fixture.write(name, "ce soir");
            assert_eq!(before, fixture.digest(), "{name} changed the identity");
        }
    }
    #[test]
    fn a_followed_note_a_new_source_and_any_other_document_stay_inputs() {
        let fixture = Fixture::new();
        fixture.git(&["init", "-q"]);
        fixture.write("src/lib.rs", "pub fn source() {}");
        fixture.write("notes.md", "suivi");
        fixture.git(&["add", "notes.md"]);
        let before = fixture.digest();
        fixture.write("notes.md", "changed");
        assert_ne!(before, fixture.digest(), "a tracked note was omitted");
        for name in ["src/new.rs", "README.md", "ideas.md", "notes.rs"] {
            let before = fixture.digest();
            fixture.write(name, "new");
            assert_ne!(before, fixture.digest(), "{name} was omitted");
        }
    }
    #[test]
    fn a_note_outside_git_cannot_be_told_from_a_source() {
        let fixture = Fixture::new();
        let before = fixture.digest();
        fixture.write("notes.md", "ce soir");
        assert_ne!(before, fixture.digest());
    }
    #[test]
    fn only_a_data_folder_git_ignores_is_local_state() {
        let fixture = Fixture::new();
        fixture.git(&["init", "-q"]);
        fixture.write(".gitignore", "/data\n");
        fixture.write("data/journal.jsonl", "{}");
        fixture.write("data/tracked.json", "{}");
        fixture.write("content/data/words.json", "[]");
        fixture.git(&["add", "-f", "data/tracked.json"]);
        let before = fixture.digest();
        fixture.write("data/journal.jsonl", "{\"more\":1}");
        fixture.write("data/saves/slot.bin", "state");
        assert_eq!(before, fixture.digest(), "the running game's state counted");
        assert!(
            source_files(&fixture.0)
                .unwrap()
                .excluded
                .contains(&"data/".to_owned())
        );
        fixture.write("data/tracked.json", "{\"more\":1}");
        assert_ne!(before, fixture.digest(), "a tracked file was omitted");
        let before = fixture.digest();
        fixture.write("content/data/words.json", "[1]");
        assert_ne!(before, fixture.digest(), "a data folder Git follows");
    }
    #[test]
    fn a_data_folder_git_cannot_judge_stays_an_input() {
        let fixture = Fixture::new();
        fixture.git(&["init", "-q"]);
        // `git check-ignore` refuses a path outside its work tree.
        assert!(!ignored_by_git(&fixture.0, Path::new("../data")).unwrap());
    }
    #[test]
    fn a_data_folder_outside_git_stays_an_input() {
        let fixture = Fixture::new();
        fixture.write("data/state.json", "{}");
        let before = fixture.digest();
        fixture.write("data/state.json", "changed");
        assert_ne!(before, fixture.digest());
    }
    #[test]
    fn a_changed_input_is_named_and_a_changed_environment_is_not_invented() {
        let fixture = Fixture::new();
        fixture.write("src/lib.rs", "pub fn before() {}");
        fixture.write("src/gone.rs", "pub fn gone() {}");
        let capture = |fixture: &Fixture| {
            let mut identity = Identity {
                value: String::new(),
                source_roots: vec![],
                files: 0,
                excluded: vec![],
                digests: BTreeMap::new(),
                assumptions: vec![],
            };
            let mut hash = Sha256::new();
            let sources = hash_sources(&mut hash, &BTreeSet::from([fixture.0.clone()])).unwrap();
            identity.digests = sources.digests;
            identity
        };
        let before = capture(&fixture);
        assert_eq!(changes(&before, &before, &fixture.0), "");
        fixture.write("src/lib.rs", "pub fn after() {}");
        fixture.write("src/added.rs", "pub fn added() {}");
        std::fs::remove_file(fixture.0.join("src/gone.rs")).unwrap();
        assert_eq!(
            changes(&before, &capture(&fixture), &fixture.0),
            " : src/added.rs, src/gone.rs, src/lib.rs"
        );
    }
    #[test]
    fn missing_tracked_sources_fail_closed() {
        let fixture = Fixture::new();
        fixture.git(&["init", "-q"]);
        fixture.write("src/lib.rs", "pub fn tracked() {}");
        fixture.git(&["add", "src/lib.rs"]);
        std::fs::remove_file(fixture.0.join("src/lib.rs")).unwrap();
        assert!(source_files(&fixture.0).is_err());
        std::fs::create_dir(fixture.0.join("src/lib.rs")).unwrap();
        assert!(source_files(&fixture.0).is_err());
    }
    #[test]
    fn artifact_files_are_not_confused_with_artifact_directories() {
        let fixture = Fixture::new();
        fixture.write("target", "a source file named target");
        let before = fixture.digest();
        fixture.write("target", "changed");
        assert_ne!(before, fixture.digest());
        std::fs::remove_file(fixture.0.join("target")).unwrap();
        fixture.write("target/build", "first");
        let before = fixture.digest();
        fixture.write("target/build", "second");
        assert_eq!(before, fixture.digest());
    }
    #[cfg(unix)]
    #[test]
    fn source_symlinks_including_artifact_names_are_rejected() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        fixture.write("outside", "content");
        symlink(fixture.0.join("outside"), fixture.0.join("target")).unwrap();
        assert!(source_files(&fixture.0).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn wrappers_bind_both_path_and_actual_contents() {
        use std::os::unix::fs::PermissionsExt as _;
        let fixture = Fixture::new();
        let mut environment = Environment::new();
        let mut digests = Vec::new();
        for (name, body) in [
            ("wrapper-a", "#!/bin/sh\nexec \"$@\"\n"),
            ("wrapper-b", "#!/bin/sh\nexec \"$@\"\n"),
            ("wrapper-b", "#!/bin/sh\nexit 0\n"),
        ] {
            fixture.write(name, body);
            let path = fixture.0.join(name);
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            environment.insert("RUSTC_WRAPPER".into(), path.into_os_string());
            let mut hash = Sha256::new();
            hash_environment(&mut hash, &fixture.0, &environment).unwrap();
            digests.push(hex(&hash.finalize()));
        }
        assert!(digests.windows(2).all(|pair| pair.first() != pair.get(1)));
    }
    #[test]
    fn a_check_records_a_missing_node_and_a_harness_refuses_it() {
        let fixture = Fixture::new();
        std::fs::create_dir(fixture.0.join("empty")).unwrap();
        let digest = |environment: &Environment, node: Node| {
            let mut hash = Sha256::new();
            hash_node(&mut hash, &fixture.0, environment, node).map(|()| hex(&hash.finalize()))
        };
        let without =
            Environment::from([("PATH".into(), fixture.0.join("empty").into_os_string())]);
        let absent = digest(&without, Node::Optional).unwrap();
        assert!(
            digest(&without, Node::Required)
                .unwrap_err()
                .contains("node")
        );
        let with = Environment::from([("PATH".into(), std::env::var_os("PATH").unwrap())]);
        let present = digest(&with, Node::Optional).unwrap();
        assert_ne!(absent, present, "a missing Node must show in the identity");
        assert_eq!(
            present,
            digest(&with, Node::Required).unwrap(),
            "a Node that is there is hashed the same with or without a harness"
        );
    }
    #[test]
    fn windows_finds_a_program_by_its_exe() {
        let fixture = Fixture::new();
        fixture.write("bin/node.exe", "");
        fixture.write("bin/cargo", "");
        #[cfg(unix)]
        for name in ["bin/node.exe", "bin/cargo"] {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(fixture.0.join(name), mode).unwrap();
        }
        let environment =
            Environment::from([("PATH".into(), fixture.0.join("bin").into_os_string())]);
        let find = |program: &str, spelling: Spelling| {
            find_on_path(&fixture.0, OsStr::new(program), &environment, spelling).unwrap()
        };
        assert_eq!(
            find("node", Spelling::Windows),
            Some(fixture.0.join("bin/node.exe"))
        );
        assert_eq!(
            find("node.exe", Spelling::Windows),
            Some(fixture.0.join("bin/node.exe"))
        );
        assert_eq!(find("node", Spelling::Posix), None);
        assert_eq!(
            find("cargo", Spelling::Posix),
            Some(fixture.0.join("bin/cargo"))
        );
        assert_eq!(
            find("cargo", Spelling::Windows),
            None,
            "Command would not start it"
        );
    }
    #[test]
    fn windows_variable_names_are_folded_and_its_home_is_kept() {
        let variables = || {
            [("Path", "bin"), ("UserProfile", "profile"), ("Other", "x")]
                .into_iter()
                .map(|(key, value)| (OsString::from(key), OsString::from(value)))
        };
        let windows = kept_variables(variables(), Spelling::Windows);
        assert_eq!(windows.get(OsStr::new("PATH")), Some(&"bin".into()));
        assert_eq!(
            windows.get(OsStr::new("USERPROFILE")),
            Some(&"profile".into())
        );
        assert_eq!(windows.len(), 2);
        assert!(kept_variables(variables(), Spelling::Posix).is_empty());
    }
    #[test]
    fn the_windows_home_holds_the_global_cargo_config() {
        let fixture = Fixture::new();
        let environment =
            Environment::from([("USERPROFILE".into(), fixture.0.clone().into_os_string())]);
        let digest = || {
            let mut hash = Sha256::new();
            hash_configs(&mut hash, &fixture.0, &environment).unwrap();
            hex(&hash.finalize())
        };
        let before = digest();
        fixture.write(".cargo/config.toml", "[net]\noffline=true\n");
        assert_ne!(before, digest());
    }
    #[test]
    fn ancestor_and_global_cargo_configs_are_inputs() {
        let fixture = Fixture::new();
        let dir = fixture.0.join("project");
        std::fs::create_dir_all(&dir).unwrap();
        let home = fixture.0.join("cargo-home");
        let environment = Environment::from([("CARGO_HOME".into(), home.into_os_string())]);
        let digest = || {
            let mut hash = Sha256::new();
            hash_configs(&mut hash, &dir, &environment).unwrap();
            hex(&hash.finalize())
        };
        let before = digest();
        fixture.write(".cargo/config.toml", "[build]\nrustflags=['--cfg','one']\n");
        assert_ne!(before, digest());
        let before = digest();
        fixture.write(
            "cargo-home/config.toml",
            "[build]\nrustflags=['--cfg','two']\n",
        );
        assert_ne!(before, digest());
    }
    #[test]
    fn patched_dependency_changes_invalidate_the_actual_capture() {
        let fixture = Fixture::new();
        fixture.write("app/Cargo.toml", "[package]\nname='identity-app'\nversion='0.1.0'\nedition='2024'\n[dependencies]\nidentity-dep='=0.1.0'\n[patch.crates-io]\nidentity-dep={path='../dependency'}\n");
        fixture.write("app/.cargo/config.toml", "[net]\noffline=true\n");
        fixture.write(
            "app/src/lib.rs",
            "pub fn value()->u8 {identity_dep::value()}",
        );
        fixture.write(
            "dependency/Cargo.toml",
            "[package]\nname='identity-dep'\nversion='0.1.0'\nedition='2024'\n",
        );
        fixture.write("dependency/src/lib.rs", "pub fn value()->u8 {1}");
        fixture.write("app/Cargo.lock", "version=4\n[[package]]\nname='identity-app'\nversion='0.1.0'\ndependencies=['identity-dep']\n[[package]]\nname='identity-dep'\nversion='0.1.0'\n");
        let dir = fixture.0.join("app");
        let selected = BTreeSet::from(["test".to_owned()]);
        let before = capture(&dir, &plan(), &selected, None, &[]).unwrap();
        fixture.write("dependency/src/lib.rs", "pub fn value()->u8 {2}");
        let after = capture(&dir, &plan(), &selected, None, &[]).unwrap();
        assert_ne!(before.value, after.value);
        assert!(
            after
                .source_roots
                .contains(&fixture.0.join("dependency").display().to_string())
        );
        assert!(
            after
                .assumptions
                .iter()
                .any(|text| text.contains("n’est pas hermétique"))
        );
    }
    #[test]
    fn missing_inputs_and_missing_tools_cannot_yield_an_identity() {
        let fixture = Fixture::new();
        assert!(
            capture(
                &fixture.0.join("missing"),
                &plan(),
                &BTreeSet::new(),
                None,
                &[]
            )
            .is_err()
        );
        let mut hash = Sha256::new();
        assert!(
            hash_executable(
                &mut hash,
                &fixture.0,
                "RUSTC",
                fixture.0.join("missing-tool").as_os_str(),
                &Environment::new()
            )
            .is_err()
        );
        assert!(
            validate_campaign(&Environment::from([(
                "SOIREE_SIM_SEEDS".into(),
                "0".into()
            )]))
            .is_err()
        );
        assert!(
            validate_campaign(&Environment::from([("SIDEQUEST_BLESS".into(), "1".into())]))
                .is_err()
        );
    }
}
