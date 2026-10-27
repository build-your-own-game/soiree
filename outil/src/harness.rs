//! The certification harness, as `soiree certify` runs it: the embedded files
//! of this version, extracted into a cache per version, with their Node
//! dependency installed once by `npm ci`. `--harness <dossier>` or
//! `SOIREE_HARNESS` points to a working copy instead, for development.

use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::embedded::{HARNESS, VERSION};
use crate::util::{self, Result};

/// Written once `npm ci` has succeeded for this lock file.
const STAMP: &str = ".npm-ci";

/// Where the harness runs: the folder given, `SOIREE_HARNESS`, or the cache
/// of this version, extracted and installed if needed.
pub fn locate(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(dir) = explicit {
        return Ok(util::absolute(dir));
    }
    if let Some(dir) = std::env::var_os("SOIREE_HARNESS") {
        return Ok(util::absolute(Path::new(&dir)));
    }
    let dir = cache_dir().ok_or("pas de dossier de cache (ni HOME, ni LOCALAPPDATA)")?;
    if extract(&dir)? > 0 {
        println!("harnais {VERSION} écrit dans {}", dir.display());
    }
    if !installed(&dir) {
        npm_ci(&dir)?;
    }
    Ok(dir)
}

/// `<cache>/soiree/<version>/cert`: `~/Library/Caches` on macOS,
/// `%LOCALAPPDATA%` on Windows, `$XDG_CACHE_HOME` or `~/.cache` elsewhere.
/// `SOIREE_CACHE` replaces `<cache>/soiree`.
pub fn cache_dir() -> Option<PathBuf> {
    let root = if let Some(root) = std::env::var_os("SOIREE_CACHE") {
        PathBuf::from(root)
    } else if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME")?).join("Library/Caches/soiree")
    } else if cfg!(windows) {
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("soiree")
    } else if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        PathBuf::from(xdg).join("soiree")
    } else {
        PathBuf::from(std::env::var_os("HOME")?).join(".cache/soiree")
    };
    Some(root.join(VERSION).join("cert"))
}

/// Writes the embedded files that are missing or differ; returns how many.
pub fn extract(dir: &Path) -> Result<usize> {
    let mut written = 0;
    for (name, contents) in HARNESS {
        let path = dir.join(name);
        if std::fs::read_to_string(&path).ok().as_deref() != Some(*contents) {
            util::write(&path, contents)?;
            written += 1;
        }
    }
    Ok(written)
}

/// `npm ci` ran for this very lock file.
pub fn installed(dir: &Path) -> bool {
    dir.join("node_modules").is_dir()
        && std::fs::read_to_string(dir.join(STAMP)).ok().as_deref() == Some(lock_hash().as_str())
}

fn lock_hash() -> String {
    let lock = HARNESS
        .iter()
        .find(|(name, _)| *name == "package-lock.json")
        .map_or("", |(_, text)| *text);
    crate::certify::hex(&Sha256::digest(lock.as_bytes()))
}

fn npm_ci(dir: &Path) -> Result<()> {
    println!("npm ci dans {} (une fois par version)…", dir.display());
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let status = Command::new(npm)
        .args(["ci", "--no-audit", "--no-fund"])
        .current_dir(dir)
        .status()
        .map_err(|error| format!("npm : {error} (installe Node : soiree doctor)"))?;
    if !status.success() {
        return Err(format!("npm ci a échoué dans {}", dir.display()));
    }
    util::write(&dir.join(STAMP), &lock_hash())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extraction_writes_once_and_repairs() {
        let dir = crate::testdir::fresh("harness").join("cert");
        assert_eq!(extract(&dir).unwrap(), HARNESS.len());
        assert_eq!(extract(&dir).unwrap(), 0);
        util::write(&dir.join("cert.mjs"), "// abîmé\n").unwrap();
        assert_eq!(extract(&dir).unwrap(), 1);
        assert!(!installed(&dir));
        std::fs::create_dir_all(dir.join("node_modules")).unwrap();
        util::write(&dir.join(STAMP), &lock_hash()).unwrap();
        assert!(installed(&dir));
        util::write(&dir.join(STAMP), "une autre version").unwrap();
        assert!(!installed(&dir));
    }

    #[test]
    fn the_cache_is_per_version() {
        let dir = cache_dir().unwrap();
        assert!(
            dir.ends_with(Path::new(VERSION).join("cert")),
            "{}",
            dir.display()
        );
    }
}
