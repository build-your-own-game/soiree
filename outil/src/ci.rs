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

#[cfg(test)]
mod tests {
    use super::*;

    /// The events of a workflow, as written under its top-level `on:`.
    fn triggers(workflow: &str) -> Vec<&str> {
        workflow
            .lines()
            .skip_while(|line| *line != "on:")
            .skip(1)
            .take_while(|line| line.starts_with(' '))
            .map(str::trim)
            .filter(|line| !line.starts_with('#'))
            .collect()
    }

    #[test]
    fn a_game_spends_macos_minutes_only_on_demand_or_on_a_pull_request() {
        let workflow = workflow().unwrap();
        assert_eq!(triggers(&workflow), ["workflow_dispatch:", "pull_request:"]);
        assert!(!workflow.contains("push"));
    }

    #[test]
    fn a_workflow_that_runs_on_every_push_is_refused() {
        let dir = crate::testdir::fresh("ci-trigger");
        write(&dir).unwrap();
        let mut report = Report::new();
        check(&dir, &mut report);
        assert_eq!(report.count(crate::report::Level::Error), 0);
        let path = dir.join(PATH);
        let text = util::read(&path).unwrap();
        let pushed = text.replace("  pull_request:", "  pull_request:\n  push:");
        assert_ne!(pushed, text);
        util::write(&path, &pushed).unwrap();
        let mut report = Report::new();
        check(&dir, &mut report);
        assert_eq!(report.count(crate::report::Level::Error), 1);
    }
}
