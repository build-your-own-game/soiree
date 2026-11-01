//! What the common repository's documents say about the code, read against
//! the code. A fact copied by hand starts lying the day the code moves, and
//! here a false sentence becomes false code in every agent that reads it.
//! This reads facts, not meaning: a sentence can pass and still be wrong.

use std::collections::BTreeSet;
use std::path::Path;

use crate::embedded::VERSION;
use crate::report::Report;
use crate::util;
use crate::verification::resolve;

/// What an agent reads. `CHANGELOG.md`, `LEARNINGS.md` and archives are
/// memory: they may name what no longer exists.
const DOCS: [&str; 17] = [
    "AGENTS.md",
    "CERTIFICATION.md",
    "MANIFESTE.md",
    "README.md",
    "REJOINDRE.md",
    "VERIFICATION.md",
    "catalogue.toml",
    "certifications/README.md",
    "modele/AGENTS.md",
    "modele/CERTIFICATION.md",
    "modele/README.md",
    "outil/dossier/AGENTS.md",
    "remote/AGENTS.md",
    "remote/CONTRAT.md",
    "remote/README.md",
    "salon/AGENTS.md",
    "salon/README.md",
];

/// A document may name these before they exist: runs and `soiree new` write
/// them, and the host's folder has its games beside this repository.
const WRITTEN_LATER: [&str; 6] = [
    ".github/workflows/soiree.yml",
    "certification/",
    "data/",
    "dist/",
    "games/",
    "target/",
];

/// Where a path cited by a document may live. The model's documents speak
/// from inside a future game, and every game has `.soiree/`.
const FROM: [&str; 5] = ["", "modele", "outil", "remote", "salon"];

const HARNESS_FILE: &str = "tools/cert/cert.mjs";
const SCENARIOS_LABEL: &str = "Les scénarios, dans l'ordre :";
const CERT_JSON_HEADING: &str = "## Le fichier `cert.json`";
const NOT_YET_BOUND: &str = "à relier";

/// Options of the harness itself, after `--`.
const HARNESS_OPTIONS: [&str; 3] = ["--only", "--seed", "--soak"];

pub fn check(dir: &Path, report: &mut Report) {
    let before = report.lines.len();
    for doc in DOCS {
        match util::read(&dir.join(doc)) {
            Ok(text) => {
                for error in versions(doc, &text, VERSION) {
                    report.error(error);
                }
                for error in commands(doc, &text, crate::HELP) {
                    report.error(error);
                }
                paths(dir, doc, &text, report);
            }
            Err(error) => report.error(error),
        }
    }
    harness_facts(dir, report);
    if report.lines.len() == before {
        report.ok("documents du commun : versions, commandes, chemins, scénarios, clés et items conformes au code");
    }
}

fn harness_facts(dir: &Path, report: &mut Report) {
    let certification = util::read(&dir.join("CERTIFICATION.md"));
    let harness = util::read(&dir.join(HARNESS_FILE));
    let (Ok(certification), Ok(harness)) = (certification, harness) else {
        report.error(format!("CERTIFICATION.md ou {HARNESS_FILE} illisible"));
        return;
    };
    let run = scenarios_in_harness(&harness);
    for error in scenarios(&certification, &run) {
        report.error(error);
    }
    for error in items(&certification, &run, resolve::HARNESS) {
        report.error(error);
    }
    match harness_keys(dir) {
        Ok(keys) => {
            for error in cert_json_keys(&certification, &keys) {
                report.error(error);
            }
        }
        Err(error) => report.error(error),
    }
}

/// A version in a document is this tool's own. An example writes `vX.Y.Z`.
fn versions(doc: &str, text: &str, version: &str) -> Vec<String> {
    let mut errors = Vec::new();
    for (index, _) in text.match_indices('v') {
        let before = text.get(..index).and_then(|t| t.chars().next_back());
        if before.is_some_and(char::is_alphanumeric) {
            continue;
        }
        let rest = text.get(index.saturating_add(1)..).unwrap_or_default();
        let cited: String = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        let cited = cited.trim_end_matches('.');
        let parts: Vec<&str> = cited.split('.').collect();
        let numbered = parts.len() == 3 && parts.iter().all(|p| !p.is_empty());
        if numbered && cited != version {
            errors.push(format!(
                "{doc} cite v{cited}, alors que le commun est à v{version} : écris vX.Y.Z, ou renvoie à soiree.toml"
            ));
        }
    }
    errors
}

/// Commands and their options, as the tool's own help gives them.
fn known_commands(help: &str) -> Vec<(String, BTreeSet<String>)> {
    let mut known = vec![("help".to_owned(), BTreeSet::new())];
    let mut lines = help.lines().peekable();
    while let Some(line) = lines.next() {
        let Some(rest) = line.trim_start().strip_prefix("soiree ") else {
            continue;
        };
        if !line.starts_with(' ') {
            continue;
        }
        let Some(name) = rest.split_whitespace().next() else {
            continue;
        };
        let mut usage = rest.to_owned();
        // A long usage continues on the next, more indented lines.
        while let Some(next) = lines.peek() {
            if next.trim_start().starts_with('[') {
                usage.push(' ');
                usage.push_str(next);
                lines.next();
            } else {
                break;
            }
        }
        known.push((name.to_owned(), options_in(&usage)));
    }
    known
}

fn options_in(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .filter(|word| word.len() > 2 && word.starts_with("--"))
        .map(str::to_owned)
        .collect()
}

/// The code in a document: fenced lines, and spans between backticks.
fn code_in(text: &str) -> Vec<String> {
    let mut code = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        } else if fenced {
            code.push(line.to_owned());
        } else {
            code.extend(line.split('`').skip(1).step_by(2).map(str::to_owned));
        }
    }
    code
}

fn commands(doc: &str, text: &str, help: &str) -> Vec<String> {
    let known = known_commands(help);
    let mut errors = Vec::new();
    for snippet in code_in(text) {
        for (start, _) in snippet.match_indices("soiree ") {
            let before = snippet.get(..start).and_then(|t| t.chars().next_back());
            if before.is_some_and(|c| !c.is_whitespace() && c != '(') {
                continue;
            }
            let rest = snippet.get(start.saturating_add(7)..).unwrap_or_default();
            let call = rest
                .split(['#', ';', '&', '|', ')'])
                .next()
                .unwrap_or_default();
            let mut tokens = call.split_whitespace();
            let Some(name) = tokens.next() else { continue };
            if !name.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
                continue;
            }
            let Some((_, options)) = known.iter().find(|(known, _)| known == name) else {
                errors.push(format!(
                    "{doc} cite `soiree {name}`, une commande qui n'existe pas"
                ));
                continue;
            };
            let mut harness = false;
            for token in tokens {
                if token == "--" {
                    harness = true;
                } else if token.starts_with("--") && token != "--help" {
                    let accepted = if harness {
                        HARNESS_OPTIONS.contains(&token)
                    } else {
                        options.contains(token)
                    };
                    if !accepted {
                        errors.push(format!(
                            "{doc} cite `soiree {name} {token}`, une option qui n'existe pas"
                        ));
                    }
                }
            }
        }
    }
    errors
}

fn paths(dir: &Path, doc: &str, text: &str, report: &mut Report) {
    let parent = Path::new(doc).parent().unwrap_or(Path::new(""));
    for path in crate::check::cited_paths(text) {
        let local = path
            .strip_prefix(".soiree/")
            .or_else(|| path.strip_prefix("soiree/"))
            .unwrap_or(&path);
        let found = WRITTEN_LATER.iter().any(|p| local.starts_with(p))
            || dir.join(parent).join(local).exists()
            || FROM.iter().any(|from| dir.join(from).join(local).exists());
        if !found {
            report.error(format!("{doc} cite `{path}`, qui n'existe pas"));
        }
    }
}

/// The list `const ALL = [...]` of the harness, in its order.
fn scenarios_in_harness(harness: &str) -> Vec<String> {
    let Some((_, rest)) = harness.split_once("const ALL = [") else {
        return Vec::new();
    };
    let list = rest.split(']').next().unwrap_or_default();
    list.split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

fn scenarios(certification: &str, run: &[String]) -> Vec<String> {
    let Some((_, rest)) = certification.split_once(SCENARIOS_LABEL) else {
        return vec![format!(
            "CERTIFICATION.md : phrase « {SCENARIOS_LABEL} » absente"
        )];
    };
    let sentence = rest.split(". ").next().unwrap_or_default();
    let cited: Vec<String> = sentence
        .split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect();
    if cited == run {
        Vec::new()
    } else {
        vec![format!(
            "CERTIFICATION.md : les scénarios cités ({}) diffèrent de ceux du harnais ({})",
            cited.join(", "),
            run.join(", ")
        )]
    }
}

/// Each item says who verifies it. A scenario it names really reports that
/// item, and an item nothing runs says so.
fn items(certification: &str, run: &[String], harness: &[(&str, &[&str])]) -> Vec<String> {
    let mut errors = Vec::new();
    for line in certification.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        let (Some(id), Some(last)) = (cells.get(1), cells.iter().rev().nth(1)) else {
            continue;
        };
        let identifier = id.chars().next().is_some_and(char::is_uppercase)
            && id.len() >= 2
            && id.chars().skip(1).all(|c| c.is_ascii_digit());
        if cells.len() < 5 || !identifier {
            continue;
        }
        let unbound = resolve::UNBOUND.iter().any(|(known, _, _)| known == id);
        let marked = last.contains(NOT_YET_BOUND);
        if unbound && !marked {
            errors.push(format!(
                "CERTIFICATION.md : {id} n'a aucun contrôle relié, sa colonne doit dire « {NOT_YET_BOUND} »"
            ));
        }
        if marked && !unbound {
            errors.push(format!(
                "CERTIFICATION.md : {id} dit « {NOT_YET_BOUND} », alors que l'outil le relie à un contrôle"
            ));
        }
        for scenario in last.split('`').skip(1).step_by(2) {
            if !run.iter().any(|known| known == scenario) {
                continue;
            }
            let reports = harness
                .iter()
                .any(|(name, ids)| *name == scenario && ids.contains(id));
            if !reports {
                errors.push(format!(
                    "CERTIFICATION.md : {id} cite le scénario `{scenario}`, qui ne rapporte pas cet item"
                ));
            }
        }
    }
    errors
}

/// The `cert.json` keys read by the harness, which attacks the game knowing
/// only this file.
fn harness_keys(dir: &Path) -> util::Result<BTreeSet<String>> {
    let folder = dir.join("tools/cert");
    let entries = std::fs::read_dir(&folder).map_err(|e| e.to_string())?;
    let mut keys = BTreeSet::new();
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path.to_string_lossy();
        if name.ends_with(".mjs") && !name.ends_with(".test.mjs") {
            keys.extend(config_keys(&util::read(&path)?));
        }
    }
    Ok(keys)
}

fn config_keys(source: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for (start, _) in source.match_indices("config") {
        let rest = source.get(start.saturating_add(6)..).unwrap_or_default();
        let before = source.get(..start).and_then(|t| t.chars().next_back());
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.') {
            continue;
        }
        let Some(key) = rest.strip_prefix("?.").or_else(|| rest.strip_prefix('.')) else {
            continue;
        };
        let key: String = key
            .chars()
            .take_while(|c| c.is_ascii_lowercase() || *c == '_')
            .collect();
        if !key.is_empty() {
            keys.insert(key);
        }
    }
    keys
}

fn cert_json_keys(certification: &str, keys: &BTreeSet<String>) -> Vec<String> {
    let Some((_, section)) = certification.split_once(CERT_JSON_HEADING) else {
        return vec![format!(
            "CERTIFICATION.md : section « {CERT_JSON_HEADING} » absente"
        )];
    };
    let section = section.split("\n## ").next().unwrap_or_default();
    keys.iter()
        .filter(|key| {
            !section.contains(&format!("**`{key}")) && !section.contains(&format!("\"{key}\""))
        })
        .map(|key| format!("CERTIFICATION.md : la clé `{key}` de cert.json n'est pas documentée"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Vec<String> = Vec::new();

    const HELP: &str = "soiree : l'outil\n\n  soiree check [--fast] [--locked] [dossier…]\n                                     règles\n  soiree certify [dossier] [--harness H] [-- options du harnais]\n  soiree dom <fichier.rs> [--html H…]\n                          [--css C…]\n";

    #[test]
    fn a_stale_version_is_refused_and_the_current_one_or_a_placeholder_passes() {
        assert_eq!(
            versions("D", "tag v0.3.0, ou vX.Y.Z et la v1", "0.3.0"),
            NONE
        );
        assert_eq!(versions("D", "revision v0.1.0", "0.3.0").len(), 1);
        assert_eq!(versions("D", "`v0.1.0`.", "0.3.0").len(), 1);
        assert_eq!(versions("D", "archiv0.1.0", "0.3.0"), NONE);
    }

    #[test]
    fn a_command_or_option_the_tool_does_not_have_is_refused() {
        assert_eq!(commands("D", "`soiree check --fast`", HELP), NONE);
        assert_eq!(
            commands("D", "`soiree certify -- --only ui,panic`", HELP),
            NONE
        );
        assert_eq!(
            commands("D", "```sh\nsoiree dom x.rs --css y\n```", HELP),
            NONE
        );
        assert!(
            commands("D", "`soiree play`", HELP)
                .first()
                .is_some_and(|e| e.contains("commande"))
        );
        assert!(
            commands("D", "`soiree check --inventee`", HELP)
                .first()
                .is_some_and(|e| e.contains("option"))
        );
        assert_eq!(commands("D", "`soiree certify -- --seul x`", HELP).len(), 1);
    }

    #[test]
    fn prose_and_install_lines_are_not_commands() {
        let line = "`cargo install --git https://github.com/x/soiree --tag vX.Y.Z soiree`";
        assert_eq!(commands("D", line, HELP), NONE);
        assert_eq!(
            commands("D", "soiree play, sans accent ni code", HELP),
            NONE
        );
        assert_eq!(commands("D", "`soiree <nom>` et `soiree.toml`", HELP), NONE);
    }

    #[test]
    fn the_help_lists_every_option_the_parsers_accept() {
        let sources = [
            include_str!("main.rs"),
            include_str!("dom.rs"),
            include_str!("mutants.rs"),
            include_str!("upgrade.rs"),
        ];
        for source in sources {
            for option in source.split('"').skip(1).step_by(2) {
                if option.starts_with("--") && option.len() > 2 {
                    let parsed = source.contains(&format!("\"{option}\" =>"));
                    let root = matches!(option, "--help" | "--version");
                    assert!(
                        !parsed || root || crate::HELP.contains(option),
                        "{option} absent de l'aide"
                    );
                }
            }
        }
    }

    #[test]
    fn a_missing_path_is_refused_and_a_run_output_is_not() {
        let dir = std::env::temp_dir().join(format!("soiree-docs-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("salon/crates")).unwrap();
        let mut report = Report::new();
        paths(
            &dir,
            "README.md",
            "`salon/crates`, `.soiree/salon/crates`, `certification/check.json`",
            &mut report,
        );
        assert!(report.lines.is_empty(), "{:?}", report.lines);
        paths(&dir, "README.md", "`certification-x/absent`", &mut report);
        assert_eq!(report.lines.len(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_documented_scenarios_are_the_harness_ones_in_its_order() {
        let run = scenarios_in_harness(r#"const ALL = ["join", "rtc"];"#);
        assert_eq!(run, ["join", "rtc"]);
        let doc = |list: &str| format!("{SCENARIOS_LABEL} {list}. La suite");
        assert_eq!(scenarios(&doc("`join`, `rtc`"), &run), NONE);
        assert_eq!(scenarios(&doc("`rtc`, `join`"), &run).len(), 1);
        assert_eq!(scenarios(&doc("`join`"), &run).len(), 1);
        assert_eq!(scenarios("rien", &run).len(), 1);
    }

    #[test]
    fn an_item_names_only_what_really_reports_it() {
        let run = ["monkey".to_owned(), "reload".to_owned()];
        let harness: &[(&str, &[&str])] = &[("reload", &["C4"]), ("monkey", &["E6"])];
        let row = |id: &str, by: &str| format!("| {id} | Exigence | Origine | {by} |");
        assert_eq!(items(&row("C4", "2 `reload`"), &run, harness), NONE);
        assert_eq!(items(&row("E6", "2 `reload`"), &run, harness).len(), 1);
        // D5 is unbound: citing a scenario is the lie, and so is saying nothing.
        assert_eq!(items(&row("D5", "à relier"), &run, harness), NONE);
        assert_eq!(items(&row("D5", "2 `monkey`"), &run, harness).len(), 2);
        assert_eq!(items(&row("C4", "à relier"), &run, harness).len(), 1);
        assert_eq!(
            items("| # | Exigence | Origine | Vérifié par |", &run, harness),
            NONE
        );
    }

    #[test]
    fn a_key_the_harness_reads_must_be_documented() {
        let keys = config_keys("a(config.phone, config?.text); myconfig.x; thisconfig.y");
        assert_eq!(keys, BTreeSet::from(["phone".into(), "text".into()]));
        let doc = format!(
            "{CERT_JSON_HEADING}\n\n- **`phone`** …\n\n```json\n{{ \"text\": 1 }}\n```\n\n## Après\n**`ui`**"
        );
        assert_eq!(cert_json_keys(&doc, &keys), NONE);
        let more = BTreeSet::from(["ui".to_owned()]);
        assert_eq!(cert_json_keys(&doc, &more).len(), 1);
        assert_eq!(cert_json_keys("rien", &keys).len(), 1);
    }
}
