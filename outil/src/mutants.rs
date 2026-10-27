//! Opt-in mutation testing of the simulation. Never called by check/certify.
//!
//! cargo-mutants mutates a scratch copy, not the author's files. Surviving
//! mutants and timeouts are diagnostic: only tool/baseline failures fail this
//! command. The default scope is the core package, not the browser or server.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde_json::{Value, json};

use crate::util::{self, Result};

const INSTALL: &str = "cargo install --locked cargo-mutants --version 27.1.0";

const HELP: &str = r"soiree mutants [dossier] [--in-diff FICHIER] [--re REGEX] [--timeout S]

À la demande uniquement, jamais depuis check ou certify. Le paquet crates/core
est muté sur une copie : aucun fichier de travail n'est modifié.

Prérequis : cargo install --locked cargo-mutants --version 27.1.0
Pour les lignes modifiées : git diff HEAD > /tmp/jeu.diff
                          soiree mutants --in-diff /tmp/jeu.diff
--re limite les noms de mutants (rapport partiel) ; --timeout limite chaque test.
Les survivants et timeouts sont diagnostiques ; un échec de l'outil ou de la base
non mutée reste une erreur. Rapports : certification/mutants/rapport.md et .json.

Exclusions persistantes, dans .cargo/soiree-mutants.toml :

[[exclude]]
regex = 'impl Debug'
reason = 'Format de diagnostic, sans effet sur les règles du jeu'

Une raison non vide est obligatoire pour chaque exclusion. La configuration
native .cargo/mutants.toml est ignorée par cette commande. Les tests du commun
exercent réellement trois mutations ciblées du modèle ; les auteurs de jeux
ne paient ce coût que lorsqu'ils lancent mutants explicitement.
";

#[derive(Debug, Default)]
struct Options {
    dir: PathBuf,
    diff: Option<PathBuf>,
    regex: Option<String>,
    timeout: Option<u32>,
}

#[derive(Debug)]
struct Exclusion {
    regex: String,
    reason: String,
}

#[derive(Debug)]
struct Counts {
    total: u64,
    caught: u64,
    missed: u64,
    timeout: u64,
    unviable: u64,
}

impl Counts {
    fn read(value: &Value) -> Result<Self> {
        let count = |key| {
            value
                .get(key)
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("outcomes.json : entier {key} absent ou invalide"))
        };
        let total = count("total_mutants")?;
        let caught = count("caught")?;
        let missed = count("missed")?;
        let timeout = count("timeout")?;
        let unviable = count("unviable")?;
        let success = count("success")?;
        let sum = [caught, missed, timeout, unviable, success]
            .into_iter()
            .try_fold(0_u64, u64::checked_add)
            .ok_or("outcomes.json : agrégats trop grands")?;
        if sum != total {
            return Err("outcomes.json : agrégats incohérents".to_owned());
        }
        let scenarios = value
            .get("outcomes")
            .and_then(Value::as_array)
            .ok_or("outcomes.json : scénarios absents")?;
        if u64::try_from(
            scenarios
                .iter()
                .filter(|scenario| {
                    scenario
                        .get("scenario")
                        .and_then(|s| s.get("Mutant"))
                        .is_some()
                })
                .count(),
        )
        .ok()
            != Some(total)
        {
            return Err("outcomes.json : nombre de scénarios incohérent".to_owned());
        }
        if value.get("end_time").is_none_or(Value::is_null) {
            return Err("outcomes.json : passage incomplet".to_owned());
        }
        Ok(Self {
            total,
            caught,
            missed,
            timeout,
            unviable,
        })
    }
}

struct Results {
    outcomes: Value,
    counts: Option<Counts>,
    error: Option<String>,
}

impl Results {
    fn read(raw_dir: &Path, exit_code: Option<i32>) -> Self {
        let load = || -> Result<Value> {
            let path = raw_dir.join("outcomes.json");
            if path.is_file() {
                return serde_json::from_str(&util::read(&path)?)
                    .map_err(|error| error.to_string());
            }
            // cargo-mutants writes only mutants.json for an empty selection.
            // Verify that explicit empty list instead of inventing zero counts
            // for a missing or truncated result from an actual campaign.
            let selected: Value = serde_json::from_str(&util::read(&raw_dir.join("mutants.json"))?)
                .map_err(|error| error.to_string())?;
            if exit_code == Some(0) && selected.as_array().is_some_and(Vec::is_empty) {
                Ok(
                    json!({"outcomes":[], "total_mutants":0,"caught":0,"missed":0,"timeout":0,"unviable":0,"success":0,"end_time":util::now_utc()}),
                )
            } else {
                Err("outcomes.json absent : aucun verdict de mutation".to_owned())
            }
        };
        match load() {
            Ok(outcomes) => match Counts::read(&outcomes) {
                Ok(counts) => Self {
                    outcomes,
                    counts: Some(counts),
                    error: None,
                },
                Err(error) => Self {
                    outcomes,
                    counts: None,
                    error: Some(error),
                },
            },
            Err(error) => Self {
                outcomes: Value::Null,
                counts: None,
                error: Some(error),
            },
        }
    }
}

pub fn run(args: &[String]) -> ExitCode {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }
    match parse(args).and_then(|options| execute(&options)) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("soiree mutants : {error}");
            ExitCode::FAILURE
        }
    }
}

fn parse(args: &[String]) -> Result<Options> {
    let mut options = Options {
        dir: PathBuf::from("."),
        ..Options::default()
    };
    let mut directory = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--in-diff" => {
                options.diff = Some(PathBuf::from(
                    args.next().ok_or("--in-diff demande un fichier git diff")?,
                ));
            }
            "--re" => {
                options.regex = Some(args.next().ok_or("--re demande une expression")?.clone());
            }
            "--timeout" => {
                options.timeout = Some(
                    args.next()
                        .ok_or("--timeout demande un nombre de secondes")?
                        .parse::<u32>()
                        .ok()
                        .filter(|seconds| *seconds > 0)
                        .ok_or("--timeout doit être un entier strictement positif")?,
                );
            }
            flag if flag.starts_with('-') => return Err(format!("option inconnue : {flag}")),
            dir if directory.is_none() => directory = Some(PathBuf::from(dir)),
            extra => {
                return Err(format!(
                    "un seul dossier attendu, argument en trop : {extra}"
                ));
            }
        }
    }
    if let Some(directory) = directory {
        options.dir = directory;
    }
    Ok(options)
}

fn exclusions(dir: &Path) -> Result<Vec<Exclusion>> {
    let path = dir.join(".cargo/soiree-mutants.toml");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let table = util::parse_toml(&util::read(&path)?, ".cargo/soiree-mutants.toml")?;
    for key in table.keys() {
        if key != "exclude" {
            return Err(format!(".cargo/soiree-mutants.toml : clé inconnue {key}"));
        }
    }
    let Some(entries) = table.get("exclude") else {
        return Ok(Vec::new());
    };
    entries
        .as_array()
        .ok_or(".cargo/soiree-mutants.toml : attendu [[exclude]]")?
        .iter()
        .map(|entry| {
            let value = |key| {
                entry
                    .get(key)
                    .and_then(toml::Value::as_str)
                    .filter(|text| !text.trim().is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| format!("chaque [[exclude]] demande {key} non vide"))
            };
            let fields = entry.as_table().ok_or("attendu [[exclude]]")?;
            for key in fields.keys() {
                if !matches!(key.as_str(), "regex" | "reason") {
                    return Err(format!("[[exclude]] : clé inconnue {key}"));
                }
            }
            Ok(Exclusion {
                regex: value("regex")?,
                reason: value("reason")?,
            })
        })
        .collect()
}

/// Preserve Cargo's ancestor configuration in the scratch tree, in particular
/// the host's local salon patches. Cargo resolves paths from the config file's
/// original location. Inner configurations override outer ones, as usual.
fn ancestor_configs(dir: &Path) -> Vec<PathBuf> {
    let mut configs = Vec::new();
    for ancestor in dir.ancestors().skip(1) {
        let cargo = ancestor.join(".cargo");
        let legacy = cargo.join("config");
        let config = if legacy.is_file() {
            legacy
        } else {
            cargo.join("config.toml")
        };
        if config.is_file() {
            configs.push(config);
        }
    }
    configs.reverse();
    configs
}

fn execute(options: &Options) -> Result<bool> {
    let dir = util::absolute(&options.dir);
    let core = dir.join("crates/core/Cargo.toml");
    let package = util::parse_toml(&util::read(&core)?, "crates/core/Cargo.toml")?
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .ok_or("crates/core/Cargo.toml : package.name absent")?
        .to_owned();
    let excluded = exclusions(&dir)?;
    let version = util::stdout_of(&dir, "cargo", &["mutants", "--version"])
        .ok_or_else(|| format!("cargo-mutants absent ; installe-le avec `{INSTALL}`"))?;
    let output = dir.join("target/soiree-mutants");
    std::fs::create_dir_all(&output).map_err(|error| format!("{} : {error}", output.display()))?;
    let mut command = Command::new("cargo");
    command.current_dir(&dir).args([
        "mutants",
        "--no-config",
        "--no-shuffle",
        "--colors=never",
        "--gitignore=true",
        "--jobs=1",
        "--package",
        &package,
        "--file",
        "crates/core/src/**/*.rs",
    ]);
    command.arg("--output").arg(&output);
    // Keep invocation out of the console log: absolute paths name this machine.
    for config in ancestor_configs(&dir) {
        command.arg("--cargo-arg=--config");
        command.arg(format!("--cargo-arg={}", config.display()));
    }
    if let Some(diff) = &options.diff {
        let diff = util::absolute(diff);
        if !diff.is_file() {
            return Err(format!("diff introuvable : {}", diff.display()));
        }
        command.arg("--in-diff").arg(diff);
    }
    if let Some(regex) = &options.regex {
        command.arg("--re").arg(regex);
    }
    if let Some(seconds) = options.timeout {
        command.arg("--timeout").arg(seconds.to_string());
    }
    for exclusion in &excluded {
        command.arg("--exclude-re").arg(&exclusion.regex);
    }
    let before = crate::certify::fingerprint(&dir)
        .ok()
        .map(|fingerprint| fingerprint.value);
    let started = util::now_utc();
    println!("Mutation de {package}, sur une copie ; les survivants ne bloquent rien.");
    let status = command
        .status()
        .map_err(|error| format!("cargo mutants : {error}"))?;
    let raw_dir = output.join("mutants.out");
    let results = Results::read(&raw_dir, status.code());
    write_report(
        options,
        &dir,
        &package,
        &version,
        &started,
        status.code(),
        &results,
        &excluded,
        &raw_dir,
        before.as_deref(),
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "report inputs are the complete immutable record of one invocation"
)]
fn write_report(
    options: &Options,
    dir: &Path,
    package: &str,
    version: &str,
    started: &str,
    exit_code: Option<i32>,
    results: &Results,
    excluded: &[Exclusion],
    raw_dir: &Path,
    before: Option<&str>,
) -> Result<bool> {
    let fingerprint = crate::certify::fingerprint(dir);
    let source_changed = before
        .zip(fingerprint.as_ref().ok())
        .is_some_and(|(before, after)| before != after.value);
    let diagnostic =
        matches!(exit_code, Some(0 | 2 | 3)) && !source_changed && results.counts.is_some();
    let report = json!({
        "soiree": crate::embedded::VERSION,
        "cargo_mutants": version,
        "package": package,
        "scope": "crates/core/src/**/*.rs",
        "regex": options.regex,
        "in_diff": options.diff.is_some(),
        "timeout_seconds": options.timeout,
        "exclusions": excluded.iter().map(|e| json!({"regex": e.regex, "reason": e.reason})).collect::<Vec<_>>(),
        "started": started,
        "finished": util::now_utc(),
        "exit_code": exit_code,
        "completed": diagnostic,
        "blocking": false,
        "source_changed_during_run": source_changed,
        "fingerprint_before": before,
        "fingerprint": fingerprint.as_ref().map_or_else(|error| json!({"error": error}), |fingerprint| json!({"algorithm": "sha256", "value": fingerprint.value, "files": fingerprint.files, "uncommitted": fingerprint.uncommitted})),
        "validation_error": results.error,
        "outcomes": results.outcomes,
    });
    let target = dir.join("certification/mutants");
    util::write(
        &target.join("rapport.json"),
        &format!(
            "{}\n",
            serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
        ),
    )?;
    let mut text = format!(
        "# Mutation testing\n\n{version}. Paquet `{package}`, périmètre `crates/core/src/**/*.rs`.\n\nDiagnostic à la demande : les survivants et les délais dépassés ne bloquent ni check ni certify.\n\nDébut : {started}. Code de sortie de cargo-mutants : {exit_code:?}.\n"
    );
    if let Some(regex) = &options.regex {
        let _ = writeln!(text, "\nFiltre : `{regex}` (rapport partiel).");
    }
    if options.diff.is_some() {
        text.push_str("\nLimité aux lignes du diff fourni (rapport partiel).\n");
    }
    if diagnostic && let Some(counts) = &results.counts {
        text.push_str("\n| Testés | Détectés | Survivants | Délais dépassés | Non compilables |\n|---|---|---|---|---|\n");
        let _ = writeln!(
            text,
            "| {} | {} | {} | {} | {} |",
            counts.total, counts.caught, counts.missed, counts.timeout, counts.unviable
        );
    } else {
        text.push_str(
            "\nÉchec de l’outil ou des tests avant mutation ; aucun verdict de couverture.\n",
        );
    }
    if let Some(error) = &results.error {
        let _ = writeln!(text, "\nRapport inexploitable : {error}");
    }
    for (name, heading) in [
        ("missed.txt", "Survivants"),
        ("timeout.txt", "Délais dépassés"),
    ] {
        if let Ok(list) = std::fs::read_to_string(raw_dir.join(name))
            && !list.trim().is_empty()
        {
            let _ = writeln!(text, "\n## {heading}\n\n```text\n{}\n```", list.trim());
        }
    }
    if !excluded.is_empty() {
        text.push_str("\n## Exclusions justifiées\n\n");
        for exclusion in excluded {
            let _ = writeln!(text, "- `{}` : {}", exclusion.regex, exclusion.reason);
        }
    }
    util::write(&target.join("rapport.md"), &text)?;
    println!("{} écrit.", util::shown(&target, dir));
    Ok(diagnostic)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn exclusions_must_explain_what_is_not_checked() {
        let dir = crate::testdir::fresh("mutants-exclusions");
        let path = dir.join(".cargo/soiree-mutants.toml");
        util::write(&path, "[[exclude]]\nregex = 'impl Debug'\n").unwrap();
        assert!(exclusions(&dir).unwrap_err().contains("reason"));
        util::write(&path, "[[exclude]]\nregex = 'impl Debug'\nreason = ' '\n").unwrap();
        assert!(exclusions(&dir).is_err());
        util::write(&path, "[[exclude]]\nregex = 'impl Debug'\nreason = 'Human-readable diagnostics, not game rules'\n").unwrap();
        assert_eq!(exclusions(&dir).unwrap().len(), 1);
        assert!(parse(&args(&["--exclude-re", "anything"])).is_err());
    }

    #[test]
    fn scope_and_timeout_options_are_explicit() {
        let options = parse(&args(&[
            "game",
            "--in-diff",
            "change.diff",
            "--re",
            "score",
            "--timeout",
            "5",
        ]))
        .unwrap();
        assert_eq!(options.dir, PathBuf::from("game"));
        assert_eq!(options.diff, Some(PathBuf::from("change.diff")));
        assert_eq!(options.timeout, Some(5));
        assert!(parse(&args(&["--timeout", "0"])).is_err());
        assert!(parse(&args(&["--in-diff"])).is_err());
    }

    #[test]
    fn incomplete_or_malformed_campaigns_never_become_zero_successes() {
        let complete = json!({"total_mutants":1,"caught":1,"missed":0,"timeout":0,"unviable":0,"success":0,"end_time":"finished","outcomes":[{"scenario":{"Mutant":{}},"summary":"CaughtMutant"}]});
        assert!(Counts::read(&complete).is_ok());
        for key in [
            "total_mutants",
            "caught",
            "missed",
            "timeout",
            "unviable",
            "success",
            "end_time",
            "outcomes",
        ] {
            let mut partial = complete.clone();
            partial.as_object_mut().unwrap().remove(key);
            assert!(Counts::read(&partial).is_err(), "missing {key}");
        }
        assert!(Counts::read(&Value::Null).is_err());
        let mut inconsistent = complete;
        inconsistent["caught"] = json!(0);
        assert!(Counts::read(&inconsistent).is_err());
        let dir = crate::testdir::fresh("empty-mutants");
        util::write(&dir.join("mutants.json"), "[]").unwrap();
        assert_eq!(Results::read(&dir, Some(0)).counts.unwrap().total, 0);
        assert!(Results::read(&dir, Some(2)).counts.is_none());
    }

    #[test]
    fn a_real_mutant_in_the_generated_model_is_caught() {
        let parent = crate::testdir::fresh("model-mutants");
        let dir = crate::new::create("mutant-canards", &parent, crate::embedded::ORG).unwrap();
        // Exercise the actual embedded model without requiring its next,
        // unpublished release tag to exist on GitHub.
        let manifest = dir.join("Cargo.toml");
        let shared = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut cargo = util::read(&manifest).unwrap();
        for (name, relative) in [
            ("salon-core", "salon/crates/core"),
            ("salon-host", "salon/crates/host"),
            ("salon-web", "salon/crates/web"),
        ] {
            let original = format!(
                "{name} = {{ git = \"https://github.com/{}/soiree\", tag = \"v{}\" }}",
                crate::embedded::ORG,
                crate::embedded::VERSION
            );
            let local = format!(
                "{name} = {{ path = \"{}\" }}",
                shared.join(relative).canonicalize().unwrap().display()
            );
            cargo = cargo.replace(&original, &local);
        }
        util::write(&manifest, &cargo).unwrap();
        assert!(
            util::run(&dir, "git", &["init", "-q"])
                .unwrap()
                .status
                .success()
        );
        assert!(
            util::run(&dir, "git", &["add", "-A"])
                .unwrap()
                .status
                .success()
        );
        let options = Options {
            dir: dir.clone(),
            regex: Some("next_score".to_owned()),
            timeout: Some(10),
            diff: None,
        };
        assert!(
            execute(&options).unwrap(),
            "install prerequisite: {INSTALL}"
        );
        let report: Value = serde_json::from_str(
            &util::read(&dir.join("certification/mutants/rapport.json")).unwrap(),
        )
        .unwrap();
        assert!(report["fingerprint"]["files"].as_u64().unwrap() > 0);
        assert_eq!(report["source_changed_during_run"], false);
        let outcomes = &report["outcomes"];
        assert!(
            outcomes["caught"].as_u64().unwrap() > 0,
            "a real changed model function must be detected"
        );
        assert_eq!(outcomes["missed"].as_u64(), Some(0));
        assert_eq!(outcomes["unviable"].as_u64(), Some(0));
    }
}
