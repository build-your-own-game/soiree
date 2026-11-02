//! Pure resolution: a declaration can add obligations, never remove common minima.
use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::model::{
    Applicability, Binding, Contract, Control, Display, DisplayRole, ExecutionSpec, Method,
    Obligation, Plan, Progression,
};
use crate::embedded;

pub const HARNESS: &[(&str, &[&str])] = &[
    ("ui", &["U1"]),
    ("performance", &["P1", "P2"]),
    ("panic", &["C10"]),
    ("join", &["C1"]),
    ("capacity", &["C8", "R3"]),
    ("text", &["T2"]),
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
    ("progression", &["D2"]),
    ("monkey", &["E6", "P1"]),
    ("restart", &["C7"]),
    ("resources", &["R4"]),
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
    let bound = bound_requirements(contract, &checks, &mut plan);
    game_requirements(&bound, &mut plan);
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
            && !contract
                .bindings
                .iter()
                .any(|b| b.checks.contains(&check.id))
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
            && !contract
                .bindings
                .iter()
                .any(|b| b.checks.contains(&check.id))
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
            "La suite native a tourné, sans test en échec ; un test ignoré reste à juger",
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

/// Common requirements nothing common can run: each game owns their meaning.
/// They stay visible as not attested until a game binds its own checks to them
/// in `verification.toml`. `CERTIFICATION.md` marks these, and only these,
/// « à relier » (`docs.rs`).
pub const UNBOUND: [(&str, &str, Option<&str>); 7] = [
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
];

/// The valid bindings, by requirement. An invalid one binds nothing, and its
/// diagnostic fails the run: a typo never reads as a requirement under control.
fn bound_requirements<'a>(
    contract: &'a Contract,
    checks: &BTreeMap<String, String>,
    plan: &mut Plan,
) -> BTreeMap<&'a str, (&'a Binding, Vec<String>)> {
    let mut seen = BTreeSet::new();
    let mut bound = BTreeMap::new();
    for binding in &contract.bindings {
        let id = binding.requirement.as_str();
        if !UNBOUND.iter().any(|(known, _, _)| *known == id) {
            plan.diagnostics.push(format!(
                "liaison {id} : seules les exigences « à relier » du socle se relient ; un minimum commun garde ses propres contrôles"
            ));
            continue;
        }
        if !seen.insert(id) {
            plan.diagnostics
                .push(format!("liaison {id} : exigence reliée deux fois"));
            bound.remove(id);
            continue;
        }
        if let Some(controls) = bound_controls(binding, checks, plan) {
            bound.insert(id, (binding, controls));
        }
    }
    bound
}

fn bound_controls(
    binding: &Binding,
    checks: &BTreeMap<String, String>,
    plan: &mut Plan,
) -> Option<Vec<String>> {
    let id = &binding.requirement;
    if binding.checks.is_empty() {
        plan.diagnostics
            .push(format!("liaison {id} : aucun contrôle"));
        return None;
    }
    let mut controls = Vec::new();
    let mut valid = true;
    for check in &binding.checks {
        match checks.get(check) {
            Some(control) if controls.contains(control) => {
                plan.diagnostics
                    .push(format!("liaison {id} : contrôle répété {check}"));
                valid = false;
            }
            Some(control) => controls.push(control.clone()),
            None => {
                plan.diagnostics
                    .push(format!("liaison {id} : contrôle inconnu {check}"));
                valid = false;
            }
        }
    }
    valid.then_some(controls)
}

fn game_requirements(bound: &BTreeMap<&str, (&Binding, Vec<String>)>, plan: &mut Plan) {
    for (id, claim, condition) in UNBOUND {
        if let Some((binding, controls)) = bound.get(id) {
            // Binding a conditional requirement declares its condition met: it
            // applies, and its checks decide. It never becomes an exclusion.
            let mut assumptions = binding.assumptions.clone();
            assumptions.push("Liaison déclarée par le jeu : elle désigne ces contrôles, elle n’établit pas qu’ils couvrent toute l’exigence.".to_owned());
            plan.obligations.push(Obligation {
                id: id.to_owned(),
                claim: claim.to_owned(),
                applicability: Applicability::Applicable,
                controls: controls.clone(),
                owner: "game".to_owned(),
                assumptions,
            });
            continue;
        }
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

fn continuous_reason(config: &Value, plan: &mut Plan) -> Option<String> {
    config.get("progression").and_then(|value| match serde_json::from_value::<Progression>(value.clone()) {
        Ok(Progression::Continuous { reason }) if !reason.trim().is_empty()
            && ["start", "advance"].iter().all(|key| config.get(key).is_none_or(|value| value.is_null() || value == false)) => Some(reason),
        Ok(Progression::Finite { checks }) => {
            if checks.is_empty() || checks.iter().any(|check| {
                check.get("name").and_then(Value::as_str).is_none_or(|name| name.trim().is_empty())
                    || check.get("command").and_then(Value::as_array).is_none_or(|command| command.is_empty() || command.iter().any(|part| part.as_str().is_none()))
            }) {
                plan.diagnostics.push("cert.json.progression : finite exige checks avec name et command non vides".to_owned());
            }
            None
        },
        Ok(Progression::Continuous { .. }) => {
            plan.diagnostics.push("cert.json.progression : continuous exige une raison et aucune commande start/advance".to_owned());
            None
        }
        Err(error) => {
            plan.diagnostics.push(format!("cert.json.progression : {error}"));
            None
        }
    })
}

fn display_scope(config: &Value, plan: &mut Plan) -> Option<Display> {
    config.get("display").and_then(|value| {
        match serde_json::from_value::<Display>(value.clone()) {
            Ok(display)
                if display.role != DisplayRole::Desktop
                    || (!display.reason.trim().is_empty()
                        && ["phone", "socket", "start", "advance"].iter().all(|key| {
                            config.get(key).is_none_or(|value| value.is_null() || value == false)
                        })) =>
            {
                Some(display)
            }
            Ok(_) => {
                plan.diagnostics.push(
                    "cert.json.display : desktop exige une raison et un éditeur solo sans phone, socket, start ou advance".to_owned(),
                );
                None
            }
            Err(error) => {
                plan.diagnostics
                    .push(format!("cert.json.display : {error}"));
                None
            }
        }
    })
}

/// JavaScript's idea of a value: the harness skips a key that reads empty.
fn declared(config: &Value, key: &str) -> bool {
    config
        .get(key)
        .is_some_and(|value| !value.is_null() && value != false && value != "")
}

fn harness_controls(config: &Value, plan: &mut Plan) {
    let progression = continuous_reason(config, plan);
    let display = display_scope(config, plan);
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
    let phone = declared(config, "phone");
    let network = declared(config, "socket");
    let solo = display
        .as_ref()
        .is_some_and(|display| display.role == DisplayRole::Desktop);
    if phone && !network {
        plan.diagnostics.push(
            "cert.json : phone déclaré sans socket ; attendu le chemin du WebSocket de salon, sinon E1 à E4 et C7 ne seraient plus exercés".to_owned(),
        );
    }
    if !phone && !solo && config.get("display").is_none_or(Value::is_null) {
        plan.diagnostics.push(
            "cert.json : sans téléphone, attendu display {\"role\":\"desktop\",\"reason\":\"…\"} ; retirer phone n'exclut aucun item".to_owned(),
        );
    } else if !phone && !solo {
        plan.diagnostics.push(
            "cert.json : sans téléphone, display doit valoir role = desktop, avec sa raison"
                .to_owned(),
        );
    }
    let mut obligations: BTreeMap<String, Obligation> = BTreeMap::new();
    for &(scenario, items) in HARNESS {
        for &item in items {
            let id = harness_id(scenario, item);
            let method = if item.starts_with('P') || ["R2", "R4"].contains(&item) {
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
            let applicability = if item == "T3"
                && display
                    .as_ref()
                    .is_some_and(|display| display.role == DisplayRole::Desktop)
            {
                Applicability::NotApplicable(format!(
                    "Éditeur desktop : {}. Les cadres et textes restent vérifiés par T2.",
                    display
                        .as_ref()
                        .map_or("", |display| display.reason.as_str())
                ))
            } else if ["C1", "C2", "C3", "C4", "C5", "C8", "C10", "T4", "R1", "R3"].contains(&item)
                && !phone
                && solo
            {
                Applicability::NotApplicable(
                    "cert.json ne déclare aucun écran téléphone".to_owned(),
                )
            } else if ["E1", "E2", "E3", "E4", "C7"].contains(&item) && !network && solo {
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
            } else if item == "D2" && progression.is_some() {
                Applicability::NotApplicable(format!(
                    "Interaction continue, aucune fin de partie : {}",
                    progression.as_deref().unwrap_or_default()
                ))
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
    if let Some(obligation) = plan
        .obligations
        .iter_mut()
        .find(|obligation| obligation.id == "R1")
    {
        let id = "review:R1:playability".to_owned();
        plan.controls.push(Control {
            id: id.clone(),
            method: Method::Justification,
            scope: "Ressenti de jeu de bout en bout sur les appareils et le réseau réels"
                .to_owned(),
            execution: ExecutionSpec::Manual,
        });
        obligation.controls.push(id);
        obligation.assumptions.push("Le scénario jitter établit la causalité fonctionnelle et la continuité ; il n'établit pas le ressenti physique.".to_owned());
    }
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
    fn functional_jitter_does_not_claim_to_measure_physical_playability() {
        let mut plan = empty_plan(vec![]);
        harness_controls(
            &serde_json::json!({"phone": "/play", "socket": "/ws"}),
            &mut plan,
        );
        rehearsal(&mut plan);
        plan.obligations.retain(|obligation| obligation.id == "R1");
        let ids = plan.obligations.first().unwrap().controls.clone();
        plan.controls.retain(|control| ids.contains(&control.id));
        let control = plan
            .controls
            .iter()
            .find(|control| matches!(control.execution, ExecutionSpec::Harness { .. }))
            .unwrap();
        let observation = crate::verification::model::Observation {
            control_id: control.id.clone(),
            method: control.method,
            scope: control.scope.clone(),
            outcome: crate::verification::model::Outcome::Passed,
            input_identity: Some("current".into()),
            detail: "causal action and connectivity".into(),
            command: vec![],
            artifacts: vec![],
        };
        let selected = BTreeSet::from([control.id.clone()]);
        let evaluation = crate::verification::evaluate::evaluate(
            &plan,
            &[observation],
            &selected,
            Some("current"),
        );
        assert!(evaluation.selected_scope_satisfied);
        assert!(evaluation.all_automated_obligations_satisfied);
        assert!(!evaluation.complete);
        assert_eq!(
            evaluation.obligations[0].assessment,
            crate::verification::model::Assessment::Pending
        );
    }

    #[test]
    fn a_continuous_interaction_does_not_claim_to_test_a_finite_game() {
        let mut plan = empty_plan(vec![]);
        harness_controls(&serde_json::json!({}), &mut plan);
        assert!(matches!(
            plan.obligations
                .iter()
                .find(|o| o.id == "D2")
                .unwrap()
                .applicability,
            Applicability::Unknown(_)
        ));
        let declaration = serde_json::json!({"phone": "/play", "socket": "/ws", "progression": {"kind": "continuous", "reason": "A shared tap counter, with no rounds."}});
        let mut plan = empty_plan(vec![]);
        harness_controls(&declaration, &mut plan);
        assert_eq!(plan.diagnostics, Vec::<String>::new());
        assert!(matches!(
            plan.obligations
                .iter()
                .find(|o| o.id == "D2")
                .unwrap()
                .applicability,
            Applicability::NotApplicable(_)
        ));
        assert_eq!(
            plan.obligations
                .iter()
                .find(|o| o.id == "P1")
                .unwrap()
                .applicability,
            Applicability::Applicable
        );
        for bad in [
            serde_json::json!({"progression": {"kind": "continuous", "reason": ""}}),
            serde_json::json!({"progression": {"kind": "continuous", "reason": "finite"}, "advance": {"key": " "}}),
            serde_json::json!({"progression": {"kind": "continuous", "reason": "finite"}, "start": {"key": "Enter"}}),
            serde_json::json!({"progression": {"kind": "continous", "reason": "typo"}}),
        ] {
            let mut plan = empty_plan(vec![]);
            harness_controls(&bad, &mut plan);
            assert!(!plan.diagnostics.is_empty(), "{bad}");
            assert!(!matches!(
                plan.obligations
                    .iter()
                    .find(|o| o.id == "D2")
                    .unwrap()
                    .applicability,
                Applicability::NotApplicable(_)
            ));
        }
    }

    #[test]
    fn desktop_scope_requires_the_editor_domain_and_keeps_text_checks() {
        let mut plan = empty_plan(vec![]);
        harness_controls(&serde_json::json!({"layout": false}), &mut plan);
        assert_eq!(
            plan.obligations
                .iter()
                .find(|o| o.id == "T3")
                .unwrap()
                .applicability,
            Applicability::Applicable
        );

        let desktop = serde_json::json!({"layout": false, "phone": null, "display": {"role": "desktop", "reason": "Scrollable local editor; its text frames are tested."}});
        let mut plan = empty_plan(vec![]);
        harness_controls(&desktop, &mut plan);
        assert_eq!(plan.diagnostics, Vec::<String>::new());
        assert!(matches!(
            plan.obligations
                .iter()
                .find(|o| o.id == "T3")
                .unwrap()
                .applicability,
            Applicability::NotApplicable(_)
        ));
        assert_eq!(
            plan.obligations
                .iter()
                .find(|o| o.id == "T2")
                .unwrap()
                .applicability,
            Applicability::Applicable
        );
        assert!(plan.controls.iter().any(|c| c.id == "harness:text:T2"));

        for bad in [
            serde_json::json!({"display": {"role": "desktop", "reason": ""}}),
            serde_json::json!({"display": {"role": "desktpo", "reason": "typo"}}),
            serde_json::json!({"display": {"role": "desktop", "reason": "has phones"}, "phone": "/play"}),
            serde_json::json!({"display": {"role": "desktop", "reason": "has transport"}, "socket": "/ws"}),
            serde_json::json!({"display": {"role": "desktop", "reason": "has phases"}, "advance": {"key": " "}}),
        ] {
            let mut plan = empty_plan(vec![]);
            harness_controls(&bad, &mut plan);
            assert!(!plan.diagnostics.is_empty(), "{bad}");
            assert_eq!(
                plan.obligations
                    .iter()
                    .find(|o| o.id == "T3")
                    .unwrap()
                    .applicability,
                Applicability::Applicable
            );
        }
    }

    fn applicability_of(config: &Value, id: &str) -> (Applicability, Vec<String>) {
        let mut plan = empty_plan(vec![]);
        harness_controls(config, &mut plan);
        let applicability = obligation(&plan, id).applicability.clone();
        (applicability, plan.diagnostics)
    }

    #[test]
    fn a_phone_needs_its_socket_and_keeps_every_network_item() {
        for config in [
            serde_json::json!({"phone": "/play"}),
            serde_json::json!({"phone": "/play", "socket": null}),
            serde_json::json!({"phone": "/play", "socket": false}),
            serde_json::json!({"phone": "/play", "socket": ""}),
        ] {
            for id in ["E1", "E2", "E3", "E4", "C7"] {
                let (applicability, diagnostics) = applicability_of(&config, id);
                assert_eq!(applicability, Applicability::Applicable, "{config} {id}");
                assert!(
                    diagnostics.iter().any(|d| d.contains("socket")),
                    "{config} : {diagnostics:?}"
                );
            }
        }
        let (applicability, diagnostics) = applicability_of(
            &serde_json::json!({"phone": "/play", "socket": "/ws"}),
            "E1",
        );
        assert_eq!(applicability, Applicability::Applicable);
        assert_eq!(diagnostics, Vec::<String>::new());
    }

    #[test]
    fn no_phone_takes_a_reason_and_a_desktop_display() {
        for config in [
            serde_json::json!({}),
            serde_json::json!({"phone": null}),
            serde_json::json!({"phone": false}),
            serde_json::json!({"phone": null, "socket": null}),
            serde_json::json!({"phone": null, "display": {"role": "tv"}}),
            serde_json::json!({"phone": null, "display": {"role": "tv", "reason": "Une TV sans personne devant."}}),
            serde_json::json!({"phone": null, "display": {"role": "desktop", "reason": ""}}),
            serde_json::json!({"phone": null, "display": {"role": "desktop"}}),
        ] {
            for id in [
                "C1", "C2", "C3", "C4", "C5", "C8", "C10", "T4", "R1", "R3", "E1", "C7",
            ] {
                let (applicability, diagnostics) = applicability_of(&config, id);
                assert_eq!(applicability, Applicability::Applicable, "{config} {id}");
                assert!(
                    diagnostics.iter().any(|d| d.contains("display")),
                    "{config} : {diagnostics:?}"
                );
            }
        }
        let solo = serde_json::json!({"phone": null, "socket": null, "display": {"role": "desktop", "reason": "Éditeur solo au clavier, sans téléphone."}});
        for id in ["C1", "C10", "R3", "E1", "C7"] {
            let (applicability, diagnostics) = applicability_of(&solo, id);
            assert!(
                matches!(applicability, Applicability::NotApplicable(_)),
                "{id}"
            );
            assert_eq!(diagnostics, Vec::<String>::new());
        }
    }

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
        game_requirements(&BTreeMap::new(), &mut plan);
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
            bindings: vec![],
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

    /// A game with a test `replay`, and whatever bindings a case gives it.
    fn bound(text: &str) -> (Contract, Plan) {
        let contract: Contract = toml::from_str(&format!(
            r#"
schema = 1
[[checks]]
id = "replay"
kind = "cargo-test"
package = "game-core"
target_kind = "lib"
target = "game_core"
name = "tests::replay_is_deterministic"
[[checks]]
id = "other"
kind = "cargo-test"
package = "game-core"
target_kind = "lib"
target = "game_core"
name = "tests::other"
[[guarantees]]
id = "J1"
claim = "Une garantie du jeu."
checks = ["other"]
{text}
"#
        ))
        .unwrap();
        let config = serde_json::json!({"phone": "/play", "socket": "/ws"});
        let plan = resolve(&contract, &config);
        (contract, plan)
    }

    fn obligation<'a>(plan: &'a Plan, id: &str) -> &'a Obligation {
        plan.obligations.iter().find(|o| o.id == id).unwrap()
    }

    #[test]
    fn a_bound_requirement_runs_the_games_checks_and_only_those() {
        let (_, plan) = bound(
            r#"
[[bindings]]
requirement = "D1"
checks = ["replay"]
assumptions = ["Le replay couvre une partie entière."]
"#,
        );
        assert_eq!(plan.diagnostics, Vec::<String>::new());
        let d1 = obligation(&plan, "D1");
        assert_eq!(d1.controls, ["game:replay"]);
        assert_eq!(d1.applicability, Applicability::Applicable);
        assert_eq!(d1.owner, "game");
        assert!(d1.assumptions.iter().any(|a| a.contains("partie entière")));
        assert!(d1.assumptions.iter().any(|a| a.contains("n’établit pas")));
        assert!(plan.controls.iter().all(|c| c.id != "unbound:D1"));
        assert_eq!(
            plan.controls
                .iter()
                .filter(|c| c.id == "game:replay")
                .count(),
            1
        );
        let unbound = obligation(&plan, "D3");
        assert_eq!(unbound.controls, ["unbound:D3"]);
    }

    #[test]
    fn binding_a_conditional_requirement_declares_it_applicable() {
        let (_, plan) = bound("");
        assert!(matches!(
            obligation(&plan, "D4").applicability,
            Applicability::Unknown(_)
        ));
        let (_, plan) = bound("[[bindings]]\nrequirement = \"D4\"\nchecks = [\"replay\"]\n");
        assert_eq!(
            obligation(&plan, "D4").applicability,
            Applicability::Applicable
        );
    }

    #[test]
    fn a_binding_keeps_every_common_minimum() {
        let (_, without) = bound("");
        let (_, with) = bound("[[bindings]]\nrequirement = \"E5\"\nchecks = [\"replay\"]\n");
        let ids = |plan: &Plan| {
            plan.obligations
                .iter()
                .map(|o| o.id.clone())
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(ids(&without), ids(&with));
        for (before, after) in without.obligations.iter().zip(&with.obligations) {
            if before.id != "E5" {
                assert_eq!(before.controls, after.controls, "{}", before.id);
                assert_eq!(before.applicability, after.applicability, "{}", before.id);
            }
        }
    }

    #[test]
    fn an_invalid_binding_is_refused_and_binds_nothing() {
        let d3 = |checks: &str| format!("[[bindings]]\nrequirement = \"D3\"\nchecks = {checks}\n");
        for (text, expected) in [
            (d3(r#"["inexistant"]"#), "contrôle inconnu inexistant"),
            (d3(r#"["replay", "inexistant"]"#), "contrôle inconnu"),
            (d3("[]"), "aucun contrôle"),
            (d3(r#"["replay", "replay"]"#), "contrôle répété"),
            (
                format!("{}{}", d3(r#"["replay"]"#), d3(r#"["other"]"#)),
                "reliée deux fois",
            ),
        ] {
            let (_, plan) = bound(&text);
            assert!(
                plan.diagnostics.iter().any(|d| d.contains(expected)),
                "{text} : {:?}",
                plan.diagnostics
            );
            assert_eq!(obligation(&plan, "D3").controls, ["unbound:D3"], "{text}");
        }
        for requirement in [
            "C1",
            "P1",
            "H1",
            "S1",
            "static:common",
            "J1",
            "D2",
            "Z9",
            "",
        ] {
            let (_, plan) = bound(&format!(
                "[[bindings]]\nrequirement = \"{requirement}\"\nchecks = [\"replay\"]\n"
            ));
            assert!(
                plan.diagnostics.iter().any(|d| d.contains("à relier")),
                "{requirement} : {:?}",
                plan.diagnostics
            );
        }
        let unknown_field: Result<Contract, _> = toml::from_str(
            "schema = 1\n[[bindings]]\nrequirement = \"D1\"\nchecks = []\nsatisfied = true\n",
        );
        assert!(unknown_field.is_err());
    }

    #[test]
    fn a_check_used_only_by_a_binding_is_a_real_control() {
        let (_, plan) = bound("");
        assert!(
            plan.diagnostics
                .iter()
                .any(|d| d == "contrôle sans engagement : replay")
        );
        let (_, plan) = bound("[[bindings]]\nrequirement = \"D5\"\nchecks = [\"replay\"]\n");
        assert_eq!(plan.diagnostics, Vec::<String>::new());
        assert!(plan.controls.iter().any(
            |c| c.id == "game:replay" && matches!(c.execution, ExecutionSpec::CargoTest { .. })
        ));
    }

    fn passed(control: &Control) -> crate::verification::model::Observation {
        crate::verification::model::Observation {
            control_id: control.id.clone(),
            method: control.method,
            scope: control.scope.clone(),
            outcome: crate::verification::model::Outcome::Passed,
            input_identity: Some("current".to_owned()),
            detail: String::new(),
            command: vec![],
            artifacts: vec![],
        }
    }

    #[test]
    fn a_binding_counts_only_through_its_observation() {
        use crate::verification::model::{Assessment, Outcome};
        let (_, plan) = bound("[[bindings]]\nrequirement = \"D1\"\nchecks = [\"replay\"]\n");
        let control = plan
            .controls
            .iter()
            .find(|c| c.id == "game:replay")
            .unwrap();
        let selected = BTreeSet::from([control.id.clone()]);
        let assess = |observations: &[_]| {
            crate::verification::evaluate::evaluate(&plan, observations, &selected, Some("current"))
                .obligations
                .into_iter()
                .find(|o| o.id == "D1")
                .unwrap()
                .assessment
        };
        assert_eq!(assess(&[]), Assessment::Pending);
        assert_eq!(assess(&[passed(control)]), Assessment::Satisfied);
        for (outcome, expected) in [
            (Outcome::Failed, Assessment::Failed),
            (Outcome::Ignored, Assessment::Pending),
            (Outcome::NotRun, Assessment::Pending),
        ] {
            let mut observed = passed(control);
            observed.outcome = outcome;
            assert_eq!(assess(&[observed]), expected);
        }
    }

    #[test]
    fn binding_all_seven_lifts_the_only_cause_they_hold_and_no_other() {
        let every = UNBOUND.iter().fold(String::new(), |text, (id, _, _)| {
            format!("{text}[[bindings]]\nrequirement = \"{id}\"\nchecks = [\"replay\"]\n")
        });
        let (contract, mut plan) = bound(&every);
        // A game whose harness obligations all apply and are observed.
        for obligation in &mut plan.obligations {
            if matches!(obligation.applicability, Applicability::Unknown(_)) {
                obligation.applicability = Applicability::NotApplicable("fixture".to_owned());
            }
        }
        let automatic = |plan: &Plan| {
            let observations = plan
                .controls
                .iter()
                .filter(|c| {
                    !matches!(
                        c.execution,
                        ExecutionSpec::Manual | ExecutionSpec::Unbound { .. }
                    )
                })
                .map(passed)
                .collect::<Vec<_>>();
            let selected = observations.iter().map(|o| o.control_id.clone()).collect();
            crate::verification::evaluate::evaluate(plan, &observations, &selected, Some("current"))
        };
        assert_eq!(contract.bindings.len(), 7);
        let evaluation = automatic(&plan);
        assert!(evaluation.all_automated_obligations_satisfied);
        // The real-device rehearsal stays a human judgement.
        assert!(!evaluation.complete);
        let (_, mut partial) = bound("");
        partial.diagnostics.clear();
        let evaluation = automatic(&partial);
        assert!(!evaluation.all_automated_obligations_satisfied);
    }

    #[test]
    fn loopback_ui_observations_cannot_claim_local_network_coverage() {
        let contract = Contract {
            schema: 1,
            guarantees: vec![],
            checks: vec![],
            catalogue_decisions: vec![],
            bindings: vec![],
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
