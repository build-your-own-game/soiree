# Verification policy kernel

This crate is the finite decision policy called by the `soiree` CLI. Its
`src/lib.rs` is compiled by ordinary Cargo and checked by Verus. The proof is
attached to that production code, with no separately rewritten model and no
unchecked specification for `assess` or `combine`.

## Contract

`assess(Facts) -> Verdict` separates resolved applicability, execution outcome,
method match, scope match and freshness. A verdict is `Satisfied` **if and only
if** the obligation applies, the execution passed, both matches are established
and the observation belongs to the current inputs.

An exclusion already resolved by the requirement policy is `NotApplicable`.
Unknown applicability is `Incomplete`. Unusable, stale or unidentified evidence
is also incomplete, including a failed execution from an obsolete context. For
a valid context, a failed check, an infrastructure error and a required review
retain distinct verdicts. An ignored or unexecuted check never satisfies an
applicable obligation.

`combine(Verdict, Verdict) -> Verdict` is a conjunctive summary, with precedence
`InfrastructureError > Failed > ReviewRequired > Incomplete > Satisfied >
NotApplicable`. This priority summarizes the report; it is not a ranking of
evidence strength. Detailed observations must remain available in the caller.
An empty fold starts at `NotApplicable`, so an excluded-only collection does
not manufacture a successful verification. Unresolved obligations cannot be
hidden by satisfied ones. The operation is commutative, associative and
idempotent.

The crate does not parse declarations, execute tools, compare fingerprints or
serialize reports. The CLI wrapper performs these operations and maps their
results to `Facts`. These mappings and the truthfulness of observations remain
outside the proof. In particular, `Excluded` must not be constructed solely
because a game author requested an exclusion.

## Ordinary compilation and exhaustive checks

No verifier installation is needed to compile the CLI or the games. The erased
`vstd` dependency is pinned to `=0.0.0-2026-10-25-0158`, with default features
disabled. The standalone workspace keeps verification tooling outside every
game's workspace.

From the shared repository:

```sh
cargo +1.99 test --manifest-path outil/verification-kernel/Cargo.toml --locked
cargo +1.99 clippy --manifest-path outil/verification-kernel/Cargo.toml --all-targets --locked -- -D warnings
```

The tests exercise all 486 combinations of facts, all 36 verdict pairs and all
216 verdict triples. They also check the meaning of individual outcomes and
empty or excluded-only folds. This is exhaustive checking of the finite domain;
it is reported separately from a Verus proof.

## Formal verification and negative controls

The pinned verifier is the official
[Verus release 0.2026.10.26.d17fbd0](https://github.com/verus-lang/verus/releases/tag/release/0.2026.10.26.d17fbd0).
It requires the named toolchain `1.99.1`. Verus is installed outside this
repository following its [official installation instructions](https://github.com/verus-lang/verus/blob/release/0.2026.10.26.d17fbd0/INSTALL.md).
The verification script never downloads or installs anything.

```sh
SOIREE_VERUS=/absolute/path/to/verus tools/verify-policy.sh
SOIREE_VERUS=/absolute/path/to/verus tools/verify-policy.sh --mutations
```

The second command additionally mutates copies of the exact production source:
it accepts stale evidence, treats an unexecuted check as successful, and drops
an unresolved obligation during aggregation. The unchanged postconditions must
reject all three mutations. A syntax error or a missing tool does not count as
detecting a mutant. Mutants are never compiled into the CLI.

Logs and a structured `report.json` are written under the ignored
`target/verification-policy/`; `--output` selects another directory. The report
records source and dependency-manifest hashes, verifier version and binary
hash, commands, actual verification summaries, timings and the source stability
check. A missing/wrong verifier or a timeout produces a failed infrastructure
result; ordinary Rust tests do not become a claimed formal proof.

## Proof boundary

Verus checks the executable functions and the aggregation laws in `src/lib.rs`.
The source contains no `assume`, `admit`, `assume_specification`, `external_body`
or excluded executable body. The script rejects unchecked kernel annotations.
The trusted base still includes Verus, its solver, standard specifications and
the Rust compilation/execution platform, as described by the
[Verus trust model](https://verus-lang.github.io/verus/guide/tcb.html).

The theorem justifies the finite policy's conclusions from supplied facts. It
does not establish a game's rules, the correctness of an external test oracle,
the completeness of a capability declaration or the CLI's provenance mapping.
