//! Préparer les pages : compiler le crate des pages pour le navigateur et
//! rassembler le dossier que le jeu sert.
//!
//! Le jeu appelle [`build`] à son démarrage. `cargo run --release` suffit
//! donc à tout lancer, sans outil à installer à côté.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

use serde::Deserialize;
use wasm_bindgen_cli_support::Bindgen;

const TARGET: &str = "wasm32-unknown-unknown";

#[derive(Deserialize)]
struct Metadata {
    workspace_root: PathBuf,
    target_directory: PathBuf,
}

/// Compile le crate `package` du workspace pour le navigateur et renvoie le
/// dossier à servir.
///
/// `manifest_dir` est le dossier du crate qui appelle :
/// `Path::new(env!("CARGO_MANIFEST_DIR"))`. Le dossier `web/`, à la racine du
/// workspace, est copié tel quel à côté du code compilé : il contient
/// `index.html`, les feuilles de style et les images. La page charge le code
/// avec :
///
/// ```html
/// <script type="module">
///   import init from "/pages.js";
///   init();
/// </script>
/// ```
///
/// où `pages` est le nom du crate, ses tirets remplacés par des `_`.
pub fn build(manifest_dir: &Path, package: &str) -> io::Result<PathBuf> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(&cargo)
        .current_dir(manifest_dir)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    let metadata: Metadata = serde_json::from_slice(&output.stdout)?;

    let status = Command::new(&cargo)
        .current_dir(&metadata.workspace_root)
        .args([
            "build",
            "--release",
            "--target",
            TARGET,
            "--package",
            package,
        ])
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "les pages ne compilent pas (crate {package})"
        )));
    }

    let module = package.replace('-', "_");
    let wasm = metadata
        .target_directory
        .join(TARGET)
        .join("release")
        .join(format!("{module}.wasm"));
    let pages = metadata.target_directory.join("pages");
    if pages.exists() {
        fs::remove_dir_all(&pages)?;
    }
    fs::create_dir_all(&pages)?;
    let mut bindgen = Bindgen::new();
    let _: &mut Bindgen = bindgen
        .input_path(&wasm)
        .web(true)
        .map_err(|error| io::Error::other(format!("{error:#}")))?
        .typescript(false)
        // Sans argument, `init()` charge le fichier compilé posé à côté.
        .omit_default_module_path(false);
    bindgen
        .generate(&pages)
        .map_err(|error| io::Error::other(format!("{error:#}")))?;
    let statics = metadata.workspace_root.join("web");
    if statics.is_dir() {
        copy_dir(&statics, &pages)?;
    }
    Ok(pages)
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            let _: u64 = fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
