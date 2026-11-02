//! Research made measurable: `recherche/` in the common repository, the
//! commands `soiree recherche …`, and what `soiree check` says about it.
//!
//! The code stays free: nothing here blocks a game, nor a tight loop. `soiree
//! check .` judges the sheets of the common repository, and the debt only ever
//! stops a tag, at the step written in `AGENTS.md`.

mod controle;
mod dette;
pub mod fiche;
mod plan;
mod verifie;

use std::path::Path;
use std::process::ExitCode;

use crate::check::Options;
use crate::embedded;
use crate::report::Report;
use crate::util::{self, Result};
use controle::Disk;
use fiche::{Fiche, Perimetre, Registre, Sources};

const FOLDER: &str = "recherche";
const REGISTRY: &str = "sous-systemes.toml";
const SOURCES: &str = "sources.toml";
const COMMANDS: &str = "plan, verifie ou dette";

/// One `recherche/<sous_systeme>.toml`, as read.
pub struct Sheet {
    /// As shown to the reader.
    pub file: String,
    pub stem: String,
    pub parsed: Result<Fiche>,
}

/// What a folder holds of research. The common repository has its own
/// registry; a game reads the one this binary carries.
pub struct Folder {
    pub registre: Registre,
    pub sheets: Vec<Sheet>,
    pub sources: Option<Result<Sources>>,
    pub perimetre: Perimetre,
}

impl Folder {
    pub fn load(dir: &Path) -> Result<Self> {
        let folder = dir.join(FOLDER);
        let own = folder.join(REGISTRY);
        let (registre, perimetre) = if own.is_file() {
            let text = util::read(&own)?;
            (
                fiche::read(&text, &format!("{FOLDER}/{REGISTRY}"))?,
                Perimetre::Commun,
            )
        } else {
            (
                fiche::read(embedded::recherche::SOUS_SYSTEMES, REGISTRY)?,
                Perimetre::Jeu,
            )
        };
        let sources = folder.join(SOURCES);
        let sources = sources.is_file().then(|| {
            util::read(&sources).and_then(|text| fiche::read(&text, &format!("{FOLDER}/{SOURCES}")))
        });
        let sheets = util::files_under(&folder, &[".toml"], &[])
            .into_iter()
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| name != REGISTRY && name != SOURCES)
            })
            .map(|path| {
                let file = util::shown(&path, dir);
                let stem = path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let parsed = util::read(&path).and_then(|text| fiche::read(&text, &file));
                Sheet { file, stem, parsed }
            })
            .collect();
        Ok(Self {
            registre,
            sheets,
            sources,
            perimetre,
        })
    }

    fn read_sheets(&self) -> Vec<&Fiche> {
        self.sheets
            .iter()
            .filter_map(|sheet| sheet.parsed.as_ref().ok())
            .collect()
    }
}

/// `soiree recherche <…>`.
pub fn run(args: &[String]) -> Result<ExitCode> {
    let Some((command, rest)) = args.split_first() else {
        return Err(format!("recherche : attendu {COMMANDS}"));
    };
    match command.as_str() {
        "plan" => plan::run(rest),
        "verifie" => verifie::run(&crate::one_dir(rest)?),
        "dette" => dette::run(&crate::one_dir(rest)?),
        other => Err(format!("recherche : {other} ; attendu {COMMANDS}")),
    }
}

/// The offline control of the common repository, from `soiree check .`. With
/// `--fast` the code is moving: one line of debt, and nothing that can fail.
pub fn check(dir: &Path, options: Options, report: &mut Report) {
    if options.fast {
        debt_line(dir, report);
        return;
    }
    if !dir.join(FOLDER).join(REGISTRY).is_file() {
        report.error(format!("{FOLDER}/{REGISTRY} absent"));
        return;
    }
    let folder = match Folder::load(dir) {
        Ok(folder) => folder,
        Err(error) => {
            report.error(error);
            return;
        }
    };
    let before = report.lines.len();
    errors_of(
        &format!("{FOLDER}/{REGISTRY}"),
        controle::registry_rules(&folder.registre),
        report,
    );
    let disk = Disk::new(dir);
    for sheet in &folder.sheets {
        match &sheet.parsed {
            Ok(fiche) => errors_of(
                &sheet.file,
                controle::rules(fiche, &sheet.stem, &folder.registre, &disk),
                report,
            ),
            Err(error) => report.error(error.clone()),
        }
    }
    match &folder.sources {
        Some(Ok(sources)) => errors_of(
            &format!("{FOLDER}/{SOURCES}"),
            controle::sources_rules(sources, &folder.registre, &folder.read_sheets()),
            report,
        ),
        Some(Err(error)) => report.error(error.clone()),
        None => report.error(format!("{FOLDER}/{SOURCES} absent")),
    }
    if report.lines.len() == before {
        let sheets = folder.read_sheets();
        let lessons: usize = sheets.iter().map(|fiche| fiche.lecons.len()).sum();
        report.ok(format!(
            "recherche : {} fiche(s), {lessons} leçon(s) : format, règles, quotas, saturation et garanties conformes",
            sheets.len()
        ));
    }
    report.info(dette::Debt::of(&folder).line());
}

fn errors_of(file: &str, found: Vec<String>, report: &mut Report) {
    for error in found {
        report.error(format!("{file} : {error}"));
    }
}

/// At most one line, and never a failure: what a game or a tight loop sees.
pub fn debt_line(dir: &Path, report: &mut Report) {
    if !dir.join(FOLDER).is_dir() {
        return;
    }
    match Folder::load(dir) {
        Ok(folder) => report.info(dette::Debt::of(&folder).line()),
        Err(error) => report.info(format!("dette de recherche non lue : {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Level;

    /// The real research folder, without the code its sheets cite.
    fn fixture() -> std::path::PathBuf {
        let dir = crate::testdir::fresh("recherche");
        let real = Path::new(env!("CARGO_MANIFEST_DIR")).join("../recherche");
        for file in ["reconnexion.toml", REGISTRY, SOURCES] {
            let text = util::read(&real.join(file)).unwrap();
            util::write(&dir.join(FOLDER).join(file), &text).unwrap();
        }
        dir
    }

    #[test]
    fn the_full_check_fails_where_the_fast_one_only_informs() {
        let dir = fixture();
        let mut full = Report::quiet();
        check(&dir, Options::default(), &mut full);
        assert!(full.count(Level::Error) > 0, "{:?}", full.lines);
        let fast = Options {
            fast: true,
            ..Options::default()
        };
        let mut quick = Report::quiet();
        check(&dir, fast, &mut quick);
        assert_eq!(quick.count(Level::Error), 0, "{:?}", quick.lines);
        assert_eq!(quick.lines.len(), 1, "{:?}", quick.lines);
        assert_eq!(quick.count(Level::Info), 1);
    }

    #[test]
    fn the_common_repository_cannot_lose_its_registry_in_silence() {
        let dir = crate::testdir::fresh("recherche-absente");
        let mut full = Report::quiet();
        check(&dir, Options::default(), &mut full);
        assert_eq!(full.count(Level::Error), 1, "{:?}", full.lines);
        let mut game = Report::quiet();
        debt_line(&dir, &mut game);
        assert!(game.lines.is_empty(), "{:?}", game.lines);
    }

    #[test]
    fn the_carried_registry_and_steps_are_readable() {
        let dir = crate::testdir::fresh("recherche-jeu");
        let folder = Folder::load(&dir).unwrap();
        assert_eq!(folder.perimetre, Perimetre::Jeu);
        assert!(folder.sheets.is_empty());
        assert!(embedded::recherche::PROCESSUS.contains("## Le processus"));
    }
}
