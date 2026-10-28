//! Deterministic evaluation. The final finite decision is the proved production kernel.
use std::collections::{BTreeMap, BTreeSet};

use verification_kernel as kernel;

use super::model::{
    Applicability, AssessedObligation, Assessment, Evaluation, ExecutionSpec, Observation, Outcome,
    Plan,
};

pub fn evaluate(
    plan: &Plan,
    observations: &[Observation],
    selected: &BTreeSet<String>,
    identity: Option<&str>,
) -> Evaluation {
    let mut diagnostics = plan.diagnostics.clone();
    let mut by_control = BTreeMap::new();
    let mut duplicates = BTreeSet::new();
    for observation in observations {
        if by_control
            .insert(observation.control_id.as_str(), observation)
            .is_some()
        {
            duplicates.insert(observation.control_id.as_str());
            diagnostics.push(format!(
                "Observations concurrentes ou dupliquées : {}",
                observation.control_id
            ));
        }
        if !plan.controls.iter().any(|c| c.id == observation.control_id) {
            diagnostics.push(format!(
                "Observation hors plan : {}",
                observation.control_id
            ));
        }
    }
    let mut obligations = Vec::new();
    let mut selected_ok = !selected.is_empty() && diagnostics.is_empty();
    for obligation in &plan.obligations {
        let applicability = match obligation.applicability {
            Applicability::Applicable => kernel::Applicability::Applies,
            Applicability::NotApplicable(_) => kernel::Applicability::Excluded,
            Applicability::Unknown(_) => kernel::Applicability::Unknown,
        };
        let mut total = if obligation.controls.is_empty() {
            kernel::Verdict::Incomplete
        } else {
            kernel::Verdict::NotApplicable
        };
        let mut controls = Vec::new();
        for id in &obligation.controls {
            let expected = plan.controls.iter().find(|c| &c.id == id);
            let actual = by_control.get(id.as_str()).copied();
            let mut facts = control_facts(expected, actual, applicability, identity);
            if duplicates.contains(id.as_str()) {
                facts.execution = kernel::Execution::InfrastructureError;
            }
            let verdict = kernel::assess(facts);
            if selected.contains(id)
                && !matches!(
                    verdict,
                    kernel::Verdict::Satisfied | kernel::Verdict::NotApplicable
                )
            {
                selected_ok = false;
            }
            total = kernel::combine(total, verdict);
            controls.push((id.clone(), convert(verdict)));
        }
        obligations.push(AssessedObligation {
            id: obligation.id.clone(),
            claim: obligation.claim.clone(),
            assessment: convert(total),
            controls,
            assumptions: obligation.assumptions.clone(),
        });
    }
    for id in selected {
        if !plan.controls.iter().any(|c| &c.id == id)
            || !plan.obligations.iter().any(|o| o.controls.contains(id))
        {
            diagnostics.push(format!("Contrôle sélectionné hors obligation : {id}"));
            selected_ok = false;
        }
    }
    let satisfied = |a: Assessment| matches!(a, Assessment::Satisfied | Assessment::NotApplicable);
    let complete = diagnostics.is_empty() && obligations.iter().all(|o| satisfied(o.assessment));
    let all_automated_obligations_satisfied = diagnostics.is_empty()
        && obligations.iter().all(|o| {
            o.controls
                .iter()
                .filter(|(id, _)| {
                    plan.controls
                        .iter()
                        .any(|c| &c.id == id && !matches!(c.execution, ExecutionSpec::Manual))
                })
                .all(|(_, assessment)| satisfied(*assessment))
        });
    Evaluation {
        obligations,
        selected_scope_satisfied: selected_ok && diagnostics.is_empty(),
        all_automated_obligations_satisfied,
        complete,
        diagnostics,
    }
}

fn control_facts(
    expected: Option<&super::model::Control>,
    actual: Option<&Observation>,
    applicability: kernel::Applicability,
    identity: Option<&str>,
) -> kernel::Facts {
    let method_match = match (expected, actual) {
        (Some(e), Some(a)) if e.method == a.method => kernel::Match::Matches,
        (Some(_), Some(_)) => kernel::Match::Mismatches,
        _ => kernel::Match::Unknown,
    };
    let scope_match = match (expected, actual) {
        (Some(e), Some(a)) if e.scope == a.scope => kernel::Match::Matches,
        (Some(_), Some(_)) => kernel::Match::Mismatches,
        _ => kernel::Match::Unknown,
    };
    let freshness = match (identity, actual.and_then(|a| a.input_identity.as_deref())) {
        (Some(now), Some(then)) if now == then => kernel::Freshness::Current,
        (Some(_), Some(_)) => kernel::Freshness::Stale,
        _ => kernel::Freshness::Unknown,
    };
    let execution = actual.map_or(kernel::Execution::NotRun, |a| match a.outcome {
        Outcome::Passed => kernel::Execution::Passed,
        Outcome::Failed => kernel::Execution::Failed,
        Outcome::Ignored => kernel::Execution::Ignored,
        Outcome::NotRun => kernel::Execution::NotRun,
        Outcome::InfrastructureError => kernel::Execution::InfrastructureError,
        Outcome::ReviewRequired => kernel::Execution::ReviewRequired,
    });
    kernel::Facts {
        applicability,
        execution,
        method_match,
        scope_match,
        freshness,
    }
}

fn convert(verdict: kernel::Verdict) -> Assessment {
    match verdict {
        kernel::Verdict::Satisfied => Assessment::Satisfied,
        kernel::Verdict::NotApplicable => Assessment::NotApplicable,
        kernel::Verdict::Incomplete => Assessment::Pending,
        kernel::Verdict::Failed => Assessment::Failed,
        kernel::Verdict::InfrastructureError => Assessment::InfrastructureError,
        kernel::Verdict::ReviewRequired => Assessment::ReviewRequired,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verification::model::{Control, Method, Obligation};
    fn fixture() -> (Plan, Observation, BTreeSet<String>) {
        let id = "required".to_owned();
        let p = Plan {
            schema: 1,
            policy_version: "test".to_owned(),
            obligations: vec![Obligation {
                id: "R".to_owned(),
                claim: "Claim".to_owned(),
                applicability: Applicability::Applicable,
                controls: vec![id.clone()],
                owner: "common".to_owned(),
                assumptions: vec![],
            }],
            controls: vec![Control {
                id: id.clone(),
                method: Method::Test,
                scope: "exact target".to_owned(),
                execution: ExecutionSpec::Manual,
            }],
            catalogue: vec![],
            diagnostics: vec![],
        };
        let o = Observation {
            control_id: id.clone(),
            method: Method::Test,
            scope: "exact target".to_owned(),
            outcome: Outcome::Passed,
            input_identity: Some("now".to_owned()),
            detail: String::new(),
            command: vec![],
            artifacts: vec![],
        };
        (p, o, BTreeSet::from([id]))
    }
    #[test]
    fn absent_ignored_stale_wrong_scope_or_argument_cannot_satisfy_a_test() {
        let (p, o, s) = fixture();
        assert!(evaluate(&p, std::slice::from_ref(&o), &s, Some("now")).complete);
        assert!(!evaluate(&p, &[], &s, Some("now")).selected_scope_satisfied);
        for bad in [
            Observation {
                outcome: Outcome::Ignored,
                ..o.clone()
            },
            Observation {
                input_identity: None,
                ..o.clone()
            },
            Observation {
                input_identity: Some("old".to_owned()),
                ..o.clone()
            },
            Observation {
                method: Method::Justification,
                ..o.clone()
            },
            Observation {
                scope: "another target".to_owned(),
                ..o.clone()
            },
        ] {
            assert!(!evaluate(&p, &[bad], &s, Some("now")).complete);
        }
    }
    #[test]
    fn duplicates_and_unknown_applicability_do_not_become_green() {
        let (mut p, o, s) = fixture();
        assert!(!evaluate(&p, &[o.clone(), o.clone()], &s, Some("now")).complete);
        p.obligations.first_mut().unwrap().applicability =
            Applicability::Unknown("genre".to_owned());
        assert_eq!(
            evaluate(&p, &[o], &s, Some("now"))
                .obligations
                .first()
                .unwrap()
                .assessment,
            Assessment::Pending
        );
    }
    #[test]
    fn infrastructure_and_product_failure_remain_distinct() {
        let (p, o, s) = fixture();
        for (outcome, expected) in [
            (Outcome::Failed, Assessment::Failed),
            (
                Outcome::InfrastructureError,
                Assessment::InfrastructureError,
            ),
        ] {
            let e = evaluate(
                &p,
                &[Observation {
                    outcome,
                    ..o.clone()
                }],
                &s,
                Some("now"),
            );
            assert_eq!(e.obligations.first().unwrap().assessment, expected);
        }
    }

    #[test]
    fn manual_controls_cannot_hide_an_automatic_failure() {
        let (mut plan, mut observation, selected) = fixture();
        plan.controls.first_mut().unwrap().execution = ExecutionSpec::Static {
            name: "test-controller".into(),
        };
        plan.controls.push(Control {
            id: "manual".into(),
            method: Method::Justification,
            scope: "human".into(),
            execution: ExecutionSpec::Manual,
        });
        plan.obligations
            .first_mut()
            .unwrap()
            .controls
            .push("manual".into());
        observation.outcome = Outcome::Failed;
        let result = evaluate(&plan, &[observation], &selected, Some("now"));
        assert!(!result.all_automated_obligations_satisfied);
    }

    #[test]
    fn a_selected_orphan_is_rejected() {
        let (mut plan, observation, selected) = fixture();
        plan.obligations.clear();
        let result = evaluate(&plan, &[observation], &selected, Some("now"));
        assert!(!result.selected_scope_satisfied);
    }
}
