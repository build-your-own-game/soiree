//! « Écarts au manifeste » in `LEARNINGS.md`: each engineering principle
//! (1 to 7) is respected, or its gap is written with its reason, or it is
//! still to do. Ported from the former shell check.

use std::path::Path;

use crate::report::Report;
use crate::util;

const PRINCIPLES: u32 = 7;

pub fn check(dir: &Path, report: &mut Report) {
    let Ok(text) = util::read(&dir.join("LEARNINGS.md")) else {
        report.error("LEARNINGS.md absent");
        return;
    };
    let Some(points) = points(&text) else {
        report.error(
            "LEARNINGS.md : section « ## Écarts au manifeste » absente (un point par principe d'ingénierie, 1 à 7)",
        );
        return;
    };
    for number in 1..=PRINCIPLES {
        let text = points
            .iter()
            .find(|(n, _)| *n == number)
            .map_or("", |(_, t)| t.as_str());
        judge(dir, number, text, report);
    }
}

fn judge(dir: &Path, number: u32, text: &str, report: &mut Report) {
    if text.contains("**Respecté") {
        if number == 5 && dir.join("web").is_dir() && !sends_coep(&dir.join("crates")) {
            report
                .error("principe 5 marqué respecté, mais aucun crate n'envoie l'en-tête COOP/COEP");
        }
    } else if text.contains("**Écart") {
        if !text.contains("Raison") {
            report.error(format!("principe {number} : écart sans « Raison : »"));
        }
    } else if text.contains("**À faire") {
        report.warn(format!("principe {number} à faire : {}", todo_text(text)));
    } else if text.is_empty() {
        report.error(format!(
            "principe {number} absent de « Écarts au manifeste »"
        ));
    } else {
        report.error(format!(
            "principe {number} : commence par **Respecté.**, **Écart.** ou **À faire.**"
        ));
    }
}

/// The numbered points of the section, each with its continuation lines, or
/// `None` without the section. Like the former shell check.
fn points(text: &str) -> Option<Vec<(u32, String)>> {
    let mut inside = false;
    let mut found = false;
    let mut points: Vec<(u32, String)> = Vec::new();
    let mut current: Option<u32> = None;
    for line in text.lines() {
        if line.starts_with("## Écarts au manifeste") {
            inside = true;
            found = true;
            continue;
        }
        if inside && line.starts_with("## ") {
            inside = false;
        }
        if !inside {
            continue;
        }
        if let Some(number) = numbered(line) {
            current = Some(number);
            points.retain(|(n, _)| *n != number);
            points.push((number, line.to_owned()));
        } else if let Some(number) = current
            && !line.trim().is_empty()
            && let Some((_, text)) = points.iter_mut().find(|(n, _)| *n == number)
        {
            text.push(' ');
            text.push_str(line);
        }
    }
    found.then_some(points)
}

/// `3. …` gives 3.
fn numbered(line: &str) -> Option<u32> {
    let digits: String = line.chars().take_while(char::is_ascii_digit).collect();
    let rest = line.get(digits.len()..)?;
    if digits.is_empty() || !rest.starts_with(". ") {
        return None;
    }
    digits.parse().ok()
}

/// The point without its `N. **À faire.**` prefix, cut at 110 characters.
fn todo_text(text: &str) -> String {
    let rest = text.trim_start_matches(|c: char| c.is_ascii_digit());
    let rest = rest.strip_prefix(". ").unwrap_or(rest);
    let rest = rest.strip_prefix("**À faire").unwrap_or(rest);
    let rest = rest.strip_prefix('.').unwrap_or(rest);
    let rest = rest.strip_prefix("**").unwrap_or(rest);
    let rest = rest.strip_prefix('.').unwrap_or(rest);
    rest.trim_start().chars().take(110).collect()
}

/// Some file under `crates/` names the header (`grep -rqi`).
fn sends_coep(crates: &Path) -> bool {
    util::files_under(crates, &[""], &[]).iter().any(|file| {
        std::fs::read(file).is_ok_and(|bytes| {
            String::from_utf8_lossy(&bytes)
                .to_lowercase()
                .contains("cross-origin-embedder-policy")
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_gather_their_continuation_lines() {
        let text = "# L\n\n## Écarts au manifeste\n\nIntro.\n\n1. **Respecté.** a\n   suite\n2. **Écart.** b\n\n3. **À faire.** c\n## Autre\n4. **Respecté.** hors section\n";
        let found = points(text).unwrap();
        assert_eq!(found.len(), 3);
        assert_eq!(found.first().unwrap().1, "1. **Respecté.** a    suite");
        assert!(points(text.replace("## Écarts", "## Autres").as_str()).is_none());
    }

    #[test]
    fn todo_text_drops_its_prefix() {
        assert_eq!(todo_text("7. **À faire.** Ni overlay"), "Ni overlay");
        assert_eq!(todo_text("2. **À faire**. Plus tard"), "Plus tard");
    }

    #[test]
    fn a_gap_without_reason_and_a_missing_point_are_errors() {
        let dir = std::env::temp_dir();
        let mut report = Report::quiet();
        judge(&dir, 2, "2. **Écart.** pas de pas fixe", &mut report);
        judge(&dir, 3, "", &mut report);
        judge(&dir, 4, "4. Respecté sans gras", &mut report);
        judge(&dir, 1, "1. **Écart.** x. Raison : y", &mut report);
        assert_eq!(report.count(crate::report::Level::Error), 3);
    }
}
