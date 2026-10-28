//! The declarations that make browser coverage travel with each game.
use std::path::Path;

use serde_json::Value;

use crate::util;

pub const CATEGORIES: [&str; 4] = ["editing", "effects", "lifecycle", "async"];

pub fn validate(config: &Value) -> Result<(), String> {
    let ui = config
        .get("ui")
        .ok_or("ui absent : décrire les scénarios et les non-applicabilités")?;
    let mut covered = std::collections::BTreeSet::new();
    if let Some(scenarios) = ui.get("scenarios") {
        for scenario in scenarios
            .as_array()
            .ok_or("ui.scenarios doit être une liste")?
        {
            if scenario["name"]
                .as_str()
                .is_none_or(|s| s.trim().is_empty())
                || !matches!(scenario["screen"].as_str(), Some("tv" | "phone"))
            {
                return Err("ui.scenarios : name et screen requis".into());
            }
            let categories = scenario["covers"]
                .as_array()
                .filter(|v| !v.is_empty())
                .ok_or("ui.scenarios[].covers absent")?;
            for category in categories {
                let name = category.as_str().ok_or("catégorie ui invalide")?;
                if !CATEGORIES.contains(&name) {
                    return Err(format!("catégorie ui inconnue : {name}"));
                }
                covered.insert(name);
            }
            if !scenario["steps"].as_array().is_some_and(|steps| {
                steps
                    .iter()
                    .any(|step| step.get("assert").is_some() || step.get("unchanged").is_some())
            }) {
                return Err("ui.scenarios[].steps doit observer une assertion".into());
            }
        }
    }
    if let Some(checks) = ui.get("checks") {
        for check in checks.as_array().ok_or("ui.checks doit être une liste")? {
            if check.get("timeout_ms").is_some_and(|timeout| {
                !timeout.as_f64().is_some_and(|number| {
                    (1.0..=2_147_483_647.0).contains(&number)
                        && number.fract().classify() == std::num::FpCategory::Zero
                })
            }) {
                return Err("ui.checks[].timeout_ms doit être un entier positif fini compatible avec un délai Node".into());
            }
            if check
                .get("success")
                .is_some_and(|pattern| pattern.as_str().is_none_or(|s| s.trim().is_empty()))
            {
                return Err(
                    "ui.checks[].success doit être une expression régulière non vide".into(),
                );
            }
            if check["name"].as_str().is_none_or(|s| s.trim().is_empty())
                || !check["command"].as_array().is_some_and(|args| {
                    !args.is_empty()
                        && args
                            .iter()
                            .all(|arg| arg.as_str().is_some_and(|s| !s.is_empty()))
                })
            {
                return Err("ui.checks : name et command requis".into());
            }
            for category in check["covers"]
                .as_array()
                .filter(|v| !v.is_empty())
                .ok_or("ui.checks[].covers absent")?
            {
                let name = category.as_str().ok_or("catégorie ui invalide")?;
                if !CATEGORIES.contains(&name) {
                    return Err(format!("catégorie ui inconnue : {name}"));
                }
                covered.insert(name);
            }
        }
    }
    for category in CATEGORIES {
        let reason = ui
            .get("not_applicable")
            .and_then(|reasons| reasons.get(category))
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty());
        if covered.contains(category) {
            if reason.is_some() {
                return Err(format!(
                    "ui : {category} est couvert et déclaré non applicable"
                ));
            }
        } else if reason.is_none() {
            return Err(format!(
                "ui : {category} doit être exercé ou explicitement non applicable"
            ));
        }
    }
    Ok(())
}

/// Compile declaration patterns with the same JavaScript grammar as the observer.
/// Patterns are data in a JSON argument; the executed script is always fixed.
pub fn validate_patterns(dir: &Path, config: &Value) -> Result<(), String> {
    const SCRIPT: &str = r"
const patterns = JSON.parse(process.argv[1]);
for (const pattern of patterns) {
  try { new RegExp(pattern); }
  catch (error) { console.error(error.message); process.exit(1); }
}
";
    let Some(checks) = config.get("ui").and_then(|ui| ui.get("checks")) else {
        return Ok(());
    };
    let checks = checks.as_array().ok_or("ui.checks doit être une liste")?;
    let patterns = checks
        .iter()
        .filter_map(|check| check.get("success"))
        .map(|pattern| {
            pattern
                .as_str()
                .filter(|pattern| !pattern.trim().is_empty())
                .ok_or_else(|| {
                    "ui.checks[].success doit être une expression régulière non vide".to_owned()
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if patterns.is_empty() {
        return Ok(());
    }
    let patterns = serde_json::to_string(&patterns).map_err(|error| error.to_string())?;
    let output = util::run(dir, "node", &["-e", SCRIPT, "--", &patterns])?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "ui.checks[].success : expression régulière JavaScript invalide : {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn each_scope_is_exercised_or_explicitly_excluded() {
        let mut config = json!({"ui":{"not_applicable":{"editing":"no input","effects":"no effects","lifecycle":"page lifetime","async":"no tasks"}}});
        assert!(validate(&config).is_ok());
        config["ui"]["not_applicable"]["editing"] = json!("");
        assert!(validate(&config).is_err());
        config["ui"]["scenarios"] = json!([{"name":"edit","screen":"phone","covers":["editing"],"steps":[{"assert":"document.activeElement"}]}]);
        assert!(validate(&config).is_ok());
        config["ui"]["not_applicable"]["editing"] = json!("no fields");
        assert!(validate(&config).is_err());
        assert!(validate(&json!({})).is_err());
    }

    #[test]
    fn optional_success_evidence_is_a_nonempty_pattern_string() {
        let mut config = json!({"ui":{"checks":[{"name":"one exact test","command":["cargo","test"],"covers":CATEGORIES,"success":"test result: ok"}]}});
        assert!(validate(&config).is_ok());
        for invalid in [json!(1), json!(""), json!(null)] {
            config["ui"]["checks"][0]["success"] = invalid;
            assert!(validate(&config).is_err());
        }
    }

    #[test]
    fn command_timeout_matches_positive_integer_node_timer_limits() {
        let mut config = json!({"ui":{"checks":[{"name":"bounded","command":["node","check.mjs"],"covers":CATEGORIES}]}});
        assert!(validate(&config).is_ok());
        // JSON 1.0/1e3 also become integer Numbers in JavaScript.
        for timeout in [json!(1), json!(1.0), json!(1e3), json!(2_147_483_647u32)] {
            config["ui"]["checks"][0]["timeout_ms"] = timeout;
            assert!(validate(&config).is_ok());
        }
        for timeout in [
            json!(0),
            json!(-1),
            json!(0.5),
            json!(2_147_483_648u64),
            json!("20"),
            json!(null),
            json!(true),
        ] {
            config["ui"]["checks"][0]["timeout_ms"] = timeout;
            assert!(validate(&config).unwrap_err().contains("timeout_ms"));
        }
    }

    #[test]
    fn actual_node_parser_rejects_bad_regex_and_accepts_javascript_features() {
        let mut config = json!({"ui":{"checks":[{"name":"actual JS","command":["node","check.mjs"],"covers":CATEGORIES,"success":"["}]}});
        assert!(validate(&config).is_ok());
        assert!(
            validate_patterns(&std::env::temp_dir(), &config)
                .unwrap_err()
                .contains("expression régulière JavaScript invalide")
        );
        for pattern in [
            r"(?=ready)ready",
            r"(a)\1",
            r"(?<word>a)\k<word>",
            "'(?=quote)",
        ] {
            config["ui"]["checks"][0]["success"] = json!(pattern);
            assert!(
                validate_patterns(&std::env::temp_dir(), &config).is_ok(),
                "valid JS pattern rejected: {pattern}"
            );
        }
    }

    #[test]
    fn no_patterns_do_not_start_a_node_subprocess() {
        let missing = std::env::temp_dir().join(format!(
            "soiree-no-patterns-nonexistent-{}",
            std::process::id()
        ));
        assert!(!missing.exists());
        // A spawned process would fail because its working directory is absent.
        assert!(validate_patterns(&missing, &json!({"ui":{"checks":[]}})).is_ok());
        assert!(validate_patterns(&missing, &json!({"ui":{"checks":[{"name":"no success pattern","command":["node","check.mjs"]}]}})).is_ok());
        assert!(validate_patterns(&missing, &json!({"ui":{}})).is_ok());
    }
}
