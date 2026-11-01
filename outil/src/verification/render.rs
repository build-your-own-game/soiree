//! Readable views never decide whether an obligation is satisfied.
use std::fmt::Write as _;
use std::path::Path;

use super::model::{AssessedObligation, Assessment, Contract, ExecutionSpec, VerificationReport};
use crate::util::{self, Result};

pub const START: &str = "<!-- soiree:guarantees:start -->";
pub const END: &str = "<!-- soiree:guarantees:end -->";

pub fn guarantees(contract: &Contract) -> String {
    let mut text = format!(
        "{START}\n\n## Les items\n\n| # | Exigence | Contrôles | Origine |\n|---|---|---|---|\n"
    );
    for g in &contract.guarantees {
        let id = if g.retired {
            format!("~~{}~~", g.id)
        } else {
            g.id.clone()
        };
        let checks = g
            .checks
            .iter()
            .map(|s| format!("`{s}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let origins = g
            .origins
            .iter()
            .map(|s| format!("`{s}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let _: std::fmt::Result =
            writeln!(text, "| {id} | {} | {checks} | {origins} |", cell(&g.claim));
    }
    text.push_str("\n## Décisions du catalogue\n\n");
    for d in &contract.catalogue_decisions {
        let _: std::fmt::Result = writeln!(text, "- {} — {} : {}", d.id, d.status, cell(&d.reason));
    }
    let _: std::fmt::Result = write!(text, "\n{END}");
    text
}

pub fn check_view(dir: &Path, contract: &Contract) -> Result<()> {
    let text = util::read(&dir.join("CERTIFICATION.md"))?;
    let start = text.find(START).ok_or(
        "CERTIFICATION.md : vue structurée absente ; lancer soiree contracts --write-view",
    )?;
    let end = text
        .find(END)
        .ok_or("CERTIFICATION.md : fin de vue absente")?
        .saturating_add(END.len());
    let actual = text
        .get(start..end)
        .ok_or("CERTIFICATION.md : marqueurs inversés")?;
    if actual != guarantees(contract) {
        return Err(
            "CERTIFICATION.md : vue périmée ; lancer soiree contracts --write-view".to_owned(),
        );
    }
    Ok(())
}

pub fn write_view(dir: &Path, contract: &Contract) -> Result<()> {
    let path = dir.join("CERTIFICATION.md");
    let text = util::read(&path)?;
    let start = text
        .find(START)
        .ok_or("CERTIFICATION.md : marqueur de début absent")?;
    let end = text
        .find(END)
        .ok_or("CERTIFICATION.md : marqueur de fin absent")?
        .saturating_add(END.len());
    let before = text.get(..start).ok_or("marqueur invalide")?;
    let after = text.get(end..).ok_or("marqueur invalide")?;
    util::write(&path, &format!("{before}{}{after}", guarantees(contract)))
}

pub fn save(dir: &Path, report: &VerificationReport, stem: &str) -> Result<()> {
    let out = dir.join("certification");
    let json = serde_json::to_string_pretty(report).map_err(|e| e.to_string())?;
    util::write(&out.join(format!("{stem}.json")), &format!("{json}\n"))?;
    let evaluation = &report.evaluation;
    let mut text = format!(
        "# Vérification\n\n{}\n\nPérimètre demandé : **{}**. Ensemble des obligations automatisées : **{}**.\n\nUn périmètre réussi ne valide pas les contrôles non exécutés.\n\n| Exigence | Résultat |\n|---|---|\n",
        summary(report),
        if evaluation.selected_scope_satisfied {
            "satisfait"
        } else {
            "non satisfait"
        },
        if evaluation.all_automated_obligations_satisfied {
            "satisfait"
        } else {
            "incomplet"
        }
    );
    for o in &evaluation.obligations {
        let _: std::fmt::Result = writeln!(text, "| {} | {} |", o.id, label(o.assessment));
    }
    for d in &evaluation.diagnostics {
        let _: std::fmt::Result = writeln!(text, "\n- {}", cell(d));
    }
    let reviewed = report
        .plan
        .catalogue
        .iter()
        .filter(|d| d.status != "unreviewed")
        .count();
    let _: std::fmt::Result = writeln!(
        text,
        "\nCatalogue : {reviewed} décisions explicites ; les autres items restent à examiner. Une justification ne constitue pas une preuve.\n"
    );
    util::write(&out.join(format!("{stem}.md")), &text)
}

/// What a run means for whoever reads it: what to fix, what a person has to
/// judge, and what this run never looked at. The exit code follows the same
/// line: a judgement stays visible without failing, a hole does not.
pub fn summary(report: &VerificationReport) -> String {
    let evaluation = &report.evaluation;
    let selected_pending = |o: &AssessedObligation| {
        o.controls
            .iter()
            .any(|(id, a)| *a == Assessment::Pending && report.selection.contains(id))
    };
    let has = |o: &AssessedObligation, kind: fn(&ExecutionSpec) -> bool| {
        o.controls.iter().any(|(id, _)| {
            report
                .plan
                .controls
                .iter()
                .any(|c| &c.id == id && kind(&c.execution))
        })
    };
    let (mut verified, mut not_applicable) = (0_usize, 0_usize);
    let (mut at_the_table, mut unbound, mut elsewhere) = (0_usize, 0_usize, 0_usize);
    let (mut to_fix, mut broken, mut unverified, mut judged) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut health = None;
    for o in &evaluation.obligations {
        match o.assessment {
            Assessment::Satisfied => verified = verified.saturating_add(1),
            Assessment::NotApplicable => not_applicable = not_applicable.saturating_add(1),
            // The harness's exit code repeats the failures it reports.
            Assessment::Failed if o.id == "H1" => health = Some(line(report, o)),
            Assessment::Failed => to_fix.push(line(report, o)),
            Assessment::InfrastructureError => broken.push(line(report, o)),
            Assessment::Pending | Assessment::ReviewRequired if selected_pending(o) => {
                unverified.push(line(report, o));
            }
            Assessment::ReviewRequired => judged.push(line(report, o)),
            Assessment::Pending if has(o, |e| matches!(e, ExecutionSpec::Manual)) => {
                at_the_table = at_the_table.saturating_add(1);
            }
            Assessment::Pending if has(o, |e| matches!(e, ExecutionSpec::Unbound { .. })) => {
                unbound = unbound.saturating_add(1);
            }
            Assessment::Pending => elsewhere = elsewhere.saturating_add(1),
        }
    }
    if to_fix.is_empty() {
        to_fix.extend(health);
    }
    let mut text = if evaluation.selected_scope_satisfied {
        format!(
            "✓ Aucun problème trouvé dans ce passage : {verified} vérifiées, {not_applicable} non applicables."
        )
    } else if to_fix.is_empty() && unverified.is_empty() {
        "⚠ Le passage n’a pas abouti : le jeu n’est pas forcément en cause.".to_owned()
    } else {
        format!(
            "✗ À corriger : {} point(s).",
            to_fix.len().saturating_add(unverified.len())
        )
    };
    let problems = evaluation
        .diagnostics
        .iter()
        .map(|d| shorten(d))
        .collect::<Vec<_>>();
    for (title, lines) in [
        ("À corriger", &to_fix),
        ("Panne de l’outil, pas du jeu", &broken),
        ("Problèmes du passage", &problems),
        ("Pas pu vérifier", &unverified),
        ("À juger par un humain, sans bloquer", &judged),
    ] {
        if !lines.is_empty() {
            let _: std::fmt::Result = write!(text, "\n\n{title} :\n- {}", lines.join("\n- "));
        }
    }
    if at_the_table
        .saturating_add(unbound)
        .saturating_add(elsewhere)
        > 0
    {
        let _: std::fmt::Result = write!(
            text,
            "\n\nHors de ce passage : {at_the_table} à vérifier au salon avec de vrais téléphones, {elsewhere} que cette commande ne lance pas, {unbound} sans contrôle relié."
        );
    }
    text
}

fn line(report: &VerificationReport, o: &AssessedObligation) -> String {
    let detail = o
        .controls
        .iter()
        .filter(|(_, a)| !matches!(a, Assessment::Satisfied | Assessment::NotApplicable))
        .filter_map(|(id, _)| report.observations.iter().find(|x| &x.control_id == id))
        .filter_map(|x| x.detail.lines().next())
        .find(|d| !d.is_empty());
    format!("{} : {}", o.id, shorten(detail.unwrap_or(&o.claim)))
}

/// One readable line: the reports keep the complete text.
fn shorten(text: &str) -> String {
    const MAX: usize = 220;
    let single = cell(text);
    if single.chars().count() > MAX {
        format!("{}…", single.chars().take(MAX).collect::<String>())
    } else {
        single
    }
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}
pub fn label(a: Assessment) -> &'static str {
    match a {
        Assessment::Satisfied => "satisfait",
        Assessment::NotApplicable => "non applicable",
        Assessment::Pending => "pas vérifié ici",
        Assessment::Failed => "à corriger",
        Assessment::InfrastructureError => "panne de l’outil",
        Assessment::ReviewRequired => "à juger par un humain",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verification::model::{
        Applicability, Control, Evaluation, Method, Obligation, Observation, Outcome, Plan,
    };

    fn report(
        assessments: &[(&str, Assessment, ExecutionSpec)],
        detail: &str,
    ) -> VerificationReport {
        let id = |name: &str| format!("control:{name}");
        let satisfied =
            |a: Assessment| matches!(a, Assessment::Satisfied | Assessment::NotApplicable);
        VerificationReport {
            schema: 1,
            soiree: "test".to_owned(),
            started: String::new(),
            finished: String::new(),
            selection: assessments
                .iter()
                .filter(|(_, _, spec)| {
                    !matches!(spec, ExecutionSpec::Manual | ExecutionSpec::Unbound { .. })
                })
                .map(|(name, _, _)| id(name))
                .collect(),
            controller_exit_code: None,
            identity_before: None,
            identity_after: None,
            identity_errors: vec![],
            provenance: None,
            plan: Plan {
                schema: 1,
                policy_version: "test".to_owned(),
                obligations: assessments
                    .iter()
                    .map(|(name, _, _)| Obligation {
                        id: (*name).to_owned(),
                        claim: format!("claim {name}"),
                        applicability: Applicability::Applicable,
                        controls: vec![id(name)],
                        owner: "common".to_owned(),
                        assumptions: vec![],
                    })
                    .collect(),
                controls: assessments
                    .iter()
                    .map(|(name, _, spec)| Control {
                        id: id(name),
                        method: Method::Test,
                        scope: "scope".to_owned(),
                        execution: spec.clone(),
                    })
                    .collect(),
                catalogue: vec![],
                diagnostics: vec![],
            },
            observations: assessments
                .iter()
                .map(|(name, _, _)| Observation {
                    control_id: id(name),
                    method: Method::Test,
                    scope: "scope".to_owned(),
                    outcome: Outcome::Passed,
                    input_identity: None,
                    detail: detail.to_owned(),
                    command: vec![],
                    artifacts: vec![],
                })
                .collect(),
            evaluation: Evaluation {
                obligations: assessments
                    .iter()
                    .map(|(name, a, _)| AssessedObligation {
                        id: (*name).to_owned(),
                        claim: format!("claim {name}"),
                        assessment: *a,
                        controls: vec![(id(name), *a)],
                        assumptions: vec![],
                    })
                    .collect(),
                selected_scope_satisfied: assessments.iter().all(|(_, a, spec)| {
                    satisfied(*a)
                        || *a == Assessment::ReviewRequired
                        || (matches!(spec, ExecutionSpec::Manual | ExecutionSpec::Unbound { .. })
                            && *a == Assessment::Pending)
                }),
                all_automated_obligations_satisfied: false,
                complete: false,
                diagnostics: vec![],
            },
        }
    }

    fn automatic() -> ExecutionSpec {
        ExecutionSpec::Static {
            name: "test".to_owned(),
        }
    }

    #[test]
    fn a_human_judgement_reads_as_a_pass_with_something_to_look_at() {
        let text = summary(&report(
            &[
                ("A", Assessment::Satisfied, automatic()),
                ("C8", Assessment::ReviewRequired, automatic()),
                ("S1", Assessment::Pending, ExecutionSpec::Manual),
            ],
            "le message est-il clair ?",
        ));
        assert!(text.starts_with("✓ Aucun problème trouvé"), "{text}");
        assert!(text.contains("À juger par un humain"), "{text}");
        assert!(text.contains("C8 : le message est-il clair ?"), "{text}");
        assert!(text.contains("1 à vérifier au salon"), "{text}");
    }

    #[test]
    fn a_check_that_does_not_apply_is_not_counted_as_verified() {
        let text = summary(&report(
            &[
                ("A", Assessment::Satisfied, automatic()),
                ("B", Assessment::NotApplicable, automatic()),
            ],
            "",
        ));
        assert!(text.contains("1 vérifiées, 1 non applicables"), "{text}");
    }

    #[test]
    fn a_failure_and_a_hole_are_both_named_as_work_to_do() {
        let text = summary(&report(
            &[
                ("P1", Assessment::Failed, automatic()),
                ("E4", Assessment::Pending, automatic()),
            ],
            "détail",
        ));
        assert!(text.starts_with("✗ À corriger : 2 point(s)."), "{text}");
        assert!(text.contains("Pas pu vérifier :\n- E4"), "{text}");
    }

    #[test]
    fn a_broken_tool_is_not_blamed_on_the_game() {
        let text = summary(&report(
            &[("H1", Assessment::InfrastructureError, automatic())],
            "aucune réponse HTTP",
        ));
        assert!(text.starts_with("⚠ Le passage n’a pas abouti"), "{text}");
        assert!(text.contains("Panne de l’outil, pas du jeu"), "{text}");
    }

    #[test]
    fn a_real_failure_is_named_even_when_the_run_had_trouble() {
        let mut broken = report(&[("P1", Assessment::Failed, automatic())], "image lente");
        broken
            .evaluation
            .diagnostics
            .push("cert.json illisible".to_owned());
        let text = summary(&broken);
        assert!(text.starts_with("✗ À corriger : 1 point(s)."), "{text}");
        assert!(text.contains("Problèmes du passage"), "{text}");
    }

    #[test]
    fn the_harness_exit_code_does_not_count_as_one_more_point() {
        let text = summary(&report(
            &[
                ("H1", Assessment::Failed, automatic()),
                ("C1", Assessment::Failed, automatic()),
            ],
            "détail",
        ));
        assert!(text.starts_with("✗ À corriger : 1 point(s)."), "{text}");
        assert!(!text.contains("H1"), "{text}");
    }

    #[test]
    fn a_requirement_nothing_can_run_is_not_blamed_on_this_command() {
        let unbound = ExecutionSpec::Unbound {
            reason: "aucune liaison".to_owned(),
        };
        let text = summary(&report(&[("D1", Assessment::Pending, unbound)], ""));
        assert!(text.contains("1 sans contrôle relié"), "{text}");
    }

    #[test]
    fn only_the_first_line_of_a_detail_is_quoted() {
        let text = summary(&report(
            &[("C1", Assessment::Failed, automatic())],
            "le pire\nle reste",
        ));
        assert!(text.contains("C1 : le pire"), "{text}");
        assert!(!text.contains("le reste"), "{text}");
    }

    #[test]
    fn a_long_detail_stays_on_one_short_line() {
        let long = format!("première\nseconde {}", "x".repeat(400));
        assert!(!shorten(&long).contains('\n'));
        assert!(shorten(&long).chars().count() <= 221);
    }
}
