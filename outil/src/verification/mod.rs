//! Shared verification policy, evidence and decision pipeline.
pub mod cargo;
pub mod check;
pub mod evaluate;
pub mod harness;
pub mod identity;
pub mod migrate;
pub mod model;
pub mod render;
pub mod resolve;

use std::path::Path;

use crate::util::{self, Result};
use model::Contract;

pub fn load(dir: &Path) -> Result<Contract> {
    let text = util::read(&dir.join("verification.toml"))?;
    toml::from_str(&text).map_err(|error| format!("verification.toml : {error}"))
}

pub fn validate_declaration(dir: &Path, report: &mut crate::report::Report) {
    let result = load(dir).and_then(|contract| {
        let config = util::read(&dir.join("cert.json"))?;
        let config = serde_json::from_str(&config).map_err(|e| format!("cert.json : {e}"))?;
        let plan = resolve::resolve(&contract, &config);
        if !plan.diagnostics.is_empty() {
            return Err(plan.diagnostics.join("\n"));
        }
        render::check_view(dir, &contract)?;
        Ok(contract.guarantees.iter().filter(|g| !g.retired).count())
    });
    match result {
        Ok(count) => report.ok(format!(
            "verification.toml : {count} engagements ; références exécutables à attester par Cargo"
        )),
        Err(error) => report.error(error),
    }
}
