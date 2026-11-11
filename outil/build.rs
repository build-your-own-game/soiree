//! Embarque le jeu de départ (`modele/`) dans l'outil, pour `soiree new`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

/// Ce qu'un jeu neuf ne reçoit pas : ce qui se fabrique en compilant.
const SKIPPED: [&str; 3] = ["target", "Cargo.lock", ".DS_Store"];

fn main() -> io::Result<()> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").map_err(io::Error::other)?);
    let out = PathBuf::from(env::var("OUT_DIR").map_err(io::Error::other)?);
    let root = manifest.join("../modele");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    collect(&root, &root, &mut files)?;
    files.sort();
    let mut code = String::from("pub const FILES: &[(&str, &[u8])] = &[\n");
    for (relative, absolute) in files {
        writeln!(code, "    ({relative:?}, include_bytes!({absolute:?})),")
            .map_err(io::Error::other)?;
    }
    code.push_str("];\n");
    fs::write(out.join("modele.rs"), code)
}

fn collect(root: &Path, dir: &Path, files: &mut Vec<(String, String)>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        if SKIPPED.iter().any(|skipped| name == *skipped) {
            continue;
        }
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect(root, &path, files)?;
        } else {
            let relative = path.strip_prefix(root).map_err(io::Error::other)?;
            let absolute = path.canonicalize()?;
            files.push((
                relative.to_string_lossy().replace('\\', "/"),
                absolute.to_string_lossy().into_owned(),
            ));
        }
    }
    Ok(())
}
