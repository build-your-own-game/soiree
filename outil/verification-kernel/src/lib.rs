//! Finite decision policy shared by the ordinary CLI build and Verus.
//!
//! Resolution, tool execution, fingerprints and serialization happen outside
//! this crate. Their observations are reduced to [`Facts`]. The proof is about
//! what this policy may conclude from those facts, not their truthfulness.

#![no_std]

use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applicability {
    Applies,
    /// An exclusion already resolved by the caller's requirement policy.
    Excluded,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Execution {
    Passed,
    Failed,
    NotRun,
    Ignored,
    InfrastructureError,
    ReviewRequired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Match {
    Matches,
    Mismatches,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freshness {
    Current,
    Stale,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Facts {
    pub applicability: Applicability,
    pub execution: Execution,
    pub method_match: Match,
    pub scope_match: Match,
    pub freshness: Freshness,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Satisfied,
    NotApplicable,
    Incomplete,
    Failed,
    InfrastructureError,
    ReviewRequired,
}

/// Evidence belongs to the required method, scope and current inputs.
pub open spec fn context_is_valid(facts: Facts) -> bool {
    facts.method_match == Match::Matches
        && facts.scope_match == Match::Matches
        && facts.freshness == Freshness::Current
}

pub open spec fn success_is_justified(facts: Facts) -> bool {
    facts.applicability == Applicability::Applies
        && facts.execution == Execution::Passed
        && context_is_valid(facts)
}

fn valid_context(facts: Facts) -> (valid: bool)
    ensures valid == context_is_valid(facts),
{
    matches!((facts.method_match, facts.scope_match, facts.freshness),
        (Match::Matches, Match::Matches, Freshness::Current))
}

/// Evaluates one resolved obligation. Unusable evidence is incomplete, even
/// when its old execution succeeded or failed. Exclusions require no evidence.
#[must_use]
pub fn assess(facts: Facts) -> (result: Verdict)
    ensures
        (result == Verdict::Satisfied) == success_is_justified(facts),
        (result == Verdict::NotApplicable)
            == (facts.applicability == Applicability::Excluded),
        facts.applicability == Applicability::Unknown
            ==> result == Verdict::Incomplete,
        facts.applicability == Applicability::Applies && !context_is_valid(facts)
            ==> result == Verdict::Incomplete,
        facts.applicability == Applicability::Applies && context_is_valid(facts)
            && facts.execution == Execution::InfrastructureError
            ==> result == Verdict::InfrastructureError,
        facts.applicability == Applicability::Applies && context_is_valid(facts)
            && facts.execution == Execution::Failed
            ==> result == Verdict::Failed,
        facts.applicability == Applicability::Applies && context_is_valid(facts)
            && facts.execution == Execution::ReviewRequired
            ==> result == Verdict::ReviewRequired,
        facts.applicability == Applicability::Applies
            && (facts.execution == Execution::NotRun || facts.execution == Execution::Ignored)
            ==> result == Verdict::Incomplete,
{
    match facts.applicability {
        Applicability::Excluded => Verdict::NotApplicable,
        Applicability::Unknown => Verdict::Incomplete,
        Applicability::Applies => {
            if !valid_context(facts) {
                return Verdict::Incomplete;
            }
            match facts.execution {
                Execution::Passed => Verdict::Satisfied,
                Execution::Failed => Verdict::Failed,
                Execution::NotRun | Execution::Ignored => Verdict::Incomplete,
                Execution::InfrastructureError => Verdict::InfrastructureError,
                Execution::ReviewRequired => Verdict::ReviewRequired,
            }
        },
    }
}

/// A summary precedence, not a ranking of how strong a proof is.
pub open spec fn priority_spec(verdict: Verdict) -> u8 {
    match verdict {
        Verdict::NotApplicable => 0,
        Verdict::Satisfied => 1,
        Verdict::Incomplete => 2,
        Verdict::ReviewRequired => 3,
        Verdict::Failed => 4,
        Verdict::InfrastructureError => 5,
    }
}

fn priority(verdict: Verdict) -> (result: u8)
    ensures result == priority_spec(verdict),
{
    match verdict {
        Verdict::NotApplicable => 0,
        Verdict::Satisfied => 1,
        Verdict::Incomplete => 2,
        Verdict::ReviewRequired => 3,
        Verdict::Failed => 4,
        Verdict::InfrastructureError => 5,
    }
}

pub open spec fn combined(left: Verdict, right: Verdict) -> Verdict {
    if priority_spec(left) >= priority_spec(right) { left } else { right }
}

/// Conjunctive summary. All details stay in the caller's report: the summary
/// retains the highest-priority unresolved outcome and never turns it green.
#[must_use]
pub fn combine(left: Verdict, right: Verdict) -> (result: Verdict)
    ensures
        result == combined(left, right),
        result == left || result == right,
        priority_spec(result) >= priority_spec(left),
        priority_spec(result) >= priority_spec(right),
        (result == Verdict::NotApplicable)
            == (left == Verdict::NotApplicable && right == Verdict::NotApplicable),
        result == Verdict::Satisfied ==> (
            (left == Verdict::Satisfied || left == Verdict::NotApplicable)
            && (right == Verdict::Satisfied || right == Verdict::NotApplicable)
            && (left == Verdict::Satisfied || right == Verdict::Satisfied)
        ),
{
    if priority(left) >= priority(right) { left } else { right }
}

/// The summary is independent of item order and grouping; adding the same
/// observation twice cannot hide another obligation's unresolved verdict.
pub proof fn aggregation_laws(a: Verdict, b: Verdict, c: Verdict)
    ensures
        combined(a, b) == combined(b, a),
        combined(combined(a, b), c) == combined(a, combined(b, c)),
        combined(a, a) == a,
        combined(a, Verdict::NotApplicable) == a,
{
}

} // verus!

#[cfg(test)]
mod tests;
