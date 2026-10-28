//! Exercise the real CLI boundary between a trusted observer and policy evaluation.
#[cfg(test)]
mod observer {
    use serde_json::{Value, json};
    use std::path::PathBuf;
    use std::process::{Command, Output};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    /// Source and observer stay fixed; the invocation journal is a report artefact.
    const SCRIPT: &str = r"
import { appendFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
const game = process.argv.at(2);
const preparation = process.argv.includes('--prepare-only');
const output = process.env.SOIREE_OBSERVATIONS_PATH;
mkdirSync(join(game, 'certification'), { recursive: true });
appendFileSync(join(game, 'certification', 'observer-invocations.jsonl'), JSON.stringify({
  phase: preparation ? 'prepare' : 'observe',
  evidence_path_present: output !== undefined,
}) + '\n');
if (preparation) process.exit(0);
if (mode === 'stdout-only') {
  console.log('ALL CHECKS PASSED: this green text is not structured evidence');
  process.exit(0);
}
writeFileSync(output, JSON.stringify({
  schema: 1,
  completed: mode !== 'abort',
  infrastructure_error: mode === 'abort' ? 'fixture controller aborted after C2' : null,
  records: [{
    scenario: mode === 'wrong-scenario' ? 'reload' : 'cut',
    item: 'C2', status: 'ok', note: 'fixture completed the cut observation',
  }],
}));
process.exit(mode === 'abort' ? 2 : 0);
";

    struct Fixture {
        root: PathBuf,
        game: PathBuf,
        harness: PathBuf,
    }

    impl Fixture {
        fn new(mode: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "soiree-verification-observer-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let game = root.join("game");
            let harness = root.join("observer");
            std::fs::create_dir_all(game.join("src")).unwrap();
            std::fs::create_dir_all(&harness).unwrap();
            std::fs::write(
                game.join("Cargo.toml"),
                "[package]\nname='observer-fixture'\nversion='0.1.0'\nedition='2024'\n[workspace]\n",
            )
            .unwrap();
            std::fs::write(game.join("src/lib.rs"), "pub const VALUE: u8 = 1;\n").unwrap();
            std::fs::write(game.join("verification.toml"), "schema=1\n").unwrap();
            std::fs::write(
                game.join("cert.json"),
                r#"{"phone":"/play","socket":"/salon/ws"}"#,
            )
            .unwrap();
            std::fs::write(
                harness.join("cert.mjs"),
                format!(
                    "const mode = {};\n{SCRIPT}",
                    serde_json::to_string(mode).unwrap()
                ),
            )
            .unwrap();
            // Resolve the actual micro-project with Cargo. No compiler or game
            // build is involved, and no mocked metadata can hide identity errors.
            let metadata = Command::new("cargo")
                .current_dir(&game)
                .args(["metadata", "--offline", "--format-version", "1"])
                .output()
                .unwrap();
            assert_success(&metadata);
            assert!(game.join("Cargo.lock").is_file());
            Self {
                root,
                game,
                harness,
            }
        }

        fn run(&self) -> Output {
            Command::new(env!("CARGO_BIN_EXE_soiree"))
                .current_dir(&self.game)
                .env("CARGO_NET_OFFLINE", "true")
                .env_remove("SOIREE_OBSERVATIONS_PATH")
                .args(["certify", "--harness"])
                .arg(&self.harness)
                .args(["--", "--only", "cut", "--seed", "12"])
                .output()
                .unwrap()
        }

        fn report(&self) -> Value {
            serde_json::from_str(
                &std::fs::read_to_string(self.game.join("certification/verification.json"))
                    .unwrap(),
            )
            .unwrap()
        }

        fn raw_path(&self) -> PathBuf {
            self.game.join("certification/harness-observations.json")
        }

        fn assert_prepare_then_observe(&self) {
            let invocations =
                std::fs::read_to_string(self.game.join("certification/observer-invocations.jsonl"))
                    .unwrap();
            let events = invocations
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                events,
                [
                    json!({"phase":"prepare", "evidence_path_present":false}),
                    json!({"phase":"observe", "evidence_path_present":true}),
                ]
            );
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _: std::io::Result<()> = std::fs::remove_dir_all(&self.root);
        }
    }

    fn assert_success(output: &Output) {
        assert!(
            output.status.success(),
            "status={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn observation<'a>(report: &'a Value, id: &str) -> &'a Value {
        report["observations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|observation| observation["control_id"] == id)
            .unwrap()
    }

    fn obligation<'a>(report: &'a Value, id: &str) -> &'a Value {
        report["evaluation"]["obligations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|obligation| obligation["id"] == id)
            .unwrap()
    }

    fn assert_current_sources(report: &Value, fixture: &Fixture) {
        let identity = report["identity_before"].as_str().unwrap();
        assert_eq!(identity.len(), 64);
        assert!(
            identity
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        );
        assert_eq!(report["identity_before"], report["identity_after"]);
        assert_eq!(report["identity_errors"], json!([]));
        let roots = report["provenance"]["source_roots"].as_array().unwrap();
        for source in [&fixture.game, &fixture.harness] {
            let canonical = std::fs::canonicalize(source).unwrap();
            assert!(roots.contains(&json!(canonical.display().to_string())));
        }
        for observed in report["observations"].as_array().unwrap() {
            assert_eq!(observed["input_identity"], report["identity_after"]);
        }
    }

    fn assert_selected_cut(report: &Value) {
        assert_eq!(
            report["selection"],
            json!(["harness:cut:C2", "harness:health"])
        );
        assert_eq!(
            report["evaluation"]["all_automated_obligations_satisfied"],
            false
        );
        assert_eq!(report["evaluation"]["complete"], false);
    }

    #[test]
    fn completed_cut_satisfies_only_the_requested_scope_with_current_inputs() {
        let fixture = Fixture::new("completed");
        let output = fixture.run();
        assert_success(&output);
        let report = fixture.report();
        assert_eq!(report["controller_exit_code"], 0);
        assert_eq!(report["evaluation"]["selected_scope_satisfied"], true);
        assert_eq!(observation(&report, "harness:cut:C2")["outcome"], "passed");
        assert_eq!(obligation(&report, "C2")["assessment"], "satisfied");
        assert_eq!(observation(&report, "harness:health")["outcome"], "passed");
        assert_selected_cut(&report);
        assert_current_sources(&report, &fixture);
        fixture.assert_prepare_then_observe();
    }

    #[test]
    fn abort_preserves_finished_observations_and_reports_infrastructure_failure() {
        let fixture = Fixture::new("abort");
        let output = fixture.run();
        assert!(!output.status.success());
        let report = fixture.report();
        assert_eq!(report["controller_exit_code"], 2);
        assert_eq!(report["evaluation"]["selected_scope_satisfied"], false);
        assert_eq!(observation(&report, "harness:cut:C2")["outcome"], "passed");
        assert_eq!(obligation(&report, "C2")["assessment"], "satisfied");
        assert_eq!(
            observation(&report, "harness:health")["outcome"],
            "infrastructure-error"
        );
        assert!(
            observation(&report, "harness:health")["detail"]
                .as_str()
                .unwrap()
                .contains("fixture controller aborted")
        );
        assert_selected_cut(&report);
        assert_current_sources(&report, &fixture);
        fixture.assert_prepare_then_observe();
    }

    #[test]
    fn green_stdout_and_zero_exit_cannot_replace_missing_or_stale_raw_evidence() {
        for previous in [
            None,
            Some(json!({
                "schema":1, "completed":true, "infrastructure_error":null,
                "records":[{"scenario":"cut","item":"C2","status":"ok","note":"old successful run"}]
            })),
        ] {
            let fixture = Fixture::new("stdout-only");
            if let Some(previous) = previous {
                std::fs::create_dir_all(fixture.raw_path().parent().unwrap()).unwrap();
                std::fs::write(fixture.raw_path(), serde_json::to_vec(&previous).unwrap()).unwrap();
            }
            let output = fixture.run();
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stdout).contains("ALL CHECKS PASSED"));
            let report = fixture.report();
            assert_eq!(report["controller_exit_code"], 0);
            assert_eq!(report["evaluation"]["selected_scope_satisfied"], false);
            assert_eq!(obligation(&report, "C2")["assessment"], "pending");
            assert_eq!(
                observation(&report, "harness:health")["outcome"],
                "infrastructure-error"
            );
            assert!(
                !fixture.raw_path().exists(),
                "old evidence must be removed before launch"
            );
            assert_selected_cut(&report);
            assert_current_sources(&report, &fixture);
            fixture.assert_prepare_then_observe();
        }
    }

    #[test]
    fn a_result_for_another_scenario_cannot_satisfy_cut() {
        let fixture = Fixture::new("wrong-scenario");
        let output = fixture.run();
        assert!(!output.status.success());
        let report = fixture.report();
        assert_eq!(report["controller_exit_code"], 0);
        assert_eq!(report["evaluation"]["selected_scope_satisfied"], false);
        assert_eq!(observation(&report, "harness:cut:C2")["outcome"], "not-run");
        assert_eq!(obligation(&report, "C2")["assessment"], "pending");
        assert_eq!(observation(&report, "harness:health")["outcome"], "passed");
        assert_selected_cut(&report);
        assert_current_sources(&report, &fixture);
        fixture.assert_prepare_then_observe();
    }
}
