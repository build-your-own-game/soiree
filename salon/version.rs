//! The game's version for salon, computed from its sources: copy this file
//! as the `build.rs` of the crate that holds `Up` and `Down` (`crates/core`),
//! then write in that crate:
//!
//! ```ignore
//! /// The game's version: salon refuses a page built from other sources.
//! pub const VERSION: u32 = include!(concat!(env!("OUT_DIR"), "/version.rs"));
//! ```
//!
//! The host and its pages both compile that crate, so they get the same
//! number exactly when they were built from the same sources. A page left in
//! a browser's cache after the game changed is refused with a typed
//! `Refusal`, instead of decoding messages it no longer understands. No
//! number to remember to raise by hand.
//!
//! Every file under `src/` counts, comments and tests included: a change that
//! does not touch the messages refuses old pages too, which costs a reload.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-changed=src");
    let mut files = Vec::new();
    collect(Path::new("src"), &mut files)?;
    files.sort();
    // FNV-1a, 32 bits: stable across Rust versions, unlike `DefaultHasher`.
    let mut hash: u32 = 0x811c_9dc5;
    for file in &files {
        let name = file.to_string_lossy().replace('\\', "/");
        for byte in name.bytes().chain([0]).chain(fs::read(file)?) {
            hash = (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193);
        }
    }
    let out = PathBuf::from(std::env::var("OUT_DIR")?).join("version.rs");
    let (high, low) = (hash >> 16, hash & 0xffff);
    fs::write(out, format!("0x{high:04x}_{low:04x}_u32\n"))?;
    Ok(())
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}
