//! `soiree host-setup [dossier]`: installs the host's working folder on a
//! machine. It writes `AGENTS.md` and `.cargo/config.toml` from the embedded
//! templates (never over a different file without `--force`), clones
//! `soiree/`, then the host's games listed in `soiree/catalogue.toml`. A folder
//! that already exists is never touched. `--dry-run` does nothing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::embedded::{HOST_FOLDER, ORG, SHARED_REPOSITORY};
use crate::report::Report;
use crate::util::{self, Result};

/// How many differing lines to show before « … ».
const SHOWN_LINES: usize = 6;

#[derive(Debug, Clone)]
pub struct Options {
    pub dir: PathBuf,
    pub force: bool,
    pub dry_run: bool,
    /// Every game of the catalogue, whoever its author.
    pub all: bool,
    /// The GitHub user whose games to clone.
    pub author: Option<String>,
    /// Where `soiree/` comes from.
    pub source: String,
}

impl Options {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            force: false,
            dry_run: false,
            all: false,
            author: None,
            source: SHARED_REPOSITORY.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Outcome {
    Written,
    Identical,
    Different,
    Cloned,
    AlreadyThere,
    /// Exists, but is not a git repository or has another upstream.
    Suspicious,
    Failed,
}

impl Outcome {
    fn label(self, options: &Options) -> &'static str {
        match (self, options.dry_run) {
            (Outcome::Written, false) => "écrit",
            (Outcome::Written, true) => "à écrire",
            (Outcome::Identical, _) => "identique",
            (Outcome::Different, _) => "différent, laissé tel quel",
            (Outcome::Cloned, false) => "cloné",
            (Outcome::Cloned, true) => "à cloner",
            (Outcome::AlreadyThere, _) => "déjà là",
            (Outcome::Suspicious, _) => "déjà là, à vérifier",
            (Outcome::Failed, _) => "échec",
        }
    }
}

pub type Summary = Vec<(Outcome, String)>;

pub fn run(options: &Options) -> ExitCode {
    let mut report = Report::new();
    let summary = setup(options, &mut report);
    let mut groups: BTreeMap<Outcome, Vec<&str>> = BTreeMap::new();
    for (outcome, what) in &summary {
        groups.entry(*outcome).or_default().push(what);
    }
    println!(
        "\n== résumé{}",
        if options.dry_run {
            " (essai : rien n'a été fait)"
        } else {
            ""
        }
    );
    for (outcome, whats) in groups {
        println!("  {} : {}", outcome.label(options), whats.join(", "));
    }
    println!("\nEnsuite : soiree status {}", options.dir.display());
    report.finish()
}

pub fn setup(options: &Options, report: &mut Report) -> Summary {
    let dir = util::absolute(&options.dir);
    let mut summary = Summary::new();
    report.section(&format!(
        "{}{}",
        dir.display(),
        if options.dry_run { " (essai)" } else { "" }
    ));
    if dir.join(".git").exists() {
        report.warn("ce dossier est lui-même un dépôt git : le dossier de l'hôte n'en est pas un, il en contient plusieurs");
    }
    for (path, template) in HOST_FOLDER {
        let outcome = write_template(
            &dir,
            path,
            &template.replace("{{org}}", ORG),
            options,
            report,
        );
        summary.push((outcome, (*path).to_owned()));
    }

    let shared = dir.join("soiree");
    let outcome = ensure_clone(&options.source, &shared, &dir, options, report);
    summary.push((outcome, "soiree".to_owned()));

    let catalogue = shared.join("catalogue.toml");
    if options.dry_run && outcome == Outcome::Cloned {
        report.info("les jeux seront lus dans soiree/catalogue.toml, une fois soiree/ cloné : relance --dry-run après");
        return summary;
    }
    let games = match read_catalogue(&catalogue) {
        Ok(games) => games,
        Err(error) => {
            report.warn(format!("pas de jeux à cloner : {error}"));
            return summary;
        }
    };
    let author = if options.all {
        None
    } else {
        options.author.clone().or_else(github_user)
    };
    if !options.all && author.is_none() {
        report.info(
            "aucun auteur (--author <pseudo>, ou gh connecté) : aucun jeu cloné ; --all pour tous",
        );
        return summary;
    }
    let chosen: Vec<&Game> = games
        .iter()
        .filter(|g| options.all || author.as_deref() == Some(g.author.as_str()))
        .collect();
    match (&author, chosen.is_empty()) {
        (Some(author), true) => {
            report.info(format!("aucun jeu de {author} dans soiree/catalogue.toml"));
        }
        (Some(author), false) => report.info(format!("jeux de {author} : {}", names(&chosen))),
        (None, _) => report.info(format!("tous les jeux du catalogue : {}", names(&chosen))),
    }
    for game in chosen {
        let Some(folder) = folder_name(&game.repo) else {
            report.error(format!(
                "{} : dépôt « {} » sans nom de dossier utilisable",
                game.name, game.repo
            ));
            summary.push((Outcome::Failed, game.name.clone()));
            continue;
        };
        let target = dir.join("games").join(&folder);
        let outcome = ensure_clone(&game.repo, &target, &dir, options, report);
        summary.push((outcome, format!("games/{folder}")));
    }
    summary
}

fn names(games: &[&Game]) -> String {
    games
        .iter()
        .map(|g| g.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Writes `contents` at `dir/path`, unless a different file is already there.
fn write_template(
    dir: &Path,
    path: &str,
    contents: &str,
    options: &Options,
    report: &mut Report,
) -> Outcome {
    let target = dir.join(path);
    if let Ok(existing) = std::fs::read_to_string(&target) {
        if existing == contents {
            report.ok(format!("{path} identique au modèle"));
            return Outcome::Identical;
        }
        if !options.force {
            report.warn(format!(
                "{path} diffère du modèle ; laissé tel quel (--force pour le remplacer) :"
            ));
            for line in differences(&existing, contents) {
                report.detail(&line);
            }
            return Outcome::Different;
        }
    } else if target.exists() {
        report.error(format!(
            "{path} existe mais n'est pas un fichier lisible : laissé tel quel"
        ));
        return Outcome::Failed;
    }
    let verb = if target.exists() {
        "remplacé (--force)"
    } else {
        "écrit"
    };
    if options.dry_run {
        report.ok(format!("{path} : serait {verb}"));
        return Outcome::Written;
    }
    match util::write(&target, contents) {
        Ok(()) => {
            report.ok(format!("{path} {verb}"));
            Outcome::Written
        }
        Err(error) => {
            report.error(error);
            Outcome::Failed
        }
    }
}

/// The lines only in the file on disk (`-`) or only in the template (`+`).
fn differences(existing: &str, template: &str) -> Vec<String> {
    let only = |a: &str, b: &str, mark: char| -> Vec<String> {
        a.lines()
            .filter(|line| !b.lines().any(|other| other == *line))
            .map(|line| format!("{mark} {line}"))
            .collect()
    };
    let mut lines = only(existing, template, '-');
    lines.extend(only(template, existing, '+'));
    if lines.is_empty() {
        lines.push("(mêmes lignes, dans un autre ordre ou avec d'autres lignes vides)".to_owned());
    }
    if let Some(more) = lines.len().checked_sub(SHOWN_LINES)
        && more > 0
    {
        lines.truncate(SHOWN_LINES);
        lines.push(format!("… et {more} autre(s) ligne(s)"));
    }
    lines
}

/// Clones `url` into `target` if it is missing; otherwise only looks at it.
fn ensure_clone(
    url: &str,
    target: &Path,
    base: &Path,
    options: &Options,
    report: &mut Report,
) -> Outcome {
    let shown = util::shown(target, base);
    if target.exists() {
        if !target.join(".git").exists() {
            report.warn(format!(
                "{shown} existe mais n'est pas un dépôt git : laissé tel quel"
            ));
            return Outcome::Suspicious;
        }
        let origin = util::stdout_of(target, "git", &["remote", "get-url", "origin"]);
        return match origin {
            Some(origin) if same_remote(&origin, url) => {
                report.ok(format!("{shown} déjà là, amont {origin}"));
                Outcome::AlreadyThere
            }
            Some(origin) => {
                report.warn(format!(
                    "{shown} déjà là, mais son amont est {origin}, pas {url} : laissé tel quel"
                ));
                Outcome::Suspicious
            }
            None => {
                report.warn(format!(
                    "{shown} déjà là, sans amont origin (attendu {url}) : laissé tel quel"
                ));
                Outcome::Suspicious
            }
        };
    }
    if url.starts_with('-') {
        report.error(format!("{shown} : adresse refusée « {url} »"));
        return Outcome::Failed;
    }
    if options.dry_run {
        report.ok(format!("{shown} : serait cloné depuis {url}"));
        return Outcome::Cloned;
    }
    if let Some(parent) = target.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        report.error(format!("{} : {error}", parent.display()));
        return Outcome::Failed;
    }
    let target_text = target.display().to_string();
    match util::run(base, "git", &["clone", "--quiet", "--", url, &target_text]) {
        Ok(output) if output.status.success() => {
            report.ok(format!("{shown} cloné depuis {url}"));
            Outcome::Cloned
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            report.error(format!(
                "{shown} : git clone {url} a échoué : {}",
                stderr.lines().last().unwrap_or("").trim()
            ));
            Outcome::Failed
        }
        Err(error) => {
            report.error(error);
            Outcome::Failed
        }
    }
}

/// Two addresses of the same repository: without a trailing `/` or `.git`.
pub fn same_remote(a: &str, b: &str) -> bool {
    let normal = |url: &str| {
        let url = url.trim().trim_end_matches('/');
        url.strip_suffix(".git").unwrap_or(url).to_owned()
    };
    normal(a) == normal(b)
}

/// `https://github.com/org/tier-list.git` → `tier-list`. `None` for a name
/// that could leave `games/`.
pub fn folder_name(repo: &str) -> Option<String> {
    let last = repo.trim_end_matches('/').rsplit(['/', ':']).next()?;
    let last = last.strip_suffix(".git").unwrap_or(last);
    let fine = !last.is_empty()
        && !last.starts_with('.')
        && last
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    fine.then(|| last.to_owned())
}

#[derive(Debug)]
pub struct Game {
    pub name: String,
    pub repo: String,
    pub author: String,
}

pub fn read_catalogue(path: &Path) -> Result<Vec<Game>> {
    let table = util::read(path).and_then(|t| util::parse_toml(&t, "catalogue.toml"))?;
    let entries = table
        .get("game")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{} : aucun [[game]]", path.display()))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let field = |key: &str| {
                entry
                    .get(key)
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        format!(
                            "{} : le jeu n° {} n'a pas de {key}",
                            path.display(),
                            index.saturating_add(1)
                        )
                    })
            };
            Ok(Game {
                name: field("name")?,
                repo: field("repo")?,
                author: field("author")?,
            })
        })
        .collect()
}

/// The GitHub user `gh` is signed in as, if `gh` is there.
fn github_user() -> Option<String> {
    util::stdout_of(Path::new("."), "gh", &["api", "user", "--jq", ".login"])
        .filter(|login| !login.is_empty())
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;
    use crate::report::Level;

    fn count(report: &Report, level: Level) -> usize {
        report.count(level)
    }

    fn git(dir: &Path, args: &[&str]) {
        let base = [
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "-c",
            "commit.gpgsign=false",
        ];
        let output = util::run(dir, "git", &[&base[..], args].concat()).unwrap();
        assert!(
            output.status.success(),
            "git {args:?} : {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// A local repository with one commit, standing for a GitHub one.
    fn upstream(dir: &Path, files: &[(&str, &str)]) -> String {
        std::fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        for (path, text) in files {
            util::write(&dir.join(path), text).unwrap();
        }
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-qm", "init"]);
        dir.display().to_string()
    }

    /// Upstreams for `soiree` and three games, and an empty host folder.
    fn fixture() -> (PathBuf, Options) {
        let root = crate::testdir::fresh("host-setup");
        let up = root.join("upstream");
        let mut catalogue = String::new();
        for (name, author) in [("alpha", "alice"), ("beta", "bob"), ("gamma", "alice")] {
            let repo = upstream(&up.join(format!("{name}.git")), &[("README.md", name)]);
            writeln!(
                catalogue,
                "[[game]]\nname = \"{name}\"\nrepo = \"{repo}\"\nauthor = \"{author}\"\n\n"
            )
            .unwrap();
        }
        let source = upstream(&up.join("soiree"), &[("catalogue.toml", &catalogue)]);
        let host = root.join("host");
        std::fs::create_dir_all(&host).unwrap();
        let mut options = Options::new(host.clone());
        options.source = source;
        options.author = Some("alice".to_owned());
        (host, options)
    }

    fn outcomes(summary: &Summary) -> Vec<(Outcome, &str)> {
        summary.iter().map(|(o, w)| (*o, w.as_str())).collect()
    }

    #[test]
    fn a_fresh_folder_gets_its_files_and_its_author_s_games() {
        let (host, options) = fixture();
        let mut report = Report::quiet();
        let summary = setup(&options, &mut report);
        assert_eq!(
            outcomes(&summary),
            vec![
                (Outcome::Written, "AGENTS.md"),
                (Outcome::Written, ".cargo/config.toml"),
                (Outcome::Cloned, "soiree"),
                (Outcome::Cloned, "games/alpha"),
                (Outcome::Cloned, "games/gamma"),
            ]
        );
        assert!(!host.join("games/beta").exists());
        let config = util::read(&host.join(".cargo/config.toml")).unwrap();
        assert!(config.contains("[patch.\"https://github.com/build-your-own-game/soiree\"]"));
        assert!(!config.contains("{{"));

        // Again: nothing to do.
        let mut report = Report::quiet();
        let again = setup(&options, &mut report);
        assert!(
            again
                .iter()
                .all(|(o, _)| matches!(o, Outcome::Identical | Outcome::AlreadyThere)),
            "{again:?}"
        );
        assert_eq!(
            count(&report, Level::Error) + count(&report, Level::Warning),
            0
        );
    }

    #[test]
    fn a_different_file_is_kept_unless_forced() {
        let (host, mut options) = fixture();
        util::write(&host.join("AGENTS.md"), "# mon dossier\n").unwrap();
        let mut report = Report::quiet();
        let summary = setup(&options, &mut report);
        assert_eq!(summary.first().map(|(o, _)| *o), Some(Outcome::Different));
        assert_eq!(
            util::read(&host.join("AGENTS.md")).unwrap(),
            "# mon dossier\n"
        );
        assert!(
            differences("# mon dossier\n", "# autre\n").contains(&"- # mon dossier".to_owned())
        );

        options.force = true;
        let mut report = Report::quiet();
        let summary = setup(&options, &mut report);
        assert_eq!(summary.first().map(|(o, _)| *o), Some(Outcome::Written));
        assert!(
            util::read(&host.join("AGENTS.md"))
                .unwrap()
                .starts_with("# Le dossier de l'hôte")
        );
    }

    #[test]
    fn existing_folders_are_never_touched() {
        let (host, mut options) = fixture();
        options.all = true;
        util::write(&host.join("games/beta/notes.txt"), "à moi\n").unwrap();
        let other = upstream(&host.join("../ailleurs"), &[("x", "x")]);
        let gamma = host.join("games/gamma");
        std::fs::create_dir_all(host.join("games")).unwrap();
        git(&host.join("games"), &["clone", "-q", &other, "gamma"]);
        let mut report = Report::quiet();
        let summary = setup(&options, &mut report);
        assert_eq!(
            outcomes(&summary).get(3..).unwrap(),
            [
                (Outcome::Cloned, "games/alpha"),
                (Outcome::Suspicious, "games/beta"),
                (Outcome::Suspicious, "games/gamma"),
            ]
        );
        assert_eq!(
            util::read(&host.join("games/beta/notes.txt")).unwrap(),
            "à moi\n"
        );
        assert!(!host.join("games/beta/.git").exists());
        assert!(gamma.join("x").is_file());
        assert_eq!(count(&report, Level::Warning), 2);
    }

    #[test]
    fn a_dry_run_does_nothing() {
        let (host, mut options) = fixture();
        options.dry_run = true;
        let mut report = Report::quiet();
        let summary = setup(&options, &mut report);
        assert_eq!(
            outcomes(&summary),
            vec![
                (Outcome::Written, "AGENTS.md"),
                (Outcome::Written, ".cargo/config.toml"),
                (Outcome::Cloned, "soiree"),
            ]
        );
        assert_eq!(std::fs::read_dir(&host).unwrap().count(), 0);

        // With soiree/ already there, the dry run also lists the games.
        options.dry_run = false;
        options.author = Some("nobody".to_owned());
        setup(&options, &mut Report::quiet());
        options.dry_run = true;
        options.author = Some("bob".to_owned());
        let summary = setup(&options, &mut Report::quiet());
        assert_eq!(
            summary.last().map(|(o, w)| (*o, w.as_str())),
            Some((Outcome::Cloned, "games/beta"))
        );
        assert!(!host.join("games/beta").exists());
    }

    #[test]
    fn remotes_and_folder_names() {
        assert!(same_remote(
            "https://github.com/o/a.git",
            "https://github.com/o/a/"
        ));
        assert!(!same_remote(
            "https://github.com/o/a",
            "https://github.com/o/b"
        ));
        assert_eq!(
            folder_name("https://github.com/o/tier-list.git").as_deref(),
            Some("tier-list")
        );
        assert_eq!(
            folder_name("git@github.com:o/punchline").as_deref(),
            Some("punchline")
        );
        for bad in [
            "https://github.com/o/..",
            "https://x/.hidden",
            "",
            "https://x/a b",
        ] {
            assert_eq!(folder_name(bad), None, "{bad}");
        }
    }
}
