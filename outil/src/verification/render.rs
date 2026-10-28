//! Readable views never decide whether an obligation is satisfied.
use std::fmt::Write as _;
use std::path::Path;

use super::model::{Assessment, Contract, VerificationReport};
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
        "# Vérification\n\nPérimètre demandé : **{}**. Ensemble des obligations automatisées : **{}**.\n\nUn périmètre réussi ne valide pas les contrôles non exécutés.\n\n| Exigence | Résultat |\n|---|---|\n",
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

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}
pub fn label(a: Assessment) -> &'static str {
    match a {
        Assessment::Satisfied => "satisfait",
        Assessment::NotApplicable => "non applicable",
        Assessment::Pending => "non attesté",
        Assessment::Failed => "échec produit",
        Assessment::InfrastructureError => "panne du contrôleur",
        Assessment::ReviewRequired => "à examiner",
    }
}
