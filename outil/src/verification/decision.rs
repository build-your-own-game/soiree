//! The finite decision every evaluation reduces to. Resolution, tool execution
//! and fingerprints happen elsewhere; their observations arrive here as
//! [`Facts`]. The tests walk every combination of this small domain.

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

/// Evaluates one resolved obligation. Unusable evidence is incomplete, even
/// when its old execution succeeded or failed. Exclusions require no evidence.
#[must_use]
pub fn assess(facts: Facts) -> Verdict {
    match facts.applicability {
        Applicability::Excluded => Verdict::NotApplicable,
        Applicability::Unknown => Verdict::Incomplete,
        Applicability::Applies => {
            let usable = matches!(
                (facts.method_match, facts.scope_match, facts.freshness),
                (Match::Matches, Match::Matches, Freshness::Current)
            );
            if !usable {
                return Verdict::Incomplete;
            }
            match facts.execution {
                Execution::Passed => Verdict::Satisfied,
                Execution::Failed => Verdict::Failed,
                Execution::NotRun | Execution::Ignored => Verdict::Incomplete,
                Execution::InfrastructureError => Verdict::InfrastructureError,
                Execution::ReviewRequired => Verdict::ReviewRequired,
            }
        }
    }
}

/// A summary precedence, not a ranking of how strong a proof is.
fn priority(verdict: Verdict) -> u8 {
    match verdict {
        Verdict::NotApplicable => 0,
        Verdict::Satisfied => 1,
        Verdict::Incomplete => 2,
        Verdict::ReviewRequired => 3,
        Verdict::Failed => 4,
        Verdict::InfrastructureError => 5,
    }
}

/// Conjunctive summary: it keeps the highest-priority unresolved outcome and
/// never turns it green. Details stay in the caller's report.
#[must_use]
pub fn combine(left: Verdict, right: Verdict) -> Verdict {
    if priority(left) >= priority(right) {
        left
    } else {
        right
    }
}

#[cfg(test)]
mod tests {
    use super::{Applicability, Execution, Facts, Freshness, Match, Verdict, assess, combine};

    const APPLICABILITIES: [Applicability; 3] = [
        Applicability::Applies,
        Applicability::Excluded,
        Applicability::Unknown,
    ];
    const EXECUTIONS: [Execution; 6] = [
        Execution::Passed,
        Execution::Failed,
        Execution::NotRun,
        Execution::Ignored,
        Execution::InfrastructureError,
        Execution::ReviewRequired,
    ];
    const MATCHES: [Match; 3] = [Match::Matches, Match::Mismatches, Match::Unknown];
    const FRESHNESSES: [Freshness; 3] = [Freshness::Current, Freshness::Stale, Freshness::Unknown];
    const VERDICTS: [Verdict; 6] = [
        Verdict::Satisfied,
        Verdict::NotApplicable,
        Verdict::Incomplete,
        Verdict::Failed,
        Verdict::InfrastructureError,
        Verdict::ReviewRequired,
    ];

    #[test]
    fn every_fact_combination_preserves_the_claimed_guarantees() {
        let mut cases = 0;
        for applicability in APPLICABILITIES {
            for execution in EXECUTIONS {
                for method_match in MATCHES {
                    for scope_match in MATCHES {
                        for freshness in FRESHNESSES {
                            let facts = Facts {
                                applicability,
                                execution,
                                method_match,
                                scope_match,
                                freshness,
                            };
                            let verdict = assess(facts);
                            let valid_context = method_match == Match::Matches
                                && scope_match == Match::Matches
                                && freshness == Freshness::Current;
                            assert_eq!(
                                verdict == Verdict::Satisfied,
                                applicability == Applicability::Applies
                                    && execution == Execution::Passed
                                    && valid_context,
                                "{facts:?}"
                            );
                            assert_eq!(
                                verdict == Verdict::NotApplicable,
                                applicability == Applicability::Excluded,
                                "{facts:?}"
                            );
                            if applicability == Applicability::Unknown
                                || (applicability == Applicability::Applies && !valid_context)
                            {
                                assert_eq!(verdict, Verdict::Incomplete, "{facts:?}");
                            }
                            cases += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 486);
    }

    #[test]
    fn a_current_execution_keeps_its_distinct_outcome() {
        for (execution, expected) in [
            (Execution::Passed, Verdict::Satisfied),
            (Execution::Failed, Verdict::Failed),
            (Execution::NotRun, Verdict::Incomplete),
            (Execution::Ignored, Verdict::Incomplete),
            (Execution::InfrastructureError, Verdict::InfrastructureError),
            (Execution::ReviewRequired, Verdict::ReviewRequired),
        ] {
            assert_eq!(
                assess(Facts {
                    applicability: Applicability::Applies,
                    execution,
                    method_match: Match::Matches,
                    scope_match: Match::Matches,
                    freshness: Freshness::Current,
                }),
                expected
            );
        }
    }

    #[test]
    fn every_pair_preserves_unsatisfied_obligations() {
        for left in VERDICTS {
            for right in VERDICTS {
                let result = combine(left, right);
                assert!(result == left || result == right);
                assert_eq!(
                    result == Verdict::NotApplicable,
                    left == Verdict::NotApplicable && right == Verdict::NotApplicable
                );
                let left_ok = matches!(left, Verdict::Satisfied | Verdict::NotApplicable);
                let right_ok = matches!(right, Verdict::Satisfied | Verdict::NotApplicable);
                assert_eq!(
                    matches!(result, Verdict::Satisfied | Verdict::NotApplicable),
                    left_ok && right_ok,
                    "{left:?}, {right:?}"
                );
                assert_eq!(combine(left, right), combine(right, left));
                assert_eq!(combine(left, left), left);
                assert_eq!(combine(left, Verdict::NotApplicable), left);
            }
        }
    }

    #[test]
    fn every_triple_is_independent_of_grouping() {
        for a in VERDICTS {
            for b in VERDICTS {
                for c in VERDICTS {
                    assert_eq!(combine(combine(a, b), c), combine(a, combine(b, c)));
                }
            }
        }
    }

    #[test]
    fn empty_or_excluded_only_folds_do_not_claim_a_success() {
        assert_eq!(
            [].into_iter().fold(Verdict::NotApplicable, combine),
            Verdict::NotApplicable
        );
        assert_eq!(
            [Verdict::NotApplicable; 4]
                .into_iter()
                .fold(Verdict::NotApplicable, combine),
            Verdict::NotApplicable
        );
        assert_eq!(
            [Verdict::Satisfied, Verdict::Incomplete, Verdict::Satisfied]
                .into_iter()
                .fold(Verdict::NotApplicable, combine),
            Verdict::Incomplete
        );
    }
}
