//! `soiree status`: in a folder of several repositories (the host's), what is
//! not committed, not pushed, or has no upstream. It never fetches: « pushed »
//! means as of the last `git fetch`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::report::{Level, Report};
use crate::util;

/// How many changed files to name before « … ».
const SHOWN: usize = 6;

pub fn run(dir: &Path) -> ExitCode {
    let dir = util::absolute(dir);
    let repositories = repositories(&dir);
    let mut report = Report::new();
    if repositories.is_empty() {
        report.section(&dir.display().to_string());
        report.error("aucun dépôt git ici, ni dans ses sous-dossiers, ni sous games/");
        return report.finish();
    }
    for repository in &repositories {
        repository_status(&dir, repository, &mut report);
    }
    let pending = report
        .count(Level::Error)
        .saturating_add(report.count(Level::Warning));
    println!(
        "\n{} dépôt(s), {pending} point(s) en attente",
        repositories.len()
    );
    if pending == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// `dir` itself, its direct subfolders and those of `games/`, when they are
/// git repositories.
pub fn repositories(dir: &Path) -> Vec<PathBuf> {
    let is_repo = |p: &Path| p.join(".git").exists();
    let children = |p: &Path| -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = std::fs::read_dir(p)
            .map(|e| {
                e.filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| p.is_dir() && is_repo(p))
                    .collect()
            })
            .unwrap_or_default();
        found.sort();
        found
    };
    let mut found = Vec::new();
    if is_repo(dir) {
        found.push(dir.to_path_buf());
    }
    found.extend(children(dir));
    found.extend(children(&dir.join("games")));
    found
}

fn repository_status(base: &Path, repository: &Path, report: &mut Report) {
    let name = if repository == base {
        ".".to_owned()
    } else {
        util::shown(repository, base)
    };
    let Some(status) =
        util::stdout_of(repository, "git", &["status", "--porcelain=v1", "--branch"])
    else {
        report.section(&name);
        report.error("git status a échoué");
        return;
    };
    let mut lines = status.lines();
    let branch = lines
        .next()
        .and_then(|l| l.strip_prefix("## "))
        .unwrap_or("");
    report.section(&format!("{name} ({})", branch_label(branch)));
    let before = report.lines.len();
    let changes: Vec<&str> = lines.collect();
    let untracked = changes.iter().filter(|l| l.starts_with("??")).count();
    let modified = changes.iter().filter(|l| !l.starts_with("??")).count();
    if !changes.is_empty() {
        let names: Vec<&str> = changes
            .iter()
            .take(SHOWN)
            .map(|l| l.get(3..).unwrap_or(l))
            .collect();
        let more = if changes.len() > SHOWN { ", …" } else { "" };
        report.error(format!(
            "pas commité : {} modifié(s), {untracked} non suivi(s) : {}{more}",
            modified,
            names.join(", ")
        ));
    }
    branches(repository, report);
    if report.lines.len() == before {
        report.ok("tout est commité et poussé");
    }
}

/// `main...origin/main [ahead 2]` → `main → origin/main, 2 en avance`.
fn branch_label(line: &str) -> String {
    let (names, track) = line.split_once(" [").unwrap_or((line, ""));
    let track = track.trim_end_matches(']');
    let names = names.replace("...", " → ");
    if track.is_empty() {
        names
    } else {
        format!(
            "{names}, {}",
            track
                .replace("ahead", "en avance de")
                .replace("behind", "en retard de")
        )
    }
}

/// Each local branch: no upstream, or commits not pushed.
fn branches(repository: &Path, report: &mut Report) {
    let Some(refs) = util::stdout_of(
        repository,
        "git",
        &[
            "for-each-ref",
            "--format=%(refname:short)%09%(upstream:short)%09%(upstream:track)",
            "refs/heads",
        ],
    ) else {
        return;
    };
    for line in refs.lines() {
        let mut parts = line.split('\t');
        let branch = parts.next().unwrap_or("");
        let upstream = parts.next().unwrap_or("");
        let track = parts.next().unwrap_or("");
        if upstream.is_empty() {
            report.warn(format!(
                "branche {branch} sans amont : jamais poussée (git push -u origin {branch})"
            ));
        } else if track.contains("gone") {
            report.warn(format!(
                "branche {branch} : son amont {upstream} n'existe plus"
            ));
        } else if let Some(ahead) = ahead(track) {
            report.error(format!(
                "branche {branch} : {ahead} commit(s) pas poussé(s) vers {upstream}"
            ));
        }
    }
}

/// `[ahead 2, behind 1]` → 2.
fn ahead(track: &str) -> Option<u32> {
    let rest = track.split("ahead ").nth(1)?;
    rest.chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracking_is_read() {
        assert_eq!(ahead("[ahead 2, behind 1]"), Some(2));
        assert_eq!(ahead("[behind 1]"), None);
        assert_eq!(
            branch_label("main...origin/main [ahead 2]"),
            "main → origin/main, en avance de 2"
        );
        assert_eq!(
            branch_label("No commits yet on main"),
            "No commits yet on main"
        );
    }

    #[test]
    fn a_folder_of_repositories_is_walked() {
        let dir = crate::testdir::fresh("status");
        for repo in ["soiree", "games/a", "games/b"] {
            let path = dir.join(repo);
            std::fs::create_dir_all(&path).unwrap();
            util::run(&path, "git", &["init", "-q"]).unwrap();
        }
        std::fs::create_dir_all(dir.join("notes")).unwrap();
        let found: Vec<String> = repositories(&dir)
            .iter()
            .map(|p| util::shown(p, &dir))
            .collect();
        assert_eq!(found, vec!["soiree", "games/a", "games/b"]);
    }
}
