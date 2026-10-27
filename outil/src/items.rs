//! A game's own `CERTIFICATION.md` (the model's template is the reference):
//! the format of its items and of the catalogue items it sets aside, and
//! that every test it names exists in the code.

use std::collections::BTreeSet;
use std::path::Path;

use crate::catalogue;
use crate::markdown::{self, Line};
use crate::report::Report;
use crate::util;

pub const ITEMS: &str = "## Les items";
pub const EXCLUDED: &str = "## Items du catalogue écartés";
const COLUMNS: [&str; 3] = ["#", "Exigence", "Test"];
const ORIGIN: &str = "Origine";

#[derive(Debug, Default)]
pub struct Parsed {
    pub items: Vec<Item>,
    /// Catalogue items set aside: line and identifier.
    pub excluded: Vec<(usize, String)>,
}

#[derive(Debug)]
pub struct Item {
    pub line: usize,
    pub id: String,
    pub retired: bool,
    pub tests: Vec<String>,
    pub origin: Option<String>,
}

/// Checks `dir/CERTIFICATION.md`, if this is a game that has one.
pub fn check(dir: &Path, is_game: bool, report: &mut Report) {
    let path = dir.join("CERTIFICATION.md");
    let Ok(text) = util::read(&path) else {
        if is_game {
            report.warn("CERTIFICATION.md du jeu absent : les items propres au jeu n'ont nulle part où vivre");
        }
        return;
    };
    let before = report.lines.len();
    let parsed = lint(&text, "CERTIFICATION.md", report);

    let functions = functions_in(dir);
    for item in parsed.items.iter().filter(|i| !i.retired) {
        for test in &item.tests {
            if !functions.contains(test) {
                report.error(format!(
                    "CERTIFICATION.md:{} : {} nomme le test `{test}`, introuvable (attendu : une `fn {test}` dans le code)",
                    item.line, item.id
                ));
            }
        }
    }

    let catalogue_dir = dir.join(".soiree").join("certifications");
    if catalogue_dir.is_dir() {
        let known = catalogue::ids_in(&catalogue_dir);
        let origins = parsed
            .items
            .iter()
            .filter_map(|i| i.origin.as_ref().map(|o| (i.line, o.clone())));
        for (line, id) in origins.chain(parsed.excluded.iter().cloned()) {
            if !known.contains(&id) {
                report.warn(format!(
                    "CERTIFICATION.md:{line} : {id} n'est pas dans le catalogue (.soiree/certifications/)"
                ));
            }
        }
    }

    if report.lines.len() == before {
        let live = parsed.items.iter().filter(|i| !i.retired).count();
        if live == 0 {
            report.info("CERTIFICATION.md du jeu : aucun item pour l'instant");
        } else {
            report.ok(format!(
                "CERTIFICATION.md du jeu : {live} item(s), chaque test nommé existe ; {} item(s) du catalogue écarté(s)",
                parsed.excluded.len()
            ));
        }
    }
}

/// The format of the file. Errors name `file:line` and what is expected.
pub fn lint(text: &str, file: &str, report: &mut Report) -> Parsed {
    let lines = markdown::lines(text);
    let mut parsed = Parsed::default();
    let Some(items) = section(&lines, ITEMS) else {
        report.error(format!(
            "{file} : section « {ITEMS} » absente (attendu : un tableau | # | Exigence | Test | Origine |)"
        ));
        return parsed;
    };
    lint_table(&items, file, report, &mut parsed);
    if let Some(excluded) = section(&lines, EXCLUDED) {
        lint_excluded(&excluded, file, report, &mut parsed);
    }
    parsed
}

/// The lines read under `heading`, up to the next heading of level 1 or 2.
fn section<'a>(lines: &[Line<'a>], heading: &str) -> Option<Vec<Line<'a>>> {
    let start = lines
        .iter()
        .position(|l| l.read() && l.text.trim_end() == heading)?;
    Some(
        lines
            .iter()
            .skip(start + 1)
            .take_while(|l| !(l.read() && (l.text.starts_with("## ") || l.text.starts_with("# "))))
            .filter(|l| l.read())
            .copied()
            .collect(),
    )
}

fn lint_table(lines: &[Line<'_>], file: &str, report: &mut Report, parsed: &mut Parsed) {
    let mut rows = lines.iter().skip_while(|l| !is_row(l));
    let Some(header) = rows.next() else {
        report.error(format!(
            "{file} : « {ITEMS} » n'a pas de tableau (attendu : | # | Exigence | Test | Origine |)"
        ));
        return;
    };
    let columns = markdown::cells(header.text);
    let with_origin = columns == [&COLUMNS[..], &[ORIGIN]].concat();
    if columns != COLUMNS && !with_origin {
        report.error(format!(
            "{file}:{} : colonnes {} ; attendu, dans cet ordre : | # | Exigence | Test |, puis | Origine | si besoin",
            header.number,
            columns.join(", ")
        ));
        return;
    }
    match rows.next() {
        Some(line) if markdown::is_separator(line.text) => {}
        other => {
            report.error(format!(
                "{file}:{} : attendu la ligne de séparation |---|---|---|",
                other.map_or(header.number, |l| l.number)
            ));
            return;
        }
    }
    let mut seen = BTreeSet::new();
    let mut ended = false;
    for line in rows {
        if !is_row(line) {
            ended |= !line.text.trim().is_empty();
            continue;
        }
        if ended {
            report.error(format!(
                "{file}:{} : un seul tableau dans « {ITEMS} »",
                line.number
            ));
            return;
        }
        if let Some(item) = lint_row(line, columns.len(), file, report)
            && seen_once(&mut seen, &item, file, report)
        {
            parsed.items.push(item);
        }
    }
}

fn seen_once(seen: &mut BTreeSet<String>, item: &Item, file: &str, report: &mut Report) -> bool {
    if seen.insert(item.id.clone()) {
        return true;
    }
    report.error(format!(
        "{file}:{} : identifiant {} déjà utilisé (attendu : unique, jamais réutilisé)",
        item.line, item.id
    ));
    false
}

fn is_row(line: &Line<'_>) -> bool {
    line.text.trim_start().starts_with('|')
}

fn lint_row(line: &Line<'_>, width: usize, file: &str, report: &mut Report) -> Option<Item> {
    let at = format!("{file}:{}", line.number);
    let cells = markdown::cells(line.text);
    // A missing last `Origine` cell is still a valid row.
    if cells.len() != width && !(width == 4 && cells.len() == 3) {
        report.error(format!("{at} : {} cellules ; attendu {width}", cells.len()));
        return None;
    }
    let cell = |i: usize| cells.get(i).map_or("", String::as_str);
    let raw_id = cell(0);
    let retired = raw_id.starts_with("~~") && raw_id.ends_with("~~") && raw_id.len() > 4;
    let id = raw_id.trim_matches('~').to_owned();
    if !is_game_id(&id) {
        report.error(format!(
            "{at} : identifiant « {raw_id} » ; attendu J suivi d'un numéro (J1, J2…), barré (~~J3~~) s'il est retiré"
        ));
    }
    if cell(1).is_empty() {
        report.error(format!("{at} : {id} : Exigence vide ; attendu une phrase"));
    }
    let mut tests = Vec::new();
    for name in markdown::code_spans(cell(2)) {
        let last = name.rsplit("::").next().unwrap_or(name).trim();
        if markdown::is_ident(last) {
            tests.push(last.to_owned());
        } else {
            report.error(format!(
                "{at} : {id} : test `{name}` ; attendu le nom d'une fonction de test"
            ));
        }
    }
    if markdown::code_spans(cell(2)).is_empty() {
        report.error(format!(
            "{at} : {id} : Test vide ; attendu le nom d'un test entre accents graves, `comme_ceci`"
        ));
    }
    let origin = cell(3).trim_matches('`').trim();
    let origin = if origin.is_empty() {
        None
    } else if markdown::is_catalogue_id(origin) {
        Some(origin.to_owned())
    } else {
        report.error(format!(
            "{at} : {id} : origine « {origin} » ; attendu un identifiant du catalogue comme `VOTE-03` (2 à 5 majuscules, tiret, 2 chiffres), ou rien"
        ));
        None
    };
    is_game_id(&id).then_some(Item {
        line: line.number,
        id,
        retired,
        tests,
        origin,
    })
}

fn is_game_id(id: &str) -> bool {
    id.strip_prefix('J')
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

fn lint_excluded(lines: &[Line<'_>], file: &str, report: &mut Report, parsed: &mut Parsed) {
    for line in lines {
        let Some(entry) = line
            .text
            .strip_prefix("- ")
            .or_else(|| line.text.strip_prefix("* "))
        else {
            continue;
        };
        let at = format!("{file}:{}", line.number);
        let Some((id, reason)) = entry.split_once(':') else {
            report.error(format!(
                "{at} : attendu « - <ID> : <raison> », comme « - VOTE-02 : pas de vote secret »"
            ));
            continue;
        };
        let id = id.trim().trim_matches('`');
        if !markdown::is_catalogue_id(id) {
            report.error(format!(
                "{at} : « {id} » ; attendu un identifiant du catalogue comme VOTE-02"
            ));
        } else if reason.trim().is_empty() {
            report.error(format!(
                "{at} : {id} écarté sans raison ; attendu « - {id} : <raison> »"
            ));
        } else {
            parsed.excluded.push((line.number, id.to_owned()));
        }
    }
}

/// Every `fn name` in the Rust files of `dir`, outside build outputs.
fn functions_in(dir: &Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for file in util::files_under(
        dir,
        &[".rs"],
        &["target", ".git", ".soiree", "node_modules"],
    ) {
        if let Ok(text) = util::read(&file) {
            names.extend(function_names(&text));
        }
    }
    names
}

fn function_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("fn ") {
        let before = rest.get(..at).and_then(|b| b.chars().last());
        let after = rest.get(at + 3..).unwrap_or("");
        if before.is_none_or(|c| !(c.is_alphanumeric() || c == '_')) {
            let name: String = after
                .trim_start()
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                names.push(name);
            }
        }
        rest = after;
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Level;

    const MODEL: &str = include_str!("../../modele/CERTIFICATION.md");

    fn errors(text: &str) -> Vec<String> {
        let mut report = Report::quiet();
        lint(text, "C.md", &mut report);
        report
            .lines
            .into_iter()
            .filter(|(l, _)| *l == Level::Error)
            .map(|(_, m)| m)
            .collect()
    }

    fn game(rows: &str, excluded: &str) -> String {
        format!(
            "# x\n\n{ITEMS}\n\n| # | Exigence | Test | Origine |\n|---|---|---|---|\n{rows}\n{EXCLUDED}\n\n{excluded}\n"
        )
    }

    #[test]
    fn the_model_template_is_valid_and_empty() {
        let mut report = Report::quiet();
        let parsed = lint(MODEL, "C.md", &mut report);
        assert_eq!(report.count(Level::Error), 0, "{:?}", report.lines);
        assert!(parsed.items.is_empty() && parsed.excluded.is_empty());
    }

    #[test]
    fn a_valid_file_parses_fully() {
        let text = game(
            "| J1 | Un vote tardif ne compte pas. | `tests::late_vote_is_ignored` | `VOTE-03` |\n| J2 | Pas de doublon. | `a`, `b` | |\n| ~~J3~~ | Retiré. | `old` |",
            "Intro libre.\n- VOTE-02 : pas de vote secret.",
        );
        let mut report = Report::quiet();
        let parsed = lint(&text, "C.md", &mut report);
        assert_eq!(report.count(Level::Error), 0, "{:?}", report.lines);
        let tests: Vec<_> = parsed.items.iter().map(|i| i.tests.join("+")).collect();
        assert_eq!(tests, vec!["late_vote_is_ignored", "a+b", "old"]);
        assert_eq!(
            parsed.items.first().unwrap().origin.as_deref(),
            Some("VOTE-03")
        );
        assert!(parsed.items.get(2).unwrap().retired);
        assert_eq!(parsed.excluded, vec![(13, "VOTE-02".to_owned())]);
    }

    #[test]
    fn each_broken_rule_names_its_line() {
        let cases = [
            ("| X1 | a | `t` | |", "C.md:7 : identifiant « X1 »"),
            (
                "| J1 | a | `t` | |\n| J1 | b | `u` | |",
                "C.md:8 : identifiant J1 déjà utilisé",
            ),
            (
                "| J1 | a | `t` | |\n| J1 | b | `u` | `bad` |",
                "C.md:8 : identifiant J1 déjà utilisé",
            ),
            ("| J1 |  | `t` | |", "Exigence vide"),
            ("| J1 | a | t | |", "Test vide"),
            ("| J1 | a | `not a name` | |", "test `not a name`"),
            ("| J1 | a | `t` | `vote-3` |", "origine « vote-3 »"),
            ("| J1 | a |", "2 cellules ; attendu 4"),
        ];
        for (rows, expected) in cases {
            let found = errors(&game(rows, ""));
            assert!(
                found.iter().any(|e| e.contains(expected)),
                "{expected} ∉ {found:?}"
            );
        }
        let excluded = [
            ("- VOTE-02", "attendu « - <ID> : <raison> »"),
            ("- VOTE-2 : x", "« VOTE-2 »"),
            ("- VOTE-02 :  ", "écarté sans raison"),
        ];
        for (line, expected) in excluded {
            let found = errors(&game("| J1 | a | `t` | |", line));
            assert!(
                found.iter().any(|e| e.contains(expected)),
                "{expected} ∉ {found:?}"
            );
        }
    }

    #[test]
    fn header_order_and_presence_are_enforced() {
        let swapped = "## Les items\n\n| # | Test | Exigence |\n|---|---|---|\n";
        assert!(
            errors(swapped)
                .iter()
                .any(|e| e.contains("C.md:3 : colonnes"))
        );
        assert!(
            errors("# rien\n")
                .iter()
                .any(|e| e.contains("section « ## Les items » absente"))
        );
        let without_origin =
            "## Les items\n\n| # | Exigence | Test |\n|---|---|---|\n| J1 | a | `t` |\n";
        assert_eq!(errors(without_origin), Vec::<String>::new());
    }

    #[test]
    fn function_names_are_found() {
        let names = function_names(
            "fn a() {}\n proptest! { #[test] fn late_vote(x in 0..3) {} }\nlet fnord = 1; pub(crate) fn  b<T>() {}",
        );
        assert_eq!(names, vec!["a", "late_vote", "b"]);
    }
}
