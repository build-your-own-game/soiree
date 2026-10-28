//! The shared rules, compared by meaning rather than by line: the toolchain,
//! the edition, lints and profiles of `Cargo.toml`, and `clippy.toml`.
//!
//! Hardening is free: a lint may be stricter than the rule, and an `allow` of
//! the rules may be dropped. Anything else is an error.

use std::path::Path;

use toml::{Table, Value};

use crate::embedded::rules;
use crate::report::Report;
use crate::util;

pub fn check_toolchain(dir: &Path, report: &mut Report) {
    let path = dir.join("rust-toolchain.toml");
    if !path.exists() {
        report.error("rust-toolchain.toml absent");
        return;
    }
    let (project, expected) = match (
        util::read(&path).and_then(|t| util::parse_toml(&t, "rust-toolchain.toml")),
        util::parse_toml(rules::TOOLCHAIN, "regles/rust-toolchain.toml"),
    ) {
        (Ok(project), Ok(expected)) => (project, expected),
        (Err(error), _) | (_, Err(error)) => {
            report.error(error);
            return;
        }
    };
    let empty = Table::new();
    let project = project
        .get("toolchain")
        .and_then(Value::as_table)
        .unwrap_or(&empty);
    let expected = expected
        .get("toolchain")
        .and_then(Value::as_table)
        .unwrap_or(&empty);
    let before = report.lines.len();
    if project.get("channel") != expected.get("channel") {
        report.error(format!(
            "rust-toolchain.toml : channel {} au lieu de {}",
            shown(project.get("channel")),
            shown(expected.get("channel"))
        ));
    }
    for (key, what) in [("targets", "cible"), ("components", "composant")] {
        for wanted in strings(expected.get(key)) {
            if !strings(project.get(key)).contains(&wanted) {
                report.error(format!("rust-toolchain.toml : {what} {wanted} absent"));
            }
        }
    }
    if report.lines.len() == before {
        report.ok(format!(
            "toolchain {} des règles, avec {}",
            expected
                .get("channel")
                .and_then(Value::as_str)
                .unwrap_or("?"),
            [
                strings(expected.get("components")),
                strings(expected.get("targets"))
            ]
            .concat()
            .join(", ")
        ));
    }
}

/// The root `Cargo.toml`: edition and Rust version, lints, profiles.
pub fn check_cargo(project: &Table, report: &mut Report) {
    let rules = match util::parse_toml(rules::CARGO, "regles/cargo.toml") {
        Ok(rules) => rules,
        Err(error) => {
            report.error(error);
            return;
        }
    };
    let before = report.lines.len();
    let workspace = project.get("workspace").and_then(Value::as_table);
    let package = workspace
        .and_then(|w| w.get("package"))
        .or_else(|| project.get("package"))
        .and_then(Value::as_table);
    for (key, value) in table(rules.get("package")) {
        let actual = package.and_then(|p| p.get(key));
        if actual != Some(value) {
            report.error(format!(
                "Cargo.toml : {key} = {value} attendu ([workspace.package] ou [package]), trouvé {}",
                shown(actual)
            ));
        }
    }
    let lints = workspace
        .and_then(|w| w.get("lints"))
        .or_else(|| project.get("lints"))
        .and_then(Value::as_table);
    for (group, expected) in table(rules.get("lints")) {
        let actual = lints.and_then(|l| l.get(group)).and_then(Value::as_table);
        for (lint, rule) in table(Some(expected)) {
            check_lint(group, lint, rule, actual.and_then(|a| a.get(lint)), report);
        }
    }
    check_leaves(
        "profile",
        rules.get("profile"),
        project.get("profile"),
        report,
    );
    if report.lines.len() == before {
        report.ok("édition, lints et profils des règles");
    }
}

fn check_lint(group: &str, lint: &str, rule: &Value, actual: Option<&Value>, report: &mut Report) {
    let Some(wanted) = level(Some(rule)) else {
        return;
    };
    if wanted == 0 {
        return;
    }
    let name = format!("{group}::{lint}");
    let Some(found) = level(actual) else {
        report.error(format!(
            "Cargo.toml : lint {name} absent (il faut {})",
            rule_text(rule)
        ));
        return;
    };
    if found < wanted {
        report.error(format!(
            "Cargo.toml : lint {name} plus faible que la règle ({})",
            rule_text(rule)
        ));
    }
    let priority = |v: Option<&Value>| {
        v.and_then(|v| v.get("priority"))
            .and_then(Value::as_integer)
    };
    if let Some(wanted) = priority(Some(rule))
        && priority(actual) != Some(wanted)
    {
        report.error(format!(
            "Cargo.toml : lint {name} doit garder priority = {wanted}"
        ));
    }
}

/// 0 allow, 1 warn, 2 deny, 3 forbid.
fn level(value: Option<&Value>) -> Option<u8> {
    let value = value?;
    // `"deny"` or `{ level = "deny", priority = -1 }`; anything else has none.
    let text = if let Value::Table(t) = value {
        t.get("level")?.as_str()?
    } else {
        value.as_str()?
    };
    ["allow", "warn", "deny", "forbid"]
        .iter()
        .position(|l| *l == text)
        .and_then(|p| u8::try_from(p).ok())
}

fn rule_text(rule: &Value) -> String {
    if let Value::String(s) = rule {
        format!("\"{s}\"")
    } else {
        rule.to_string()
    }
}

/// Every leaf of `expected` has the same value in `actual`.
fn check_leaves(path: &str, expected: Option<&Value>, actual: Option<&Value>, report: &mut Report) {
    match expected {
        Some(Value::Table(expected)) => {
            for (key, value) in expected {
                let shown_key = if key.contains('*') {
                    format!("\"{key}\"")
                } else {
                    key.clone()
                };
                check_leaves(
                    &format!("{path}.{shown_key}"),
                    Some(value),
                    actual.and_then(|a| a.get(key)),
                    report,
                );
            }
        }
        Some(value) if actual != Some(value) => report.error(format!(
            "Cargo.toml : {path} = {value} attendu, trouvé {}",
            shown(actual)
        )),
        _ => {}
    }
}

/// A workspace member's `Cargo.toml` inherits the workspace's lints.
pub fn inherits_lints(member: &Table) -> bool {
    member
        .get("lints")
        .and_then(|l| l.get("workspace"))
        .and_then(Value::as_bool)
        == Some(true)
}

pub fn check_clippy(dir: &Path, report: &mut Report) {
    let path = dir.join("clippy.toml");
    if !path.exists() {
        report.error("clippy.toml absent");
        return;
    }
    let (project, rules) = match (
        util::read(&path).and_then(|t| util::parse_toml(&t, "clippy.toml")),
        util::parse_toml(rules::CLIPPY, "regles/clippy.toml"),
    ) {
        (Ok(project), Ok(rules)) => (project, rules),
        (Err(error), _) | (_, Err(error)) => {
            report.error(error);
            return;
        }
    };
    let before = report.lines.len();
    for (key, rule) in &rules {
        let actual = project.get(key);
        if stricter_permission(key, actual.and_then(Value::as_bool), rule.as_bool()) {
            continue;
        }
        if let Value::Array(entries) = rule {
            let have: Vec<String> = actual
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(entry_path).collect())
                .unwrap_or_default();
            for wanted in entries.iter().filter_map(entry_path) {
                if !have.contains(&wanted) {
                    report.error(format!("clippy.toml : il manque {wanted} dans {key}"));
                }
            }
        } else if actual != Some(rule) {
            report.error(format!(
                "clippy.toml : {key} = {rule} attendu, trouvé {}",
                shown(actual)
            ));
        }
    }
    if report.lines.len() == before {
        report.ok("types et méthodes interdits des règles");
    }
}

pub fn stricter_permission(key: &str, actual: Option<bool>, expected: Option<bool>) -> bool {
    matches!(
        key,
        "allow-unwrap-in-tests"
            | "allow-expect-in-tests"
            | "allow-panic-in-tests"
            | "allow-indexing-slicing-in-tests"
    ) && actual == Some(false)
        && expected == Some(true)
}

/// `"std::collections::HashMap"` or `{ path = "…", reason = "…" }`.
fn entry_path(entry: &Value) -> Option<String> {
    let path = if let Value::Table(t) = entry {
        t.get("path")?.as_str()
    } else {
        entry.as_str()
    };
    path.map(str::to_owned)
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn table(value: Option<&Value>) -> Vec<(&String, &Value)> {
    value
        .and_then(Value::as_table)
        .map(|t| t.iter().collect())
        .unwrap_or_default()
}

fn shown(value: Option<&Value>) -> String {
    value.map_or_else(|| "rien".to_owned(), ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Level;

    #[test]
    fn stricter_test_permissions_are_accepted_by_the_actual_rule_checker() {
        let dir = crate::testdir::fresh("strict-test-permissions");
        util::write(
            &dir.join("clippy.toml"),
            &rules::CLIPPY.replace(
                "allow-unwrap-in-tests = true",
                "allow-unwrap-in-tests = false",
            ),
        )
        .unwrap();
        let mut report = Report::quiet();
        check_clippy(&dir, &mut report);
        assert_eq!(report.count(Level::Error), 0, "{:?}", report.lines);
    }

    fn errors(report: &Report) -> Vec<String> {
        report
            .lines
            .iter()
            .filter(|(l, _)| *l == Level::Error)
            .map(|(_, m)| m.clone())
            .collect()
    }

    fn workspace(rules_text: &str) -> Table {
        let mut text = rules_text
            .replace("[package]", "[workspace.package]")
            .replace("[lints.", "[workspace.lints.");
        text.insert_str(0, "[workspace]\nmembers = []\n");
        util::parse_toml(&text, "test").unwrap()
    }

    #[test]
    fn the_rules_pass_themselves_as_package_and_as_workspace() {
        let mut report = Report::quiet();
        check_cargo(&util::parse_toml(rules::CARGO, "r").unwrap(), &mut report);
        check_cargo(&workspace(rules::CARGO), &mut report);
        assert_eq!(errors(&report), Vec::<String>::new());
    }

    #[test]
    fn hardening_passes_and_softening_fails() {
        let hardened = rules::CARGO
            .replace("cast_precision_loss = \"allow\"", "")
            .replace("unwrap_used = \"deny\"", "unwrap_used = \"forbid\"");
        let mut report = Report::quiet();
        check_cargo(&workspace(&hardened), &mut report);
        assert_eq!(errors(&report), Vec::<String>::new());

        let softened = rules::CARGO
            .replace("unwrap_used = \"deny\"", "unwrap_used = \"warn\"")
            .replace("todo = \"deny\"", "")
            .replace("overflow-checks = true", "overflow-checks = false")
            .replace(
                "pedantic = { level = \"deny\", priority = -1 }",
                "pedantic = \"deny\"",
            );
        let mut report = Report::quiet();
        check_cargo(&workspace(&softened), &mut report);
        let found = errors(&report);
        assert_eq!(found.len(), 4, "{found:?}");
        assert!(found.iter().any(|e| e.contains("unwrap_used plus faible")));
        assert!(found.iter().any(|e| e.contains("todo absent")));
        assert!(found.iter().any(|e| e.contains("overflow-checks")));
        assert!(
            found
                .iter()
                .any(|e| e.contains("pedantic doit garder priority"))
        );
    }

    #[test]
    fn member_inheritance_is_read() {
        let yes = util::parse_toml("[lints]\nworkspace = true\n", "m").unwrap();
        let dotted = util::parse_toml("lints.workspace = true\n", "m").unwrap();
        let no = util::parse_toml("[package]\nname = \"x\"\n", "m").unwrap();
        assert!(inherits_lints(&yes) && inherits_lints(&dotted) && !inherits_lints(&no));
    }
}
