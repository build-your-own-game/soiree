//! The shared catalogue of items by mechanic, `certifications/` in the shared
//! repository: one file per prefix, each item in the same five fields, and a
//! README that lists every identifier.
//!
//! ```text
//! ### PHY-01 · <titre>
//! - **Mécaniques :** …
//! - **Le bug d'origine :** … ([source](https://…))
//! - **Propriété :** …
//! - **Test :** …   (peut se poursuivre par un bloc de code)
//! - **Adapter :** …
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::markdown::{self, Line};
use crate::report::Report;
use crate::util;

pub const FIELDS: [&str; 5] = [
    "Mécaniques",
    "Le bug d'origine",
    "Propriété",
    "Test",
    "Adapter",
];
const SOURCE: &str = "Le bug d'origine";
const UNVERIFIED: &str = "source non vérifiée";

#[derive(Debug, Clone)]
pub struct Entry {
    pub file: String,
    pub line: usize,
    pub id: String,
}

struct Field {
    label: String,
    line: usize,
    content: String,
}

struct Open {
    id: String,
    line: usize,
    fields: Vec<Field>,
}

/// Lints `certifications/` (with its README.md) under `dir`.
pub fn check(dir: &Path, report: &mut Report) {
    let folder = dir.join("certifications");
    let before = report.lines.len();
    let mut entries: Vec<Entry> = Vec::new();
    let files = catalogue_files(&folder);
    for path in &files {
        let shown = util::shown(path, dir);
        match util::read(path) {
            Ok(text) => entries.extend(lint_file(&text, &shown, report)),
            Err(error) => report.error(error),
        }
    }
    let mut first: BTreeMap<&str, &Entry> = BTreeMap::new();
    for entry in &entries {
        if let Some(earlier) = first.insert(&entry.id, entry) {
            report.error(format!(
                "{}:{} : {} existe déjà ({}:{}) ; attendu des identifiants uniques dans tout le catalogue",
                entry.file, entry.line, entry.id, earlier.file, earlier.line
            ));
        }
    }
    let readme = folder.join("README.md");
    match util::read(&readme) {
        Ok(text) => lint_readme(&text, &util::shown(&readme, dir), &entries, report),
        Err(error) => report.error(error),
    }
    if report.lines.len() == before {
        report.ok(format!(
            "catalogue : {} item(s) dans {} fichier(s), README.md à jour",
            entries.len(),
            files.len()
        ));
    }
}

/// The identifiers of a catalogue folder, without judging its format.
pub fn ids_in(folder: &Path) -> BTreeSet<String> {
    let mut quiet = Report::quiet();
    catalogue_files(folder)
        .iter()
        .filter_map(|path| util::read(path).ok())
        .flat_map(|text| lint_file(&text, "", &mut quiet))
        .map(|entry| entry.id)
        .collect()
}

fn catalogue_files(folder: &Path) -> Vec<std::path::PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(folder)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.extension().is_some_and(|e| e == "md")
                        && p.file_name().is_some_and(|n| n != "README.md")
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

pub fn lint_file(text: &str, file: &str, report: &mut Report) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut prefix: Option<String> = None;
    let mut open: Option<Open> = None;
    for line in markdown::lines(text) {
        let heading = !line.fenced && !line.commented;
        if heading && line.text.starts_with("### ") {
            close(open.take(), file, report);
            let Some((id, title)) = line.text.get(4..).and_then(|rest| rest.split_once(" · "))
            else {
                report.error(format!(
                    "{file}:{} : titre « {} » ; attendu « ### PRÉFIXE-NN · titre »",
                    line.number,
                    line.text.trim()
                ));
                continue;
            };
            let id = id.trim();
            if !markdown::is_catalogue_id(id) {
                report.error(format!(
                    "{file}:{} : identifiant « {id} » ; attendu 2 à 5 majuscules, un tiret, 2 chiffres (PHY-01)",
                    line.number
                ));
                continue;
            }
            if title.trim().is_empty() {
                report.error(format!(
                    "{file}:{} : {id} sans titre ; attendu « ### {id} · titre »",
                    line.number
                ));
            }
            let this = id.split('-').next().unwrap_or(id).to_owned();
            match &prefix {
                Some(p) if *p != this => report.error(format!(
                    "{file}:{} : préfixe {this} ; attendu un seul préfixe par fichier, ici {p}",
                    line.number
                )),
                Some(_) => {}
                None => prefix = Some(this),
            }
            entries.push(Entry {
                file: file.to_owned(),
                line: line.number,
                id: id.to_owned(),
            });
            open = Some(Open {
                id: id.to_owned(),
                line: line.number,
                fields: Vec::new(),
            });
        } else if heading && (line.text.starts_with("## ") || line.text.starts_with("# ")) {
            close(open.take(), file, report);
        } else if let Some(item) = open.as_mut() {
            feed(item, &line, file, report);
        }
    }
    close(open, file, report);
    entries
}

/// One line inside an item: a new field, or the continuation of the last.
fn feed(item: &mut Open, line: &Line<'_>, file: &str, report: &mut Report) {
    if line.read()
        && let Some(rest) = line.text.strip_prefix("- **")
    {
        let (label, content) = rest.split_once(":**").unwrap_or((rest, ""));
        item.fields.push(Field {
            label: label.trim().replace('’', "'"),
            line: line.number,
            content: content.trim().to_owned(),
        });
    } else if !line.text.trim().is_empty() {
        if let Some(field) = item.fields.last_mut() {
            field.content.push('\n');
            field.content.push_str(line.text.trim());
        } else {
            report.error(format!(
                "{file}:{} : {} : texte avant le premier champ ; attendu « - **Mécaniques :** … »",
                line.number, item.id
            ));
        }
    }
}

/// An item is complete: exactly the five fields, in order, none empty. A
/// missing field is reported once, where the next one shows up.
fn close(item: Option<Open>, file: &str, report: &mut Report) {
    let Some(item) = item else {
        return;
    };
    let expected = FIELDS.join(", ");
    let mut next = 0;
    for field in &item.fields {
        let later = FIELDS
            .iter()
            .enumerate()
            .skip(next)
            .find(|(_, f)| **f == field.label)
            .map(|(at, _)| at);
        match (later, FIELDS.get(next)) {
            (Some(at), _) => {
                for missing in FIELDS.get(next..at).unwrap_or_default() {
                    report.error(format!(
                        "{file}:{} : {} : il manque « {missing} » avant « {} » ; attendu exactement {expected}, dans cet ordre",
                        field.line, item.id, field.label
                    ));
                }
                next = at.saturating_add(1);
            }
            (None, Some(wanted)) => {
                report.error(format!(
                    "{file}:{} : {} : champ « {} » ; attendu « {wanted} » à cette place ({expected})",
                    field.line, item.id, field.label
                ));
                next = next.saturating_add(1);
            }
            (None, None) => report.error(format!(
                "{file}:{} : {} : champ en trop « {} » ; attendu exactement {expected}",
                field.line, item.id, field.label
            )),
        }
        lint_content(field, &item.id, file, report);
    }
    for missing in FIELDS.iter().skip(next) {
        report.error(format!(
            "{file}:{} : {} : il manque « {missing} » ; attendu exactement {expected}, dans cet ordre",
            item.line, item.id
        ));
    }
}

fn lint_content(field: &Field, id: &str, file: &str, report: &mut Report) {
    let content = field.content.trim().trim_matches('…').trim();
    if content.is_empty() {
        report.error(format!(
            "{file}:{} : {id} : « {} » vide ; attendu un texte",
            field.line, field.label
        ));
    } else if field.label == SOURCE
        && !content.contains("http://")
        && !content.contains("https://")
        && !content.to_lowercase().contains(UNVERIFIED)
    {
        report.error(format!(
            "{file}:{} : {id} : « {SOURCE} » sans lien ; attendu un lien http(s) ou la mention « {UNVERIFIED} »",
            field.line
        ));
    }
}

fn lint_readme(text: &str, file: &str, entries: &[Entry], report: &mut Report) {
    let known: BTreeSet<&str> = entries.iter().map(|e| e.id.as_str()).collect();
    let mut listed = BTreeSet::new();
    for line in markdown::lines(text).iter().filter(|l| l.read()) {
        for id in markdown::catalogue_ids(line.text) {
            if !known.contains(id.as_str()) {
                report.error(format!(
                    "{file}:{} : cite {id}, qui n'existe dans aucun fichier du catalogue",
                    line.number
                ));
            }
            listed.insert(id);
        }
    }
    for entry in entries {
        if !listed.contains(&entry.id) {
            report.error(format!(
                "{file} : ne liste pas {} ({}:{}) ; attendu chaque identifiant du catalogue",
                entry.id, entry.file, entry.line
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Level;

    const VALID: &str = "# Physique\n\nIntro libre.\n\n### PHY-01 · Deux sauts à la même image\n- **Mécaniques :** plateformes, collisions\n- **Le bug d'origine :** on se traversait ([source](https://example.org/1))\n- **Propriété :** deux corps ne se recouvrent jamais\n- **Test :**\n  ```rust\n  fn same_frame_jumps_collide() {}\n  ```\n- **Adapter :** remplacer la hitbox par celle du jeu\n\n### PHY-02 · Pente\n- **Mécaniques :** pentes\n- **Le bug d’origine :** source non vérifiée\n- **Propriété :** on ne glisse pas à l'arrêt\n- **Test :** `slope_rest`\n- **Adapter :** l'angle\n";

    fn errors_of(text: &str) -> Vec<String> {
        let mut report = Report::quiet();
        lint_file(text, "PHY.md", &mut report);
        report
            .lines
            .into_iter()
            .filter(|(l, _)| *l == Level::Error)
            .map(|(_, m)| m)
            .collect()
    }

    #[test]
    fn a_valid_file_has_no_error_and_two_entries() {
        let mut report = Report::quiet();
        let entries = lint_file(VALID, "PHY.md", &mut report);
        assert!(report.lines.is_empty(), "{:?}", report.lines);
        let ids: Vec<_> = entries.iter().map(|e| (e.id.as_str(), e.line)).collect();
        assert_eq!(ids, vec![("PHY-01", 5), ("PHY-02", 15)]);
    }

    #[test]
    fn each_broken_rule_is_one_error_at_its_line() {
        let cases = [
            (
                VALID.replace("### PHY-01 · Deux", "### PHY-01 - Deux"),
                "PHY.md:5 : titre",
            ),
            (
                VALID.replace("### PHY-01 · Deux sauts à la même image", "### PHY-01 · "),
                "PHY.md:5 : PHY-01 sans titre",
            ),
            (
                VALID.replace("PHY-02", "PHY-2"),
                "PHY.md:15 : identifiant « PHY-2 »",
            ),
            (
                VALID.replace("PHY-02", "VOTE-02"),
                "PHY.md:15 : préfixe VOTE",
            ),
            (
                VALID.replace("PHY-02", "PHY-01"),
                "PHY.md:15 : PHY-01 existe déjà",
            ),
            (
                VALID.replace("- **Propriété :** deux corps ne se recouvrent jamais\n", ""),
                "PHY.md:8 : PHY-01 : il manque « Propriété » avant « Test »",
            ),
            (
                VALID.replace("**Mécaniques :** pentes", "**Mécanique :** pentes"),
                "PHY.md:16 : PHY-02 : champ « Mécanique »",
            ),
            (
                VALID.replace(
                    "- **Adapter :** l'angle",
                    "- **Adapter :** l'angle\n- **Notes :** x",
                ),
                "champ en trop « Notes »",
            ),
            (
                VALID.replace(
                    "**Propriété :** on ne glisse pas à l'arrêt",
                    "**Propriété :** ",
                ),
                "PHY.md:18 : PHY-02 : « Propriété » vide",
            ),
            (
                VALID.replace("([source](https://example.org/1))", "(voir le forum)"),
                "PHY.md:7 : PHY-01 : « Le bug d'origine » sans lien",
            ),
            (
                VALID.replace("### PHY-02 · Pente\n", "### PHY-02 · Pente\nUn mot.\n"),
                "PHY.md:16 : PHY-02 : texte avant le premier champ",
            ),
        ];
        for (text, expected) in cases {
            let found = if expected.contains("existe déjà") {
                let dir = fixture(&[("PHY.md", &text)], "PHY-01");
                let mut report = Report::quiet();
                check(&dir, &mut report);
                report.lines.into_iter().map(|(_, m)| m).collect()
            } else {
                errors_of(&text)
            };
            assert_eq!(found.len(), 1, "{expected} : {found:?}");
            assert!(
                found.iter().any(|e| e.contains(expected)),
                "{expected} ∉ {found:?}"
            );
        }
    }

    #[test]
    fn the_readme_lists_every_id_and_no_other() {
        let dir = fixture(&[("PHY.md", VALID)], "PHY-01, PHY-02");
        let mut report = Report::quiet();
        check(&dir, &mut report);
        assert_eq!(report.count(Level::Error), 0, "{:?}", report.lines);

        let dir = fixture(&[("PHY.md", VALID)], "PHY-01 et VOTE-09");
        let mut report = Report::quiet();
        check(&dir, &mut report);
        let found: Vec<_> = report.lines.iter().map(|(_, m)| m.as_str()).collect();
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(
            found
                .iter()
                .any(|m| m.contains("README.md:3 : cite VOTE-09"))
        );
        assert!(found.iter().any(|m| m.contains("ne liste pas PHY-02")));
        assert_eq!(ids_in(&dir.join("certifications")).len(), 2);
    }

    /// A shared repository with `certifications/`, in a fresh temporary folder.
    fn fixture(files: &[(&str, &str)], readme_ids: &str) -> std::path::PathBuf {
        let dir = crate::testdir::fresh("catalogue");
        for (name, text) in files {
            util::write(&dir.join("certifications").join(name), text).unwrap();
        }
        util::write(
            &dir.join("certifications/README.md"),
            &format!("# Catalogue\n\n{readme_ids}\n\n```\n### XYZ-99 · exemple\n```\n"),
        )
        .unwrap();
        dir
    }
}
