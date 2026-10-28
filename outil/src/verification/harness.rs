//! The browser harness produces observations; the common evaluator owns the verdict.
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

use serde::Deserialize;

use super::{
    check::observe,
    evaluate, identity, load,
    model::{self, ExecutionSpec, Outcome},
    render, resolve,
};
use crate::{
    embedded,
    util::{self, Result},
};

#[derive(Debug, Deserialize)]
struct Evidence {
    schema: u32,
    completed: bool,
    infrastructure_error: Option<String>,
    records: Vec<Record>,
}

#[derive(Debug, Deserialize)]
struct Record {
    scenario: String,
    item: String,
    status: String,
    note: String,
    #[serde(default)]
    explicit_exclusion: bool,
}

pub fn run(dir: &Path, harness: &Path, extra: &[String]) -> Result<bool> {
    let started = util::now_utc();
    let contract = load(dir)?;
    let config = util::read(&dir.join("cert.json"))?;
    let config = serde_json::from_str(&config).map_err(|e| e.to_string())?;
    let plan = resolve::resolve(&contract, &config);
    let (scenarios, arguments) = selection(extra)?;
    if scenarios.contains("ui") {
        crate::ui_contract::validate(&config)?;
        crate::ui_contract::validate_patterns(dir, &config)?;
    }
    let mut selected = BTreeSet::from(["harness:health".to_owned()]);
    for control in &plan.controls {
        if let ExecutionSpec::Harness { scenario, .. } = &control.execution
            && scenarios.contains(scenario)
        {
            selected.insert(control.id.clone());
        }
    }
    // Build served artefacts before the observation snapshot. Their bytes remain
    // covered by the identity; they are never silently excluded as "generated".
    let prepared = prepare(dir, harness, &arguments)?;
    let before = identity::capture(dir, &plan, &selected, Some(harness), &arguments);
    let input = before.as_ref().ok().map(|i| i.value.clone());
    let mut errors = before
        .as_ref()
        .err()
        .cloned()
        .into_iter()
        .collect::<Vec<_>>();
    let out = dir.join("certification");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let raw = out.join("harness-observations.json");
    if raw.exists() {
        std::fs::remove_file(&raw).map_err(|e| e.to_string())?;
    }
    let (status, output, transport_errors) = if prepared.status.success() {
        launch(dir, harness, &arguments, &raw)?
    } else {
        (
            prepared.status,
            String::from_utf8_lossy(&prepared.stdout)
                .lines()
                .map(str::to_owned)
                .collect(),
            vec![format!(
                "Préparation du contrôleur échouée : {}",
                String::from_utf8_lossy(&prepared.stderr)
            )],
        )
    };
    errors.extend(transport_errors);
    let evidence: Result<Evidence> =
        util::read(&raw).and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()));
    let observations = collect(
        &plan,
        &selected,
        evidence,
        status,
        input.as_deref(),
        &arguments,
    );
    let after = identity::capture(dir, &plan, &selected, Some(harness), &arguments);
    errors.extend(after.as_ref().err().cloned());
    if before.as_ref().ok().map(|i| &i.value) != after.as_ref().ok().map(|i| &i.value) {
        errors.push("Les entrées ou le contrôleur ont changé pendant le passage".to_owned());
    }
    let mut evaluation = evaluate::evaluate(
        &plan,
        &observations,
        &selected,
        after.as_ref().ok().map(|i| i.value.as_str()),
    );
    if !errors.is_empty() {
        evaluation.selected_scope_satisfied = false;
        evaluation.complete = false;
        evaluation.all_automated_obligations_satisfied = false;
        evaluation.diagnostics.extend(errors.clone());
    }
    let passed = evaluation.selected_scope_satisfied;
    let report = model::VerificationReport {
        schema: 1,
        soiree: embedded::VERSION.to_owned(),
        started,
        finished: util::now_utc(),
        selection: selected.into_iter().collect(),
        controller_exit_code: status.code(),
        identity_before: input,
        identity_after: after.ok().map(|i| i.value),
        identity_errors: errors,
        provenance: before.ok(),
        plan,
        observations,
        evaluation,
    };
    render::save(dir, &report, "verification")?;
    copy_artifacts(&out, &output)?;
    println!(
        "\nPérimètre demandé {} ; voir certification/verification.json. Les contrôles absents restent non attestés.",
        if passed { "satisfait" } else { "non satisfait" }
    );
    Ok(passed)
}

fn prepare(dir: &Path, harness: &Path, arguments: &[String]) -> Result<std::process::Output> {
    println!("Préparation des artefacts servis avant l’empreinte…");
    Command::new("node")
        .arg(harness.join("cert.mjs"))
        .arg(dir)
        .args(arguments)
        .arg("--prepare-only")
        .output()
        .map_err(|error| format!("préparation node : {error}"))
}

fn copy_artifacts(out: &Path, output: &[String]) -> Result<()> {
    if let Some(captures) = crate::certify::captures(output) {
        for name in ["performance.json", "rapport.md"] {
            let path = captures.join(name);
            if path.is_file() {
                util::write(
                    &out.join(if name == "rapport.md" {
                        "harness-observations.md"
                    } else {
                        "harness-performance.json"
                    }),
                    &util::read(&path)?,
                )?;
            }
        }
    }
    Ok(())
}

fn launch(
    dir: &Path,
    harness: &Path,
    arguments: &[String],
    raw: &Path,
) -> Result<(std::process::ExitStatus, Vec<String>, Vec<String>)> {
    let mut errors = Vec::new();
    let mut child = Command::new("node")
        .arg(harness.join("cert.mjs"))
        .arg(dir)
        .args(arguments)
        .env("SOIREE_OBSERVATIONS_PATH", raw)
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("node : {e}"))?;
    let mut output = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(line) => {
                    println!("{line}");
                    output.push(line);
                }
                Err(error) => errors.push(format!("lecture du contrôleur : {error}")),
            }
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    Ok((status, output, errors))
}

fn collect(
    plan: &model::Plan,
    selected: &BTreeSet<String>,
    evidence: Result<Evidence>,
    status: std::process::ExitStatus,
    input: Option<&str>,
    arguments: &[String],
) -> Vec<model::Observation> {
    let mut observations = Vec::new();
    let mut health = Outcome::Passed;
    let mut health_detail = format!("exit={:?}", status.code());
    match evidence {
        Err(error) => {
            health = Outcome::InfrastructureError;
            let _: std::fmt::Result = write!(
                health_detail,
                " ; observations absentes ou illisibles : {error}"
            );
        }
        Ok(evidence) => {
            if evidence.schema != 1
                || !evidence.completed
                || evidence.infrastructure_error.is_some()
                || status.code().is_none_or(|c| c > 1)
            {
                health = Outcome::InfrastructureError;
                let _: std::fmt::Result = write!(
                    health_detail,
                    " ; contrôleur incomplet : {:?}",
                    evidence.infrastructure_error
                );
            } else if !status.success() {
                health = Outcome::Failed;
            }
            for control in &plan.controls {
                if let ExecutionSpec::Harness { scenario, item } = &control.execution {
                    if !selected.contains(&control.id) {
                        continue;
                    }
                    let records = evidence
                        .records
                        .iter()
                        .filter(|r| {
                            &r.scenario == scenario
                                && &r.item == item
                                && !(item == "U1" && r.explicit_exclusion)
                        })
                        .collect::<Vec<_>>();
                    let outcome = summarize(&records);
                    let detail = records
                        .iter()
                        .map(|r| r.note.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    observations.push(observe(
                        control,
                        outcome,
                        input.map(str::to_owned),
                        detail,
                        arguments.to_vec(),
                    ));
                }
            }
            for record in &evidence.records {
                if record.status == "fail" {
                    let id = resolve::harness_id(&record.scenario, &record.item);
                    if !selected.contains(&id) {
                        health = if record.item == "H1" {
                            Outcome::InfrastructureError
                        } else {
                            Outcome::Failed
                        };
                        let _: std::fmt::Result =
                            write!(health_detail, " ; {} : {}", record.item, record.note);
                    }
                }
            }
        }
    }
    if let Some(control) = plan.controls.iter().find(|c| c.id == "harness:health") {
        observations.push(observe(
            control,
            health,
            input.map(str::to_owned),
            health_detail,
            arguments.to_vec(),
        ));
    }
    observations
}

fn summarize(records: &[&Record]) -> Outcome {
    if records.is_empty() {
        return Outcome::NotRun;
    }
    if records.iter().any(|r| r.status == "fail") {
        return Outcome::Failed;
    }
    if records.iter().any(|r| r.status == "partial") {
        return Outcome::ReviewRequired;
    }
    if records.iter().any(|r| r.status == "skip") {
        return Outcome::NotRun;
    }
    if records.iter().all(|r| r.status == "ok") {
        Outcome::Passed
    } else {
        Outcome::InfrastructureError
    }
}

fn selection(extra: &[String]) -> Result<(BTreeSet<String>, Vec<String>)> {
    let mut selected = resolve::HARNESS
        .iter()
        .filter(|(s, _)| *s != "soak")
        .map(|(s, _)| (*s).to_owned())
        .collect::<BTreeSet<_>>();
    let mut arguments = extra.to_vec();
    let mut args = extra.iter();
    while let Some(arg) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("{arg} attend une valeur"))?;
        match arg.as_str() {
            "--only" => {
                selected = value.split(',').map(str::to_owned).collect();
                if selected.is_empty()
                    || selected.iter().any(|s| {
                        !resolve::HARNESS.iter().any(|(known, _)| known == s) || s == "soak"
                    })
                {
                    return Err("--only contient un scénario absent ou invalide".to_owned());
                }
            }
            "--soak" => {
                let minutes = value
                    .parse::<u32>()
                    .map_err(|_| "--soak attend un nombre de minutes")?;
                if minutes < 30 {
                    return Err("L’obligation R2 exige au moins 30 minutes".to_owned());
                }
                selected.insert("soak".to_owned());
            }
            "--phones" | "--seed" => {
                let _: u32 = value
                    .parse()
                    .map_err(|_| format!("{arg} attend un entier positif"))?;
            }
            _ => return Err(format!("option harnais inconnue : {arg}")),
        }
    }
    if !extra.iter().any(|s| s == "--seed") {
        let seed = util::now_utc()
            .bytes()
            .fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(u32::from(b)));
        arguments.extend(["--seed".to_owned(), seed.to_string()]);
    }
    Ok((selected, arguments))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_measurements_preserve_the_legacy_report_bytes() {
        let dir = crate::testdir::fresh("observer-artifacts");
        let out = dir.join("certification");
        let captures = dir.join("captures");
        util::write(&out.join("performance.json"), "legacy measurements\n").unwrap();
        util::write(&out.join("rapport.md"), "legacy report\n").unwrap();
        util::write(&captures.join("performance.json"), "new measurements\n").unwrap();
        util::write(&captures.join("rapport.md"), "new observations\n").unwrap();
        copy_artifacts(&out, &[format!("Captures : {}", captures.display())]).unwrap();
        assert_eq!(
            util::read(&out.join("performance.json")).unwrap(),
            "legacy measurements\n"
        );
        assert_eq!(
            util::read(&out.join("rapport.md")).unwrap(),
            "legacy report\n"
        );
        assert_eq!(
            util::read(&out.join("harness-performance.json")).unwrap(),
            "new measurements\n"
        );
        assert_eq!(
            util::read(&out.join("harness-observations.md")).unwrap(),
            "new observations\n"
        );
    }
    #[test]
    fn absent_skip_and_partial_are_not_success() {
        assert_eq!(summarize(&[]), Outcome::NotRun);
        for (status, outcome) in [
            ("skip", Outcome::NotRun),
            ("partial", Outcome::ReviewRequired),
            ("fail", Outcome::Failed),
            ("invented", Outcome::InfrastructureError),
        ] {
            let r = Record {
                scenario: "join".into(),
                item: "R3".into(),
                status: status.into(),
                note: String::new(),
                explicit_exclusion: false,
            };
            assert_eq!(summarize(&[&r]), outcome);
        }
    }
    #[test]
    fn selection_rejects_empty_or_unknown_and_short_soak() {
        for args in [["--only", ""], ["--only", "invented"], ["--soak", "1"]] {
            assert!(selection(&args.map(str::to_owned)).is_err());
        }
        let (s, a) = selection(&["--only".into(), "ui,panic".into()]).unwrap();
        assert_eq!(s.len(), 2);
        assert!(a.iter().any(|s| s == "--seed"));
    }
}
