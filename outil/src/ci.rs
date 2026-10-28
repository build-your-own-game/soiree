//! The versioned workflow is part of a game's inherited contract.
use crate::embedded::{self, rules};
use crate::report::Report;
use crate::util::{self, Result};
use std::path::Path;

pub const PATH: &str = ".github/workflows/soiree.yml";

pub fn workflow() -> Result<String> {
    Ok(rules::CI
        .replace("{{version}}", embedded::VERSION)
        .replace("{{channel}}", &embedded::channel()?))
}

pub fn write(dir: &Path) -> Result<()> {
    util::write(&dir.join(PATH), &workflow()?)
}

pub fn check(dir: &Path, report: &mut Report) {
    match (util::read(&dir.join(PATH)), workflow()) {
        (Ok(actual), Ok(expected)) if actual == expected => report.ok("CI commune épinglée à la version du jeu"),
        _ => report.error("CI commune absente ou périmée : soiree upgrade réinstalle .github/workflows/soiree.yml"),
    }
}
