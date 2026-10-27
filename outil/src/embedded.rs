//! Everything this binary carries, so that one version of `soiree` holds
//! exactly the rules, documents and model of that version.
//!
//! Everything comes from this repository, relative to `outil/`: the rules
//! (`regles/`, once the `toml` blocks of `MANIFESTE.md`), the host's folder
//! (`dossier/`), the documents (`../MANIFESTE.md`, `../salon/README.md`…), the
//! model (`../modele/`), the catalogue (`../certifications/`, through
//! `build.rs`) and the harness (`../tools/cert/`). Nothing outside it.

use std::path::Path;

use crate::util::{self, Result};

/// This binary's version, which is the shared repository's tag without its `v`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The GitHub organisation; `--org` overrides it where it matters.
pub const ORG: &str = "build-your-own-game";

/// The shared repository, cloned by `soiree host-setup`.
pub const SHARED_REPOSITORY: &str = "https://github.com/build-your-own-game/soiree";

pub mod rules {
    pub const TOOLCHAIN: &str = include_str!("../regles/rust-toolchain.toml");
    pub const CARGO: &str = include_str!("../regles/cargo.toml");
    pub const CLIPPY: &str = include_str!("../regles/clippy.toml");
}

/// The documents written to a game's `.soiree/`, at their path there.
pub const DOCS: &[(&str, &str)] = &[
    ("MANIFESTE.md", include_str!("../../MANIFESTE.md")),
    ("CERTIFICATION.md", include_str!("../../CERTIFICATION.md")),
    ("salon/README.md", include_str!("../../salon/README.md")),
    ("remote/CONTRAT.md", include_str!("../../remote/CONTRAT.md")),
    // The build script that gives a game its version (salon/README.md).
    ("salon/version.rs", include_str!("../../salon/version.rs")),
];

include!(concat!(env!("OUT_DIR"), "/catalogue.rs"));

/// The certification harness (`../tools/cert/`), extracted by `soiree
/// certify` into a cache per version, then `npm ci` installs its one
/// dependency there.
pub const HARNESS: &[(&str, &str)] = &[
    ("cert.mjs", include_str!("../../tools/cert/cert.mjs")),
    ("browser.mjs", include_str!("../../tools/cert/browser.mjs")),
    (
        "performance.mjs",
        include_str!("../../tools/cert/performance.mjs"),
    ),
    (
        "socket-observer.test.mjs",
        include_str!("../../tools/cert/socket-observer.test.mjs"),
    ),
    (
        "performance.test.mjs",
        include_str!("../../tools/cert/performance.test.mjs"),
    ),
    (
        "input.test.mjs",
        include_str!("../../tools/cert/input.test.mjs"),
    ),
    (
        "storage.test.mjs",
        include_str!("../../tools/cert/storage.test.mjs"),
    ),
    (
        "toxiproxy.mjs",
        include_str!("../../tools/cert/toxiproxy.mjs"),
    ),
    ("nasty.json", include_str!("../../tools/cert/nasty.json")),
    (
        "package.json",
        include_str!("../../tools/cert/package.json"),
    ),
    (
        "package-lock.json",
        include_str!("../../tools/cert/package-lock.json"),
    ),
];

/// The host's working folder, at its path there (`soiree host-setup`).
/// `{{org}}` is replaced when it is written. In the shared repository, these
/// templates live in `outil/dossier/`.
pub const HOST_FOLDER: &[(&str, &str)] = &[
    ("AGENTS.md", include_str!("../dossier/AGENTS.md")),
    (
        ".cargo/config.toml",
        include_str!("../dossier/.cargo/config.toml"),
    ),
];

/// The model a new game starts from, at its path in the game. `{{nom}}`,
/// `{{org}}` and `{{version}}` are replaced when it is written.
pub const MODEL: &[(&str, &str)] = &[
    ("AGENTS.md", include_str!("../../modele/AGENTS.md")),
    (
        "tools/press-check.mjs",
        include_str!("../../modele/tools/press-check.mjs"),
    ),
    (
        "crates/host/tests/replays/session.jsonl",
        include_str!("../../modele/crates/host/tests/replays/session.jsonl"),
    ),
    (
        "crates/host/src/tests.rs",
        include_str!("../../modele/crates/host/src/tests.rs"),
    ),
    ("README.md", include_str!("../../modele/README.md")),
    (
        "crates/web/src/page.rs",
        include_str!("../../modele/crates/web/src/page.rs"),
    ),
    (
        "crates/core/src/replay.rs",
        include_str!("../../modele/crates/core/src/replay.rs"),
    ),
    ("cert.json", include_str!("../../modele/cert.json")),
    (
        "crates/core/Cargo.toml",
        include_str!("../../modele/crates/core/Cargo.toml"),
    ),
    (
        "crates/core/build.rs",
        include_str!("../../modele/crates/core/build.rs"),
    ),
    (
        "crates/core/src/lib.rs",
        include_str!("../../modele/crates/core/src/lib.rs"),
    ),
    (
        "crates/core/src/protocol.rs",
        include_str!("../../modele/crates/core/src/protocol.rs"),
    ),
    (
        "crates/host/Cargo.toml",
        include_str!("../../modele/crates/host/Cargo.toml"),
    ),
    (
        "crates/host/src/main.rs",
        include_str!("../../modele/crates/host/src/main.rs"),
    ),
    (
        "crates/host/src/native.rs",
        include_str!("../../modele/crates/host/src/native.rs"),
    ),
    (
        "crates/web/Cargo.toml",
        include_str!("../../modele/crates/web/Cargo.toml"),
    ),
    (
        "crates/web/src/lib.rs",
        include_str!("../../modele/crates/web/src/lib.rs"),
    ),
    (
        "web/index.html",
        include_str!("../../modele/web/index.html"),
    ),
    ("web/style.css", include_str!("../../modele/web/style.css")),
    ("soiree.toml", include_str!("../../modele/soiree.toml")),
    ("Cargo.toml", include_str!("../../modele/Cargo.toml")),
    (
        "CERTIFICATION.md",
        include_str!("../../modele/CERTIFICATION.md"),
    ),
    ("LEARNINGS.md", include_str!("../../modele/LEARNINGS.md")),
    (".gitignore", include_str!("../../modele/.gitignore")),
    (
        "certification/README.md",
        include_str!("../../modele/certification/README.md"),
    ),
    ("rust-toolchain.toml", rules::TOOLCHAIN),
    ("clippy.toml", rules::CLIPPY),
];

/// The toolchain channel the rules require, like `1.99`.
pub fn channel() -> Result<String> {
    let table = util::parse_toml(rules::TOOLCHAIN, "regles/rust-toolchain.toml")?;
    table
        .get("toolchain")
        .and_then(|t| t.get("channel"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "regles/rust-toolchain.toml : pas de toolchain.channel".to_owned())
}

/// The command that installs this tool at `version`.
pub fn install_command(version: &str) -> String {
    format!("cargo install --locked --git https://github.com/{ORG}/soiree --tag v{version} soiree")
}

/// Writes the documents into `game/.soiree/`, replacing what was there.
pub fn write_docs(game: &Path) -> Result<()> {
    let target = game.join(".soiree");
    if target.exists() {
        std::fs::remove_dir_all(&target)
            .map_err(|error| format!("{} : {error}", target.display()))?;
    }
    for (path, contents) in DOCS {
        util::write(&target.join(path), contents)?;
    }
    for (name, contents) in CATALOGUE {
        util::write(&target.join("certifications").join(name), contents)?;
    }
    util::write(&target.join("version"), &format!("{VERSION}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_parse_and_name_a_channel() {
        assert_eq!(channel().unwrap(), "1.99");
        util::parse_toml(rules::CARGO, "cargo").unwrap();
        util::parse_toml(rules::CLIPPY, "clippy").unwrap();
    }

    #[test]
    fn model_toml_files_parse_once_filled() {
        for (path, contents) in MODEL {
            if Path::new(path).extension().is_some_and(|e| e == "toml") {
                let filled = contents
                    .replace("{{nom}}", "essai")
                    .replace("{{org}}", "org")
                    .replace("{{version}}", VERSION);
                util::parse_toml(&filled, path).unwrap();
            }
        }
    }
}
