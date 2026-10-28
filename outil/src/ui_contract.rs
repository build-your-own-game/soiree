//! The declarations that make browser coverage travel with each game.
use serde_json::Value;

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
}
