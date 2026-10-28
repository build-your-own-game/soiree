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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Identity {
    pub value: String,
    pub source_roots: Vec<String>,
    pub files: usize,
    #[serde(default)]
    pub assumptions: Vec<String>,
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
    let count = hash_sources(&mut hash, &roots)?;
    hash_configs(&mut hash, &dir, &environment)?;
    hash_environment(&mut hash, &dir, &environment)?;
    hash_tools(&mut hash, &dir, &environment)?;
    Ok(Identity {
        value: hex(&hash.finalize()),
        source_roots: roots
            .iter()
            .map(|root| root.display().to_string())
            .collect(),
        files: count,
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
        "L’empreinte couvre tous les fichiers des racines du projet, des dépendances path/git et du contrôleur déclaré, y compris les fichiers ignorés par Git et data/dist. Les répertoires .git, target, node_modules, .soiree et certification sont exclus ; leurs contenus ne doivent pas servir d’entrées source non déclarées.",
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

fn hash_sources(hash: &mut Sha256, roots: &BTreeSet<PathBuf>) -> Result<usize> {
    let mut count = 0usize;
    for root in roots {
        add(hash, "root", path_text(root)?.as_bytes());
        for path in source_files(root)? {
            let name = path.strip_prefix(root).map_err(|e| e.to_string())?;
            add_file(hash, path_text(name)?, &path)?;
            count = count.saturating_add(1);
        }
    }
    Ok(count)
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

fn add_file(hash: &mut Sha256, name: &str, path: &Path) -> Result<()> {
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
    add_digest(hash, name, &digest.finalize());
    Ok(())
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

fn source_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    walk(root, &mut files)?;
    // A tracked file missing from the working tree is never silently omitted.
    if has_git_ancestor(root)? {
        let tracked = util::run(root, "git", &["ls-files", "-z", "--cached", "--", "."])?;
        if !tracked.status.success() {
            return Err(format!("inventaire Git impossible dans {}", root.display()));
        }
        let tracked = String::from_utf8(tracked.stdout).map_err(|e| e.to_string())?;
        for name in tracked.split('\0').filter(|name| !name.is_empty()) {
            let relative = Path::new(name);
            if relative.is_absolute()
                || relative
                    .components()
                    .any(|part| !matches!(part, Component::Normal(_)))
            {
                return Err("chemin Git hors racine source".to_owned());
            }
            if relative.parent().is_some_and(|parent| {
                parent.components().any(|part| {
                    ARTIFACT_DIRECTORIES
                        .iter()
                        .any(|skip| part.as_os_str() == *skip)
                })
            }) {
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
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
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

fn walk(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{} : {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let name = entry.file_name();
        if kind.is_dir() && ARTIFACT_DIRECTORIES.iter().any(|skip| name == *skip) {
            continue;
        }
        if kind.is_symlink() {
            return Err(symbolic_source(&entry.path()));
        }
        if kind.is_dir() {
            walk(&entry.path(), files)?;
        } else if kind.is_file() {
            files.push(entry.path());
        } else {
            return Err(format!(
                "entrée source spéciale non prise en charge : {}",
                entry.path().display()
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
                .get(OsStr::new("HOME"))
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
    std::env::vars_os()
        .filter(|(key, _)| {
            key.to_str().is_some_and(|key| {
                [
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
    for (program, args) in [
        ("rustc", vec!["-Vv"]),
        ("cargo", vec!["-V"]),
        ("node", vec!["--version"]),
    ] {
        let resolved = hash_executable(hash, dir, program, OsStr::new(program), environment)?;
        let output = util::run(dir, program, &args)?;
        if !output.status.success() {
            return Err(format!("identité de {program} inconnue"));
        }
        add(hash, program, &output.stdout);
        // rustup proxies share one executable; also identify the selected tool.
        if resolved.file_stem().is_some_and(|name| name == "rustup") && program != "node" {
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
    let search = environment
        .get(OsStr::new("PATH"))
        .ok_or("PATH absent : exécutables non identifiés")?;
    for root in std::env::split_paths(search) {
        let root = if root.is_absolute() {
            root
        } else {
            dir.join(root)
        };
        let candidate = root.join(path);
        if is_executable(&candidate)? {
            return std::fs::canonicalize(candidate).map_err(|e| e.to_string());
        }
    }
    Err(format!(
        "programme introuvable pour l’identité : {}",
        program.to_string_lossy()
    ))
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
    fn ignored_inputs_data_and_dist_are_real_inputs() {
        let fixture = Fixture::new();
        fixture.git(&["init", "-q"]);
        fixture.write(".gitignore", "/inputs/\n/data/\n/dist/\n");
        fixture.write(
            "src/lib.rs",
            "pub const INPUT: &str = include_str!(\"../inputs/value.txt\");",
        );
        fixture.write("inputs/value.txt", "before");
        fixture.write("data/state.json", "{}");
        fixture.write("dist/game.wasm", "wasm");
        for name in ["inputs/value.txt", "data/state.json", "dist/game.wasm"] {
            let before = fixture.digest();
            fixture.write(name, "changed");
            assert_ne!(before, fixture.digest(), "{name} was omitted");
        }
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
