//! Legacy Markdown parser used only by the explicit one-way migration.
//! Names in this format are declarations, never execution evidence.

use std::collections::BTreeSet;

use crate::markdown::{self, Line};
use crate::report::Report;

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
    let mut after = lines
        .iter()
        .skip_while(|l| !(l.read() && l.text.trim_end() == heading));
    after.next()?;
    Some(
        after
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Level;

    const MODEL: &str = include_str!("../tests/fixtures/legacy-certification.md");

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
    fn a_historical_template_remains_migratable() {
        let mut report = Report::quiet();
        let parsed = lint(MODEL, "C.md", &mut report);
        assert_eq!(report.count(Level::Error), 0, "{:?}", report.lines);
        assert_eq!(parsed.items.len(), 5);
        assert_eq!(parsed.excluded.len(), 0);
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
}
