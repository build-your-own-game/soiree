//! Embeds the shared catalogue of items by mechanic (`../certifications/`),
//! whatever files it holds, so that `soiree check` can write it into a
//! game's `.soiree/certifications/`.
//!
//! The generated list names each file relative to `CARGO_MANIFEST_DIR`: no
//! absolute path of the machine that built it, nothing outside this repository.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::PathBuf;

fn main() -> io::Result<()> {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let folder = manifest.join("../certifications");
    println!("cargo:rerun-if-changed={}", folder.display());
    let mut names: Vec<String> = fs::read_dir(&folder)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|e| e == "md"))
                .filter_map(|path| path.file_name().map(|n| n.to_string_lossy().into_owned()))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    let mut code = String::from(
        "/// The shared catalogue: file name and contents.\npub const CATALOGUE: &[(&str, &str)] = &[\n",
    );
    for name in names {
        writeln!(
            code,
            "    ({name:?}, include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../certifications/\", {name:?}))),"
        )
        .map_err(io::Error::other)?;
    }
    code.push_str("];\n");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap_or_default());
    fs::write(out.join("catalogue.rs"), code)
}
