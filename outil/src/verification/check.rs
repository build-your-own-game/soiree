//! Existing controllers adapted to structured observations and one evaluator.
use std::collections::BTreeSet;
use std::path::Path;

use super::{
    cargo, evaluate, identity, load,
    model::{self, Control, ExecutionSpec, Observation, Outcome, Plan},
    render, resolve,
};
use crate::{
    check, embedded,
    report::{Level, Report},
    util,
};

struct Passage {
    started: String,
    plan: Plan,
    selected: BTreeSet<String>,
    before: util::Result<identity::Identity>,
    observations: Vec<Observation>,
}

pub fn run(dir: &Path, options: check::Options) -> bool {
    if !dir.is_dir() {
        eprintln!("{} : dossier absent ; aucun rapport créé", dir.display());
        return false;
    }
    let started = util::now_utc();
    let plan = match directory_plan(dir) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("{error}");
            return false;
        }
    };
    let selected = select(&plan, options);
    // First-time creation of Cargo.lock belongs before the input snapshot.
    if dir.join("Cargo.toml").is_file() && !options.locked && !dir.join("Cargo.lock").is_file() {
        let _: util::Result<std::process::Output> =
            util::run(dir, "cargo", &["metadata", "--format-version", "1"]);
    }
    let parameters = vec![format!(
        "locked={};published={};fast={}",
        options.locked, options.published, options.fast
    )];
    let before = identity::capture(dir, &plan, &selected, None, &parameters);
    let input = before.as_ref().ok().map(|identity| identity.value.as_str());
    let mut observations = run_static(dir, &plan, options, input);
    if !options.fast && plan.diagnostics.is_empty() {
        observations.extend(run_commands(dir, &plan, options, input));
        observations.extend(run_cargo(dir, &plan, options, input));
    }
    let after = identity::capture(dir, &plan, &selected, None, &parameters);
    finish(
        dir,
        options,
        Passage {
            started,
            plan,
            selected,
            before,
            observations,
        },
        after,
    )
}

fn directory_plan(dir: &Path) -> util::Result<Plan> {
    let capabilities = resolve::FoundationCapabilities {
        has_cargo: dir.join("Cargo.toml").is_file(),
        has_ios: dir.join("ios/Remote.xcodeproj").is_dir(),
    };
    if !dir.join("soiree.toml").is_file() {
        return Ok(resolve::foundation(capabilities));
    }
    let contract = load(dir)?;
    let config = util::read(&dir.join("cert.json")).and_then(|text| {
        serde_json::from_str(&text).map_err(|error| format!("cert.json : {error}"))
    })?;
    Ok(resolve::resolve_with_capabilities(
        &contract,
        &config,
        capabilities,
    ))
}

fn select(plan: &Plan, options: check::Options) -> BTreeSet<String> {
    plan.controls
        .iter()
        .filter(|control| match control.execution {
            ExecutionSpec::Static { .. } => {
                control.id == "static:common" || (control.id == "cargo:suite" && !options.fast)
            }
            ExecutionSpec::Command { .. } | ExecutionSpec::CargoTest { .. } => !options.fast,
            ExecutionSpec::Harness { .. }
            | ExecutionSpec::Manual
            | ExecutionSpec::Unbound { .. } => false,
        })
        .map(|control| control.id.clone())
        .collect()
}

fn run_static(
    dir: &Path,
    plan: &Plan,
    options: check::Options,
    input: Option<&str>,
) -> Vec<Observation> {
    let mut report = Report::new();
    check::check_project(
        dir,
        check::Options {
            fast: true,
            ..options
        },
        &mut report,
    );
    plan.controls
        .iter()
        .find(|control| control.id == "static:common")
        .map(|control| {
            observe(
                control,
                if report.count(Level::Error) == 0 {
                    Outcome::Passed
                } else {
                    Outcome::Failed
                },
                input.map(str::to_owned),
                format!("{:?}", report.lines),
                vec![],
            )
        })
        .into_iter()
        .collect()
}

fn run_commands(
    dir: &Path,
    plan: &Plan,
    options: check::Options,
    input: Option<&str>,
) -> Vec<Observation> {
    plan.controls
        .iter()
        .filter_map(|control| {
            let ExecutionSpec::Command { program, arguments } = &control.execution else {
                return None;
            };
            let mut arguments = arguments.clone();
            if control.id == "command:web-contract" && !cfg!(target_os = "macos") {
                let mut command = vec![program.clone()];
                command.extend(arguments);
                return Some(observe(
                    control,
                    Outcome::NotRun,
                    input.map(str::to_owned),
                    "Contrat Swift/Rust/JavaScript non exécuté : macOS et Swift sont nécessaires"
                        .to_owned(),
                    command,
                ));
            }
            if options.locked
                && program == "cargo"
                && arguments
                    .first()
                    .is_some_and(|arg| matches!(arg.as_str(), "clippy" | "test"))
            {
                let at = arguments
                    .iter()
                    .position(|arg| arg == "--")
                    .unwrap_or(arguments.len());
                arguments.insert(at, "--locked".to_owned());
            }
            let refs = arguments.iter().map(String::as_str).collect::<Vec<_>>();
            println!("\n{}", control.id);
            let (outcome, detail) = match util::run(dir, program, &refs) {
                Ok(output) => (
                    if output.status.success() {
                        Outcome::Passed
                    } else {
                        Outcome::Failed
                    },
                    format!(
                        "{}{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    ),
                ),
                Err(error) => (Outcome::InfrastructureError, error),
            };
            let mut command = vec![program.clone()];
            command.extend(arguments);
            if outcome != Outcome::Passed {
                eprintln!("{detail}");
            }
            Some(observe(
                control,
                outcome,
                input.map(str::to_owned),
                detail,
                command,
            ))
        })
        .collect()
}

fn run_cargo(
    dir: &Path,
    plan: &Plan,
    options: check::Options,
    input: Option<&str>,
) -> Vec<Observation> {
    if !plan.controls.iter().any(|control| {
        control.id == "cargo:suite" || matches!(control.execution, ExecutionSpec::CargoTest { .. })
    }) {
        return Vec::new();
    }
    let refs = plan
        .controls
        .iter()
        .filter_map(|control| {
            let ExecutionSpec::CargoTest { test } = &control.execution else {
                return None;
            };
            Some(test.canonicalized())
        })
        .collect::<Vec<_>>();
    let result = cargo::run_with_suites(
        dir,
        &refs,
        cargo::Options {
            locked: options.locked,
            all_workspace: true,
        },
    );
    let mut observations = Vec::new();
    for control in &plan.controls {
        let ExecutionSpec::CargoTest { test } = &control.execution else {
            continue;
        };
        let Some(execution) = execution_for(test, &result.executions) else {
            continue;
        };
        observations.push(observe(
            control,
            outcome(execution.outcome),
            input.map(str::to_owned),
            execution.detail.clone(),
            execution.command.clone(),
        ));
    }
    if let Some(control) = plan
        .controls
        .iter()
        .find(|control| control.id == "cargo:suite")
    {
        observations.push(observe_suite(control, &result, input));
    }
    observations
}

fn execution_for<'a>(
    test: &cargo::TestRef,
    executions: &'a [cargo::TestExecution],
) -> Option<&'a cargo::TestExecution> {
    let canonical = test.canonicalized();
    executions
        .iter()
        .find(|execution| execution.test == canonical)
}

fn observe_suite(control: &Control, result: &cargo::RunReport, input: Option<&str>) -> Observation {
    let mut state = if result.suites.is_empty() {
        Outcome::InfrastructureError
    } else {
        Outcome::Passed
    };
    for suite in &result.suites {
        let next = outcome(suite.outcome);
        if next == Outcome::InfrastructureError
            || (next == Outcome::Failed && state != Outcome::InfrastructureError)
        {
            state = next;
        }
    }
    let detail = format!("{:?}", result.suites);
    if state != Outcome::Passed {
        eprintln!("{detail}");
    }
    observe(
        control,
        state,
        input.map(str::to_owned),
        detail,
        vec!["Cargo/libtest".to_owned()],
    )
}

fn finish(
    dir: &Path,
    options: check::Options,
    passage: Passage,
    after: util::Result<identity::Identity>,
) -> bool {
    let Passage {
        started,
        plan,
        selected,
        before,
        observations,
    } = passage;
    let mut errors = before
        .as_ref()
        .err()
        .cloned()
        .into_iter()
        .collect::<Vec<_>>();
    errors.extend(after.as_ref().err().cloned());
    if before.as_ref().ok().map(|identity| &identity.value)
        != after.as_ref().ok().map(|identity| &identity.value)
    {
        errors.push("Les entrées ont changé pendant le passage".to_owned());
    }
    let mut evaluation = evaluate::evaluate(
        &plan,
        &observations,
        &selected,
        after.as_ref().ok().map(|identity| identity.value.as_str()),
    );
    if !errors.is_empty() {
        evaluation.selected_scope_satisfied = false;
        evaluation.complete = false;
        evaluation.all_automated_obligations_satisfied = false;
        evaluation.diagnostics.extend(errors.clone());
    }
    let passed = evaluation.selected_scope_satisfied;
    let satisfied = evaluation
        .obligations
        .iter()
        .filter(|obligation| {
            matches!(
                obligation.assessment,
                model::Assessment::Satisfied | model::Assessment::NotApplicable
            )
        })
        .count();
    println!(
        "\n{} / {} obligations satisfaites ou non applicables ; périmètre demandé {}. Les autres restent non attestées.",
        satisfied,
        evaluation.obligations.len(),
        if passed { "satisfait" } else { "non satisfait" }
    );
    let input = before.as_ref().ok().map(|identity| identity.value.clone());
    let result = model::VerificationReport {
        schema: 1,
        soiree: embedded::VERSION.to_owned(),
        started,
        finished: util::now_utc(),
        selection: selected.into_iter().collect(),
        controller_exit_code: None,
        identity_before: input,
        identity_after: after.ok().map(|identity| identity.value),
        identity_errors: errors,
        provenance: before.ok(),
        plan,
        observations,
        evaluation,
    };
    let stem = if options.fast { "check-fast" } else { "check" };
    if let Err(error) = render::save(dir, &result, stem) {
        eprintln!("{error}");
        return false;
    }
    passed
}

pub fn observe(
    control: &Control,
    outcome: Outcome,
    input_identity: Option<String>,
    detail: String,
    command: Vec<String>,
) -> Observation {
    Observation {
        control_id: control.id.clone(),
        method: control.method,
        scope: control.scope.clone(),
        outcome,
        input_identity,
        detail,
        command,
        artifacts: vec![],
    }
}

fn outcome(value: cargo::TestOutcome) -> Outcome {
    match value {
        cargo::TestOutcome::Passed => Outcome::Passed,
        cargo::TestOutcome::Failed => Outcome::Failed,
        cargo::TestOutcome::Ignored => Outcome::Ignored,
        cargo::TestOutcome::NotRun => Outcome::NotRun,
        cargo::TestOutcome::InfrastructureError => Outcome::InfrastructureError,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_directory_is_refused_without_creating_a_report() {
        let parent = crate::testdir::fresh("missing-check-folder");
        let missing = parent.join("missing");
        assert!(!run(
            &missing,
            check::Options {
                fast: true,
                ..check::Options::default()
            }
        ));
        assert!(!missing.exists());
    }

    #[test]
    fn a_catalogue_does_not_invoke_the_native_runner() {
        let plan = resolve::foundation(resolve::FoundationCapabilities {
            has_cargo: false,
            has_ios: false,
        });
        let directory = crate::testdir::fresh("catalogue-native-runner");
        assert!(
            run_cargo(
                &directory,
                &plan,
                check::Options::default(),
                Some("current")
            )
            .is_empty()
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn unavailable_swift_bindings_are_not_run_and_never_become_success() {
        let mut plan = resolve::foundation(resolve::FoundationCapabilities {
            has_cargo: true,
            has_ios: true,
        });
        plan.controls
            .retain(|control| control.id == "command:web-contract");
        let directory = crate::testdir::fresh("unavailable-swift-check");
        let observations = run_commands(
            &directory,
            &plan,
            check::Options::default(),
            Some("current"),
        );
        assert!(
            observations
                .first()
                .is_some_and(|observation| observation.outcome == Outcome::NotRun)
        );
        let selected = BTreeSet::from(["command:web-contract".to_owned()]);
        assert!(
            !evaluate::evaluate(&plan, &observations, &selected, Some("current"))
                .selected_scope_satisfied
        );
    }

    #[test]
    fn feature_order_does_not_lose_an_exact_test_observation() {
        let declared = cargo::TestRef {
            package: "game-core".to_owned(),
            target_kind: "lib".to_owned(),
            target: "game_core".to_owned(),
            name: "tests::same_name".to_owned(),
            features: vec!["z".to_owned(), "a".to_owned(), "z".to_owned()],
            default_features: true,
            profile: "dev".to_owned(),
        };
        let executions = vec![cargo::TestExecution {
            test: declared.canonicalized(),
            outcome: cargo::TestOutcome::Passed,
            command: vec![],
            detail: String::new(),
        }];
        assert!(execution_for(&declared, &executions).is_some());
        let mut other_target = declared;
        other_target.target = "another_target".to_owned();
        assert!(execution_for(&other_target, &executions).is_none());
    }
}
