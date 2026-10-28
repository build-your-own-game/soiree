//! Declarations are commitments; observations are produced by controllers.
use serde::{Deserialize, Serialize};

use super::cargo::TestRef;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub schema: u32,
    #[serde(default)]
    pub guarantees: Vec<Guarantee>,
    #[serde(default)]
    pub checks: Vec<DeclaredCheck>,
    #[serde(default)]
    pub catalogue_decisions: Vec<CatalogueDecision>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Guarantee {
    pub id: String,
    pub claim: String,
    #[serde(default)]
    pub checks: Vec<String>,
    #[serde(default)]
    pub origins: Vec<String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub retired: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeclaredCheck {
    pub id: String,
    pub kind: String,
    #[serde(flatten)]
    pub test: TestRef,
}

impl<'de> Deserialize<'de> for DeclaredCheck {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let mut fields = serde_json::Map::<String, serde_json::Value>::deserialize(deserializer)?;
        let id = fields
            .remove("id")
            .and_then(|v| v.as_str().map(str::to_owned))
            .ok_or_else(|| D::Error::custom("contrôle : id attendu"))?;
        let kind = fields
            .remove("kind")
            .and_then(|v| v.as_str().map(str::to_owned))
            .ok_or_else(|| D::Error::custom("contrôle : kind attendu"))?;
        let test =
            serde_json::from_value(serde_json::Value::Object(fields)).map_err(D::Error::custom)?;
        Ok(Self { id, kind, test })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogueDecision {
    pub id: String,
    pub status: String,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Method {
    Static,
    Test,
    Measure,
    Proof,
    Justification,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "status", content = "reason", rename_all = "kebab-case")]
pub enum Applicability {
    Applicable,
    NotApplicable(String),
    Unknown(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Obligation {
    pub id: String,
    pub claim: String,
    pub applicability: Applicability,
    pub controls: Vec<String>,
    pub owner: String,
    pub assumptions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Control {
    pub id: String,
    pub method: Method,
    pub scope: String,
    pub execution: ExecutionSpec,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ExecutionSpec {
    Unbound {
        reason: String,
    },
    Static {
        name: String,
    },
    Command {
        program: String,
        arguments: Vec<String>,
    },
    CargoTest {
        test: TestRef,
    },
    Harness {
        scenario: String,
        item: String,
    },
    Manual,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Plan {
    pub schema: u32,
    pub policy_version: String,
    pub obligations: Vec<Obligation>,
    pub controls: Vec<Control>,
    pub catalogue: Vec<CatalogueDecision>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Passed,
    Failed,
    Ignored,
    NotRun,
    InfrastructureError,
    ReviewRequired,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Observation {
    pub control_id: String,
    pub method: Method,
    pub scope: String,
    pub outcome: Outcome,
    pub input_identity: Option<String>,
    pub detail: String,
    pub command: Vec<String>,
    pub artifacts: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Assessment {
    Satisfied,
    NotApplicable,
    Pending,
    Failed,
    InfrastructureError,
    ReviewRequired,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AssessedObligation {
    pub id: String,
    pub claim: String,
    pub assessment: Assessment,
    pub controls: Vec<(String, Assessment)>,
    pub assumptions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Evaluation {
    pub obligations: Vec<AssessedObligation>,
    /// This scope ran successfully; this is not a claim about unselected work.
    pub selected_scope_satisfied: bool,
    pub all_automated_obligations_satisfied: bool,
    pub complete: bool,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VerificationReport {
    pub schema: u32,
    pub soiree: String,
    pub started: String,
    pub finished: String,
    pub selection: Vec<String>,
    pub controller_exit_code: Option<i32>,
    pub identity_before: Option<String>,
    pub identity_after: Option<String>,
    pub identity_errors: Vec<String>,
    pub provenance: Option<super::identity::Identity>,
    pub plan: Plan,
    pub observations: Vec<Observation>,
    pub evaluation: Evaluation,
}
