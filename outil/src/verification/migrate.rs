//! One-way import of historical Markdown, with test identities supplied by its author.
//!
//! No source search or old report establishes that a test exists or passed. Cargo
//! must attest the supplied identities in a subsequent verification run.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::Path;

use serde::Deserialize;

use super::cargo::TestRef;
use super::model::{CatalogueDecision, Contract, DeclaredCheck, Guarantee};
use super::render;
use crate::items;
use crate::markdown;
use crate::report::{Level, Report};
use crate::util::{self, Result};

/// Explicit identities, not observations. A list detects duplicate legacy keys.
///
/// ```text
/// {"schema":1,"references":[{"legacy":"old_test","test":{
///   "package":"game-core","target_kind":"lib","target":"game_core",
///   "name":"tests::old_test"}}]}
/// ```
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct References {
    schema: u32,
    references: Vec<Reference>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    legacy: String,
    test: TestRef,
    #[serde(default)]
    reason: String,
}

struct Migration {
    contract: Contract,
    markdown: String,
}

/// Imports once, retaining the original Markdown as a recovery file.
pub fn run(dir: &Path, references_path: &Path) -> Result<usize> {
    let contract_path = dir.join("verification.toml");
    let archive_path = dir.join("CERTIFICATION.legacy.md");
    for path in [&contract_path, &archive_path] {
        if path.exists() {
            return Err(format!(
                "{} existe déjà : migration refusée sans écrasement",
                path.display()
            ));
        }
    }
    let source_path = dir.join("CERTIFICATION.md");
    let original = util::read(&source_path)?;
    let references = util::read(references_path)?;
    let migration = convert(&original, &references)?;
    let contract =
        toml::to_string_pretty(&migration.contract).map_err(|error| error.to_string())?;
    create_new(&archive_path, &original)?;
    create_new(&contract_path, &contract)?;
    util::write(&source_path, &migration.markdown)?;
    Ok(migration.contract.guarantees.len())
}

fn create_new(path: &Path, text: &str) -> Result<()> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .map_err(|error| format!("{} : {error}", path.display()))
}

fn convert(text: &str, references: &str) -> Result<Migration> {
    if text.contains(render::START) || text.contains(render::END) {
        return Err(
            "CERTIFICATION.md contient déjà une vue structurée ; migration refusée".to_owned(),
        );
    }
    let references = reference_map(references)?;
    let mut report = Report::quiet();
    let parsed = items::lint(text, "CERTIFICATION.md", &mut report);
    let errors = report
        .lines
        .iter()
        .filter(|(level, _)| *level == Level::Error)
        .map(|(_, message)| message.as_str())
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    let lines = markdown::lines(text);
    let mut used = BTreeSet::new();
    let mut checks = BTreeMap::<TestRef, String>::new();
    let mut contract = Contract {
        schema: 1,
        guarantees: Vec::new(),
        checks: Vec::new(),
        catalogue_decisions: Vec::new(),
    };
    for item in &parsed.items {
        let line = lines
            .get(item.line.saturating_sub(1))
            .ok_or_else(|| format!("CERTIFICATION.md:{} : ligne absente", item.line))?;
        import_item(
            item,
            line,
            &references,
            &mut contract,
            &mut checks,
            &mut used,
        )?;
    }
    let extra = references
        .keys()
        .filter(|key| !used.contains(*key))
        .cloned()
        .collect::<Vec<_>>();
    if !extra.is_empty() {
        return Err(format!(
            "références JSON inutilisées ou ambiguës : {}",
            extra.join(", ")
        ));
    }
    for (line, id) in &parsed.excluded {
        let source = lines
            .get(line.saturating_sub(1))
            .ok_or("exclusion absente")?;
        let (_, reason) = source
            .text
            .split_once(':')
            .ok_or("raison d'exclusion absente")?;
        if contract
            .catalogue_decisions
            .iter()
            .any(|decision| decision.id == *id)
        {
            return Err(format!(
                "{id} est adopté et écarté, ou écarté plusieurs fois ; décision à clarifier avant migration"
            ));
        }
        contract.catalogue_decisions.push(CatalogueDecision {
            id: id.clone(),
            status: "not-applicable".to_owned(),
            reason: reason.trim().to_owned(),
        });
    }
    let markdown = migrate_view(text, &parsed, &contract)?;
    Ok(Migration { contract, markdown })
}

fn import_item(
    item: &items::Item,
    line: &markdown::Line<'_>,
    references: &BTreeMap<String, Reference>,
    contract: &mut Contract,
    checks: &mut BTreeMap<TestRef, String>,
    used: &mut BTreeSet<String>,
) -> Result<()> {
    let cells = markdown::cells(line.text);
    let claim = cells.get(1).ok_or("exigence absente")?.replace("\\|", "|");
    let original_tests = markdown::code_spans(cells.get(2).ok_or("tests absents")?);
    if !original_tests
        .iter()
        .map(|name| name.rsplit("::").next().unwrap_or(name).trim())
        .eq(item.tests.iter().map(String::as_str))
    {
        return Err(format!(
            "{} : références Markdown incohérentes avec leur lecture historique",
            item.id
        ));
    }
    let mut guarantee = Guarantee {
        id: item.id.clone(),
        claim,
        checks: Vec::new(),
        origins: item.origin.iter().cloned().collect(),
        assumptions: Vec::new(),
        retired: item.retired,
    };
    for legacy in original_tests {
        if item.retired {
            guarantee
                .assumptions
                .push(format!("Référence historique retirée : {legacy}."));
            continue;
        }
        let reference = references.get(legacy).ok_or_else(|| format!(
            "{} : référence `{legacy}` non résolue ; fournir son paquet, sa cible Cargo et son nom complet dans le fichier JSON de références (aucune recherche de fonction ne fait autorité)", item.id
        ))?;
        used.insert(legacy.to_owned());
        let test = reference.test.canonicalized();
        let id = if let Some(id) = checks.get(&test) {
            id.clone()
        } else {
            let id = format!("test-{}", checks.len().saturating_add(1));
            checks.insert(test.clone(), id.clone());
            contract.checks.push(DeclaredCheck {
                id: id.clone(),
                kind: "cargo-test".to_owned(),
                test,
            });
            id
        };
        if !guarantee.checks.contains(&id) {
            guarantee.checks.push(id);
        }
        if !reference.reason.trim().is_empty() {
            guarantee.assumptions.push(format!(
                "Référence historique {legacy} vers {} : {}",
                reference.test.name,
                reference.reason.trim()
            ));
        }
    }
    for origin in &guarantee.origins {
        add_adoption(&mut contract.catalogue_decisions, origin, &guarantee)?;
    }
    contract.guarantees.push(guarantee);
    Ok(())
}

fn reference_map(text: &str) -> Result<BTreeMap<String, Reference>> {
    let references: References = serde_json::from_str(text)
        .map_err(|error| format!("références de migration JSON : {error}"))?;
    if references.schema != 1 {
        return Err(format!(
            "références de migration : schema {} inconnu",
            references.schema
        ));
    }
    let mut map = BTreeMap::new();
    for reference in references.references {
        let legacy = reference.legacy.trim();
        let test_ref = &reference.test;
        if legacy.is_empty()
            || !legacy.split("::").all(markdown::is_ident)
            || test_ref.package.trim().is_empty()
            || test_ref.target.trim().is_empty()
            || !matches!(test_ref.target_kind.as_str(), "lib" | "bin" | "test")
            || !test_ref.name.split("::").all(markdown::is_ident)
            || test_ref.profile.trim().is_empty()
        {
            return Err(format!(
                "référence `{legacy}` : identité de test incomplète ou invalide"
            ));
        }
        let original_name = legacy.rsplit("::").next().unwrap_or(legacy);
        let new_name = test_ref.name.rsplit("::").next().unwrap_or(&test_ref.name);
        let redirected =
            original_name != new_name || (legacy.contains("::") && legacy != test_ref.name);
        if redirected && reference.reason.trim().is_empty() {
            return Err(format!(
                "référence `{legacy}` redirigée vers `{}` : reason obligatoire pour conserver le rôle du helper ou expliquer le changement",
                test_ref.name
            ));
        }
        let key = legacy.to_owned();
        if map.insert(key.clone(), reference).is_some() {
            return Err(format!(
                "référence `{key}` dupliquée : fournir une identité exacte et non ambiguë"
            ));
        }
    }
    Ok(map)
}

fn add_adoption(
    decisions: &mut Vec<CatalogueDecision>,
    origin: &str,
    guarantee: &Guarantee,
) -> Result<()> {
    if let Some(decision) = decisions.iter_mut().find(|decision| decision.id == origin) {
        if decision.status != "adopted" {
            return Err(format!("{origin} : décision contradictoire"));
        }
        let _: std::fmt::Result =
            write!(decision.reason, " {} : {}", guarantee.id, guarantee.claim);
    } else {
        decisions.push(CatalogueDecision {
            id: origin.to_owned(),
            status: "adopted".to_owned(),
            reason: format!("Adapté par {} : {}", guarantee.id, guarantee.claim),
        });
    }
    Ok(())
}

fn migrate_view(text: &str, parsed: &items::Parsed, contract: &Contract) -> Result<String> {
    let lines = markdown::lines(text);
    let heading = lines
        .iter()
        .position(|line| line.read() && line.text.trim_end() == items::ITEMS)
        .ok_or("section des items absente")?;
    let mut remove = parsed
        .items
        .iter()
        .map(|item| item.line)
        .collect::<BTreeSet<_>>();
    remove.extend(parsed.excluded.iter().map(|(line, _)| *line));
    // The old parser has checked the single declaration table. Only that table
    // and the source headings are replaced; surrounding prose stays readable.
    for line in lines.iter().skip(heading.saturating_add(1)) {
        if line.read() && (line.text.starts_with("## ") || line.text.starts_with("# ")) {
            break;
        }
        if line.read() && line.text.trim_start().starts_with('|') {
            remove.insert(line.number);
        }
    }
    let mut out = String::new();
    for (index, line) in lines.iter().enumerate() {
        if index == heading {
            out.push_str("Les engagements ont leur source dans `verification.toml`. Le bloc suivant est une vue générée (`soiree contracts --write-view`). Les identités fournies à la migration restent à attester par Cargo ; aucun résultat historique n'est importé.\n\n");
            out.push_str(&render::guarantees(contract));
            out.push('\n');
        } else if !(remove.contains(&line.number)
            || line.read() && line.text.trim_end() == items::EXCLUDED)
        {
            out.push_str(line.text);
            out.push('\n');
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "# Jeu\n\nIntroduction conservée.\n\n```markdown\n## Les items\n| J99 | exemple | `fake` | |\n```\n\n## Les items\n\n| # | Exigence | Test | Origine |\n|---|---|---|---|\n| J1 | Un vote reste unique. | `unique_vote`, `counts` | `VOTE-01` |\n| J2 | Le total est borné. | `counts` | |\n| ~~J3~~ | Ancienne règle. | `old_rule` | |\n\n## Interface\n\nIME et appareils réels restent à observer.\n\n## Items du catalogue écartés\n\n- VOTE-02 : aucun vote secret.\n\nConclusion conservée.\n";

    fn references() -> String {
        serde_json::json!({"schema":1,"references":[
            {"legacy":"unique_vote","test":{"package":"game-core","target_kind":"lib","target":"game_core","name":"tests::unique_vote"}},
            {"legacy":"counts","test":{"package":"game-core","target_kind":"test","target":"sessions","name":"sessions::totals_hold"},"reason":"counts est un helper exercé après chaque étape de cette session."}
        ]}).to_string()
    }

    #[test]
    fn imports_every_commitment_and_reason_without_importing_evidence() -> Result<()> {
        let migration = convert(SOURCE, &references())?;
        assert_eq!(migration.contract.guarantees.len(), 3);
        assert_eq!(migration.contract.checks.len(), 2);
        assert_eq!(
            migration
                .contract
                .guarantees
                .first()
                .map(|g| g.claim.as_str()),
            Some("Un vote reste unique.")
        );
        assert!(
            migration
                .contract
                .guarantees
                .last()
                .is_some_and(|g| g.retired && g.checks.is_empty())
        );
        assert!(
            migration
                .contract
                .guarantees
                .iter()
                .any(|g| g.assumptions.iter().any(|a| a.contains("helper exercé")))
        );
        assert_eq!(migration.contract.catalogue_decisions.len(), 2);
        assert!(
            migration
                .contract
                .catalogue_decisions
                .iter()
                .any(|d| d.id == "VOTE-02" && d.reason == "aucun vote secret.")
        );
        for prose in [
            "Introduction conservée.",
            "IME et appareils réels restent à observer.",
            "Conclusion conservée.",
            "`fake`",
        ] {
            assert!(migration.markdown.contains(prose));
        }
        assert!(
            migration
                .markdown
                .contains(&render::guarantees(&migration.contract))
        );
        let encoded = toml::to_string_pretty(&migration.contract).map_err(|e| e.to_string())?;
        let decoded: Contract = toml::from_str(&encoded).map_err(|e| e.to_string())?;
        assert_eq!(decoded.guarantees.len(), 3);
        Ok(())
    }

    #[test]
    fn refuses_missing_duplicate_and_unexplained_helper_references() {
        assert!(convert(SOURCE, r#"{"schema":1,"references":[]}"#).is_err());
        let duplicate = references().replace("\"legacy\":\"counts\"", "\"legacy\":\"unique_vote\"");
        assert!(convert(SOURCE, &duplicate).is_err());
        let unexplained = references().replace(
            "counts est un helper exercé après chaque étape de cette session.",
            "",
        );
        assert!(convert(SOURCE, &unexplained).is_err());
    }

    #[test]
    fn conflicting_catalogue_decisions_and_current_views_require_review() {
        let conflict = SOURCE.replace("- VOTE-02 :", "- VOTE-01 :");
        assert!(convert(&conflict, &references()).is_err());
        let migrated = format!("{SOURCE}\n{}", render::START);
        assert!(convert(&migrated, &references()).is_err());
    }
}
