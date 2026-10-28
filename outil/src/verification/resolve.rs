//! Pure resolution: a declaration can add obligations, never remove common minima.
use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::model::{Applicability, Contract, Control, ExecutionSpec, Method, Obligation, Plan};
use crate::embedded;

pub const HARNESS: &[(&str, &[&str])] = &[
    ("ui", &["U1"]),
    ("performance", &["P1", "P2"]),
    ("panic", &["C10"]),
    ("join", &["C1", "C8", "R3"]),
    ("layout", &["T3", "T4"]),
    ("storage", &["C9"]),
    ("cut", &["C2"]),
    ("freeze", &["C3"]),
    ("silent", &["C3"]),
    ("reload", &["C4"]),
    ("twin", &["C5"]),
    ("rtc", &["C1"]),
    ("hostile", &["E1", "E2", "E3", "E4"]),
    ("jitter", &["R1"]),
    ("monkey", &["E6", "T2", "D2", "P1"]),
    ("restart", &["C7"]),
    ("soak", &["R2"]),
];

pub fn harness_id(scenario: &str, item: &str) -> String {
    format!("harness:{scenario}:{item}")
}

#[derive(Clone, Copy, Debug)]
pub struct FoundationCapabilities {
    pub has_cargo: bool,
    pub has_ios: bool,
}

/// Common repositories and the catalogue use the same observations and policy,
/// without game declarations, browser scenarios or rehearsal obligations.
pub fn foundation(capabilities: FoundationCapabilities) -> Plan {
    let mut plan = empty_plan(Vec::new());
    common_controls(&mut plan, capabilities);
    plan
}

pub fn resolve(contract: &Contract, config: &Value) -> Plan {
    resolve_with_capabilities(
        contract,
        config,
        FoundationCapabilities {
            has_cargo: true,
            has_ios: false,
        },
    )
}

pub fn resolve_with_capabilities(
    contract: &Contract,
    config: &Value,
    capabilities: FoundationCapabilities,
) -> Plan {
    let mut plan = empty_plan(contract.catalogue_decisions.clone());
    if contract.schema != 1 {
        plan.diagnostics.push(format!(
            "verification.toml : schema {} non pris en charge",
            contract.schema
        ));
    }
    let checks = declared_checks(contract, &mut plan);
    declared_guarantees(contract, &checks, &mut plan);
    catalogue_decisions(contract, &mut plan);
    common_controls(&mut plan, capabilities);
    unbound_game_requirements(&mut plan);
    harness_health(&mut plan);
    harness_controls(config, &mut plan);
    rehearsal(&mut plan);
    plan
}

fn empty_plan(catalogue: Vec<super::model::CatalogueDecision>) -> Plan {
    Plan {
        schema: 1,
        policy_version: embedded::VERSION.to_owned(),
        obligations: Vec::new(),
        controls: Vec::new(),
        catalogue,
        diagnostics: Vec::new(),
    }
}

fn declared_checks(contract: &Contract, plan: &mut Plan) -> BTreeMap<String, String> {
    let mut seen = BTreeSet::new();
    let mut checks = BTreeMap::new();
    for check in &contract.checks {
        if check.id.trim().is_empty() || !seen.insert(check.id.clone()) {
            plan.diagnostics
                .push(format!("contrôle vide ou dupliqué : {}", check.id));
        }
        if check.kind != "cargo-test" {
            plan.diagnostics.push(format!(
                "contrôle {} : kind inconnu {}",
                check.id, check.kind
            ));
        }
        let id = format!("game:{}", check.id);
        checks.insert(check.id.clone(), id.clone());
        if !contract
            .guarantees
            .iter()
            .any(|g| !g.retired && g.checks.contains(&check.id))
        {
            continue;
        }
        plan.controls.push(Control {
            id: id.clone(),
            method: Method::Test,
            scope: format!("{:?}", check.test.canonicalized()),
            execution: ExecutionSpec::CargoTest {
                test: check.test.canonicalized(),
            },
        });
    }
    checks
}

fn declared_guarantees(contract: &Contract, checks: &BTreeMap<String, String>, plan: &mut Plan) {
    let mut seen = BTreeSet::new();
    for guarantee in &contract.guarantees {
        if !valid_game_id(&guarantee.id) || !seen.insert(guarantee.id.clone()) {
            plan.diagnostics.push(format!(
                "engagement invalide ou dupliqué : {}",
                guarantee.id
            ));
        }
        if guarantee.claim.trim().is_empty() {
            plan.diagnostics
                .push(format!("{} : exigence vide", guarantee.id));
        }
        if guarantee.retired {
            continue;
        }
        let mut controls = Vec::new();
        for check in &guarantee.checks {
            if let Some(id) = checks.get(check) {
                controls.push(id.clone());
            } else {
                plan.diagnostics
                    .push(format!("{} : contrôle inconnu {check}", guarantee.id));
            }
        }
        if controls.is_empty() {
            plan.diagnostics
                .push(format!("{} : aucun contrôle exécutable", guarantee.id));
        }
        plan.obligations.push(Obligation {
            id: guarantee.id.clone(),
            claim: guarantee.claim.clone(),
            applicability: Applicability::Applicable,
            controls,
            owner: "game".to_owned(),
            assumptions: guarantee.assumptions.clone(),
        });
    }
    for check in &contract.checks {
        if !contract
            .guarantees
            .iter()
            .any(|g| g.checks.contains(&check.id))
        {
            plan.diagnostics
                .push(format!("contrôle sans engagement : {}", check.id));
        }
    }
}

fn catalogue_decisions(contract: &Contract, plan: &mut Plan) {
    let mut seen = BTreeSet::new();
    let known_catalogue = crate::catalogue::ids();
    for guarantee in &contract.guarantees {
        for origin in &guarantee.origins {
            if !known_catalogue.contains(origin) {
                plan.diagnostics.push(format!(
                    "{} : origine absente du catalogue : {origin}",
                    guarantee.id
                ));
            }
        }
    }
    for decision in &contract.catalogue_decisions {
        if !seen.insert(decision.id.clone())
            || decision.reason.trim().is_empty()
            || !known_catalogue.contains(&decision.id)
            || !["adopted", "not-applicable", "unreviewed"].contains(&decision.status.as_str())
        {
            plan.diagnostics
                .push(format!("décision catalogue invalide : {}", decision.id));
        }
        let referenced = contract
            .guarantees
            .iter()
            .any(|g| !g.retired && g.origins.contains(&decision.id));
        if (decision.status == "adopted" && !referenced)
            || (decision.status == "not-applicable" && referenced)
        {
            plan.diagnostics.push(format!(
                "décision catalogue contradictoire : {}",
                decision.id
            ));
        }
    }
    for id in known_catalogue {
        if !seen.contains(&id) {
            plan.catalogue.push(super::model::CatalogueDecision {
                id,
                status: "unreviewed".to_owned(),
                reason: "Aucune décision explicite ; ni adopté, ni exclu".to_owned(),
            });
        }
    }
}

fn common_controls(plan: &mut Plan, capabilities: FoundationCapabilities) {
    for (id, claim) in [
        (
            "static:common",
            "Règles communes, versions, provenance et déclarations valides",
        ),
        ("command:fmt", "Le code respecte le format commun"),
        (
            "command:clippy-native",
            "Les cibles natives respectent les lints communs",
        ),
        (
            "command:clippy-wasm",
            "Les cibles WASM respectent les lints communs",
        ),
        (
            "cargo:suite",
            "La suite native ne contient aucun test en échec",
        ),
        (
            "command:doctests",
            "Les exemples Rust compilables passent leurs tests",
        ),
        (
            "command:web-contract",
            "Le contrat Swift/Rust/JavaScript est exercé par les vraies liaisons",
        ),
    ] {
        if id != "static:common"
            && (!capabilities.has_cargo || (id == "command:web-contract" && !capabilities.has_ios))
        {
            continue;
        }
        let execution = match id {
            "command:fmt" => command(&["fmt", "--check"]),
            "command:clippy-native" => {
                command(&["clippy", "--all-targets", "--", "-D", "warnings"])
            }
            "command:clippy-wasm" => command(&[
                "clippy",
                "--target",
                "wasm32-unknown-unknown",
                "--",
                "-D",
                "warnings",
            ]),
            "command:doctests" => command(&["test", "--workspace", "--doc"]),
            "command:web-contract" => command(&["xtask", "web-contract"]),
            _ => ExecutionSpec::Static {
                name: id.to_owned(),
            },
        };
        plan.controls.push(Control {
            id: id.to_owned(),
            method: if matches!(
                id,
                "cargo:suite" | "command:doctests" | "command:web-contract"
            ) {
                Method::Test
            } else {
                Method::Static
            },
            scope: id.to_owned(),
            execution,
        });
        plan.obligations.push(Obligation {
            id: id.to_owned(),
            claim: claim.to_owned(),
            applicability: Applicability::Applicable,
            controls: vec![id.to_owned()],
            owner: "common".to_owned(),
            assumptions: Vec::new(),
        });
    }
}

fn harness_health(plan: &mut Plan) {
    plan.controls.push(Control {
        id: "harness:health".to_owned(),
        method: Method::Test,
        scope: "harness:health".to_owned(),
        execution: ExecutionSpec::Static {
            name: "harness:health".to_owned(),
        },
    });
    plan.obligations.push(Obligation {
        id: "H1".to_owned(),
        claim: "Le contrôleur a terminé sans panne ni anomalie hors scénario".to_owned(),
        applicability: Applicability::Applicable,
        controls: vec!["harness:health".to_owned()],
        owner: "common".to_owned(),
        assumptions: vec![],
    });
}

fn unbound_game_requirements(plan: &mut Plan) {
    for (id, claim, condition) in [
        (
            "C6",
            "Un joueur parti pour de bon ne bloque pas la partie : chaque phase finit à son échéance.",
            Some("La présence de phases avec échéance et de joueurs pouvant partir dépend du jeu."),
        ),
        (
            "E5",
            "Une action hors de sa phase, ou d’un joueur qui n’est pas concerné, est refusée sans rien changer.",
            None,
        ),
        (
            "T1",
            "Chaque limite de longueur est appliquée par l’hôte, pas seulement par la page, et comptée de la même façon des deux côtés.",
            Some("Les champs de texte et la répartition page/hôte dépendent du jeu."),
        ),
        (
            "D1",
            "L’état initial, les contenus et versions, la graine et les entrées externes datées et ordonnées déterminent les états et effets logiques de la partie.",
            None,
        ),
        (
            "D3",
            "Les invariants du jeu tiennent après chaque pas. Le jeu les énumère dans son test.",
            None,
        ),
        (
            "D4",
            "Pour une simulation en temps réel, le hash de l’état est le même en natif et en WASM sur un replay.",
            Some(
                "La présence d’une simulation en temps réel et son exécution en natif et en WASM doivent être déclarées.",
            ),
        ),
        (
            "D5",
            "On relance une partie sans rien redémarrer.",
            Some(
                "Le sens d’une nouvelle partie dépend du jeu ; un éditeur continu ne possède pas nécessairement ce cycle.",
            ),
        ),
    ] {
        let control = format!("unbound:{id}");
        let reason = "Aucun contrôle relié explicitement à cette exigence commune ; les garanties J ne sont pas assimilées automatiquement.";
        plan.controls.push(Control {
            id: control.clone(),
            method: Method::Test,
            scope: control.clone(),
            execution: ExecutionSpec::Unbound {
                reason: reason.to_owned(),
            },
        });
        plan.obligations.push(Obligation {
            id: id.to_owned(),
            claim: claim.to_owned(),
            applicability: condition.map_or(Applicability::Applicable, |reason| {
                Applicability::Unknown(reason.to_owned())
            }),
            controls: vec![control],
            owner: "game".to_owned(),
            assumptions: vec![reason.to_owned()],
        });
    }
}

fn harness_controls(config: &Value, plan: &mut Plan) {
    let transport = config
        .get("phone_upstream")
        .and_then(Value::as_str)
        .unwrap_or("lan");
    if config.get("phone_upstream").is_some_and(|value| {
        value
            .as_str()
            .is_none_or(|mode| !["lan", "loopback"].contains(&mode))
    }) {
        plan.diagnostics
            .push("cert.json : phone_upstream attend lan ou loopback".to_owned());
    }
    let phone = config
        .get("phone")
        .is_some_and(|v| !v.is_null() && v != false);
    let network = config
        .get("socket")
        .is_some_and(|v| !v.is_null() && v != false);
    let mut obligations: BTreeMap<String, Obligation> = BTreeMap::new();
    for &(scenario, items) in HARNESS {
        for &item in items {
            let id = harness_id(scenario, item);
            let method = if item.starts_with('P') || item == "R2" {
                Method::Measure
            } else {
                Method::Test
            };
            plan.controls.push(Control {
                id: id.clone(),
                method,
                scope: format!("{id};phone_upstream={transport}"),
                execution: ExecutionSpec::Harness {
                    scenario: scenario.to_owned(),
                    item: item.to_owned(),
                },
            });
            let applicability = if ["C1", "C2", "C3", "C4", "C5", "C8", "C10", "T4", "R1", "R3"]
                .contains(&item)
                && !phone
            {
                Applicability::NotApplicable(
                    "cert.json ne déclare aucun écran téléphone".to_owned(),
                )
            } else if ["E1", "E2", "E3", "E4", "C7"].contains(&item) && !network {
                Applicability::NotApplicable(
                    "cert.json ne déclare aucune connexion permanente".to_owned(),
                )
            } else if transport == "loopback"
                && [
                    "C1", "C2", "C3", "C4", "C5", "C7", "C8", "E1", "E2", "E3", "E4", "R1", "R3",
                ]
                .contains(&item)
            {
                Applicability::Unknown(
                    "Passage limité au loopback : l’accès depuis le réseau local n’est pas exercé"
                        .to_owned(),
                )
            } else if item == "D2" && config.get("advance").is_none_or(Value::is_null) {
                Applicability::Unknown(
                    "La terminaison dépend du genre ; aucune progression observable n’est déclarée"
                        .to_owned(),
                )
            } else {
                Applicability::Applicable
            };
            let obligation = obligations.entry(item.to_owned()).or_insert_with(|| Obligation {
                id: item.to_owned(), claim: format!("Exigence commune {item} (.soiree/CERTIFICATION.md)"),
                applicability, controls: Vec::new(), owner: "common".to_owned(),
                assumptions: vec!["Observation sur les moteurs, paramètres et appareils du passage ; voir les limites de l’oracle".to_owned()],
            });
            obligation.controls.push(id);
        }
    }
    plan.obligations.extend(obligations.into_values());
}

fn rehearsal(plan: &mut Plan) {
    for n in 1..=10 {
        let id = format!("S{n}");
        plan.controls.push(Control {
            id: id.clone(),
            method: Method::Justification,
            scope: "rehearsal".to_owned(),
            execution: ExecutionSpec::Manual,
        });
        plan.obligations.push(Obligation {
            id: id.clone(),
            claim: format!("Répétition sur appareils réels : {id}"),
            applicability: Applicability::Applicable,
            controls: vec![id],
            owner: "host".to_owned(),
            assumptions: vec!["Un compte rendu explicite reste un jugement humain".to_owned()],
        });
    }
}

fn command(args: &[&str]) -> ExecutionSpec {
    ExecutionSpec::Command {
        program: "cargo".to_owned(),
        arguments: args.iter().map(|s| (*s).to_owned()).collect(),
    }
}

fn valid_game_id(id: &str) -> bool {
    id.strip_prefix('J')
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_catalogue_has_only_the_static_controller() {
        let plan = foundation(FoundationCapabilities {
            has_cargo: false,
            has_ios: false,
        });
        assert_eq!(plan.controls.len(), 1);
        assert_eq!(plan.obligations.len(), 1);
        assert!(
            plan.controls
                .first()
                .is_some_and(|control| control.id == "static:common")
        );
        assert!(plan.catalogue.is_empty() && plan.diagnostics.is_empty());
    }

    #[test]
    fn a_cargo_foundation_has_one_native_suite_and_no_game_or_browser_work() {
        let plan = foundation(FoundationCapabilities {
            has_cargo: true,
            has_ios: false,
        });
        assert_eq!(plan.controls.len(), 6);
        assert_eq!(
            plan.controls
                .iter()
                .filter(|control| control.id == "cargo:suite")
                .count(),
            1
        );
        assert!(plan.controls.iter().all(|control| matches!(
            control.execution,
            ExecutionSpec::Static { .. } | ExecutionSpec::Command { .. }
        )));
        assert!(
            plan.controls
                .iter()
                .all(|control| control.id != "harness:health")
        );
        // The typed native runner owns the suite. The only `cargo test`
        // command left alongside it is the separate doctest target.
        assert!(
            plan.controls
                .iter()
                .all(|control| match &control.execution {
                    ExecutionSpec::Command { arguments, .. }
                        if arguments.first().is_some_and(|arg| arg == "test") =>
                        arguments.iter().any(|arg| arg == "--doc"),
                    ExecutionSpec::Static { .. }
                    | ExecutionSpec::Command { .. }
                    | ExecutionSpec::CargoTest { .. }
                    | ExecutionSpec::Harness { .. }
                    | ExecutionSpec::Manual
                    | ExecutionSpec::Unbound { .. } => true,
                })
        );
    }

    #[test]
    fn ios_bindings_remain_an_applicable_test_on_every_platform() {
        let plan = foundation(FoundationCapabilities {
            has_cargo: true,
            has_ios: true,
        });
        let control = plan
            .controls
            .iter()
            .find(|control| control.id == "command:web-contract");
        assert!(control.is_some_and(|control| control.method == Method::Test
            && matches!(&control.execution, ExecutionSpec::Command { program, arguments }
                if program == "cargo" && arguments == &["xtask", "web-contract"])));
        assert!(
            plan.obligations
                .iter()
                .any(|obligation| obligation.id == "command:web-contract"
                    && obligation.applicability == Applicability::Applicable)
        );
    }

    #[test]
    fn unbound_game_requirements_stay_visible_without_failing_the_static_scope() {
        let mut plan = foundation(FoundationCapabilities {
            has_cargo: false,
            has_ios: false,
        });
        let control = plan.controls.first().unwrap().clone();
        unbound_game_requirements(&mut plan);
        let pending = plan
            .controls
            .iter()
            .filter(|control| matches!(control.execution, ExecutionSpec::Unbound { .. }))
            .count();
        assert_eq!(pending, 7);
        for obligation in plan
            .obligations
            .iter()
            .filter(|obligation| obligation.owner == "game")
        {
            assert_eq!(
                obligation.applicability == Applicability::Applicable,
                ["D1", "D3", "E5"].contains(&obligation.id.as_str())
            );
        }
        let observed = crate::verification::model::Observation {
            control_id: control.id.clone(),
            method: control.method,
            scope: control.scope,
            outcome: crate::verification::model::Outcome::Passed,
            input_identity: Some("current".to_owned()),
            detail: String::new(),
            command: vec![],
            artifacts: vec![],
        };
        let evaluation = crate::verification::evaluate::evaluate(
            &plan,
            &[observed],
            &BTreeSet::from([control.id]),
            Some("current"),
        );
        assert!(evaluation.selected_scope_satisfied);
        assert!(!evaluation.all_automated_obligations_satisfied);
        assert!(!evaluation.complete);
    }

    #[test]
    fn common_minima_cannot_be_replaced_by_a_game_declaration() {
        let contract = Contract {
            schema: 1,
            guarantees: vec![],
            checks: vec![],
            catalogue_decisions: vec![],
        };
        let plan = resolve(&contract, &serde_json::json!({}));
        assert!(
            plan.obligations
                .iter()
                .any(|o| o.id == "static:common" && o.applicability == Applicability::Applicable)
        );
        assert!(
            plan.obligations
                .iter()
                .any(|o| o.id == "D2" && matches!(o.applicability, Applicability::Unknown(_)))
        );
    }

    #[test]
    fn loopback_ui_observations_cannot_claim_local_network_coverage() {
        let contract = Contract {
            schema: 1,
            guarantees: vec![],
            checks: vec![],
            catalogue_decisions: vec![],
        };
        let config =
            serde_json::json!({"phone":"/play", "socket":"/socket", "phone_upstream":"loopback"});
        let plan = resolve(&contract, &config);
        assert_eq!(plan.diagnostics, Vec::<String>::new());
        assert!(
            plan.obligations
                .iter()
                .any(|o| o.id == "C1" && matches!(o.applicability, Applicability::Unknown(_)))
        );
        assert!(
            plan.obligations
                .iter()
                .any(|o| o.id == "C10" && o.applicability == Applicability::Applicable)
        );
        assert!(
            plan.controls
                .iter()
                .filter(|c| matches!(c.execution, ExecutionSpec::Harness { .. }))
                .all(|c| c.scope.contains("phone_upstream=loopback"))
        );
        for invalid in [serde_json::json!(true), serde_json::json!("invented")] {
            let mut bad = config.clone();
            bad["phone_upstream"] = invalid;
            assert_ne!(resolve(&contract, &bad).diagnostics, Vec::<String>::new());
        }
    }
}
