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
if (mode === 'touch-noise') {
  writeFileSync(join(game, '.DS_Store'), 'finder');
  writeFileSync(join(game, 'notes.md'), 'ce soir');
  mkdirSync(join(game, 'data'), { recursive: true });
  writeFileSync(join(game, 'data', 'journal.jsonl'), '{}');
}
if (mode === 'touch-source') writeFileSync(join(game, 'src', 'lib.rs'), 'pub const VALUE: u8 = 2;\n');
if (mode.startsWith('touch-file:')) writeFileSync(join(game, mode.slice('touch-file:'.length)), 'changed');
if (mode === 'stdout-only') {
  console.log('ALL CHECKS PASSED: this green text is not structured evidence');
  process.exit(0);
}
const load = (share) => ({ cores: 8, cpu_share: share, load_1m: 12.5 });
writeFileSync(output, JSON.stringify({
  schema: 1,
  ...(mode === 'loaded' ? { machine_load: { start: load(0.82), end: load(0.12), threshold: 0.5, state: 'loaded' } } : {}),
  ...(mode === 'calm' ? { machine_load: { start: load(0.02), end: load(0.03), threshold: 0.5, state: 'calm' } } : {}),
  completed: mode !== 'abort',
  infrastructure_error: mode === 'abort' ? 'fixture controller aborted after C2' : null,
  records: [{
    scenario: mode === 'wrong-scenario' ? 'reload' : 'cut',
    item: 'C2', status: ['partial', 'review'].includes(mode) ? mode : 'ok', note: 'fixture completed the cut observation',
  }, ...(mode.startsWith('stray-') ? [{
    scenario: 'cut', item: 'T2', status: mode.slice('stray-'.length), note: 'stray note outside its scenario',
  }] : [])],
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

        /// The game under Git, with the `/data` a real game ignores.
        fn track(&self) {
            std::fs::write(self.game.join(".gitignore"), "/data\n/target\n").unwrap();
            let init = Command::new("git")
                .current_dir(&self.game)
                .args(["init", "-q"])
                .output()
                .unwrap();
            assert_success(&init);
        }

        fn stale_lock(&self) {
            let manifest = self.game.join("Cargo.toml");
            let text = std::fs::read_to_string(&manifest).unwrap();
            std::fs::write(manifest, text.replace("0.1.0", "0.2.0")).unwrap();
        }

        /// A real test bound to the common requirement D1.
        fn bind_d1(&self, body: &str, declared_name: &str) {
            std::fs::write(
                self.game.join("src/lib.rs"),
                format!(
                    "#[cfg(test)]\nmod tests {{\n    #[test]\n    fn replay() {{ {body} }}\n}}\n"
                ),
            )
            .unwrap();
            std::fs::write(
                self.game.join("soiree.toml"),
                "name = \"observer-fixture\"\n",
            )
            .unwrap();
            std::fs::write(
                self.game.join("verification.toml"),
                format!(
                    "schema = 1\n[[checks]]\nid = \"replay\"\nkind = \"cargo-test\"\npackage = \"observer-fixture\"\ntarget_kind = \"lib\"\ntarget = \"observer_fixture\"\nname = \"{declared_name}\"\n[[bindings]]\nrequirement = \"D1\"\nchecks = [\"replay\"]\n"
                ),
            )
            .unwrap();
        }

        /// A library with these tests, and whatever else a case adds.
        fn tests(&self, tests: &str) {
            std::fs::write(
                self.game.join("src/lib.rs"),
                format!("#[cfg(test)]\nmod tests {{\n{tests}\n}}\n"),
            )
            .unwrap();
        }

        /// A second crate of the workspace, with these tests.
        fn member(&self, tests: &str) {
            let member = self.game.join("other");
            std::fs::create_dir_all(member.join("src")).unwrap();
            std::fs::write(
                member.join("Cargo.toml"),
                "[package]\nname='other-fixture'\nversion='0.1.0'\nedition='2024'\n",
            )
            .unwrap();
            std::fs::write(
                member.join("src/lib.rs"),
                format!("#[cfg(test)]\nmod tests {{\n{tests}\n}}\n"),
            )
            .unwrap();
            let manifest = self.game.join("Cargo.toml");
            let text = std::fs::read_to_string(&manifest).unwrap();
            std::fs::write(
                manifest,
                text.replace("[workspace]", "[workspace]\nmembers = ['other']"),
            )
            .unwrap();
        }

        /// The real command, with what it printed and what it reported.
        fn check(&self) -> (Output, Value) {
            let output = Command::new(env!("CARGO_BIN_EXE_soiree"))
                .current_dir(&self.game)
                .env("CARGO_NET_OFFLINE", "true")
                .arg("check")
                .output()
                .unwrap();
            let report = std::fs::read_to_string(self.game.join("certification/check.json"))
                .unwrap_or_else(|error| {
                    panic!("{error}: {}", String::from_utf8_lossy(&output.stderr))
                });
            (output, serde_json::from_str(&report).unwrap())
        }

        fn check_report(&self) -> Value {
            let output = Command::new(env!("CARGO_BIN_EXE_soiree"))
                .current_dir(&self.game)
                .env("CARGO_NET_OFFLINE", "true")
                .arg("check")
                .output()
                .unwrap();
            let report = self.game.join("certification/check.json");
            serde_json::from_str(&std::fs::read_to_string(report).unwrap_or_else(|error| {
                panic!("{error}: {}", String::from_utf8_lossy(&output.stderr))
            }))
            .unwrap()
        }

        fn check_fast(&self, extra: &[&str]) -> Output {
            Command::new(env!("CARGO_BIN_EXE_soiree"))
                .current_dir(&self.game)
                .env("CARGO_NET_OFFLINE", "true")
                .args(["check", "--fast"])
                .args(extra)
                .output()
                .unwrap()
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
    fn a_human_judgement_passes_the_command_without_counting_as_verified() {
        let fixture = Fixture::new("review");
        let output = fixture.run();
        assert_success(&output);
        let report = fixture.report();
        assert_eq!(report["evaluation"]["selected_scope_satisfied"], true);
        assert_eq!(report["evaluation"]["complete"], false);
        assert_eq!(obligation(&report, "C2")["assessment"], "review-required");
        assert!(String::from_utf8_lossy(&output.stdout).contains("À juger par un humain"));
    }

    #[test]
    fn a_bound_requirement_is_decided_by_the_real_test_it_names() {
        for (body, declared, assessment) in [
            ("assert_eq!(1, 1);", "tests::replay", "satisfied"),
            ("assert_eq!(1, 2);", "tests::replay", "failed"),
            ("assert_eq!(1, 1);", "tests::absent", "pending"),
        ] {
            let fixture = Fixture::new("completed");
            fixture.bind_d1(body, declared);
            let report = fixture.check_report();
            assert_eq!(report["plan"]["diagnostics"], json!([]), "{body}");
            assert_eq!(
                obligation(&report, "D1")["assessment"],
                assessment,
                "{body} {declared}"
            );
        }
    }

    const REAL: &str = "#[test]\nfn real() {}\n";
    const IGNORED: &str = "#[test]\n#[ignore = \"plus tard\"]\nfn red() { panic!(); }\n";

    #[test]
    fn a_suite_where_nothing_ran_cannot_pass_the_real_command() {
        for (tests, why) in [
            (IGNORED, "1 ignoré"),
            ("", "aucun test découvert"),
            (
                "#[test]\n#[ignore]\nfn a() {}\n#[test]\n#[ignore]\nfn b() {}\n",
                "2 ignoré",
            ),
        ] {
            let fixture = Fixture::new("completed");
            fixture.tests(tests);
            let (output, report) = fixture.check();
            assert!(!output.status.success(), "{tests}");
            assert_eq!(
                observation(&report, "cargo:suite")["outcome"],
                "not-run",
                "{tests}"
            );
            assert_eq!(
                obligation(&report, "cargo:suite")["assessment"],
                "pending",
                "{tests}"
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("Pas pu vérifier"), "{stdout}");
            assert!(stdout.contains(why), "{why} : {stdout}");
        }
    }

    #[test]
    fn a_crate_whose_tests_are_all_ignored_is_not_saved_by_its_neighbour() {
        let fixture = Fixture::new("completed");
        fixture.tests(REAL);
        fixture.member(IGNORED);
        let (output, report) = fixture.check();
        assert!(!output.status.success());
        assert_eq!(observation(&report, "cargo:suite")["outcome"], "not-run");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("other-fixture"), "{stdout}");
    }

    #[test]
    fn one_ignored_test_is_counted_and_left_to_a_human() {
        let fixture = Fixture::new("completed");
        fixture.tests(&format!("{REAL}{IGNORED}"));
        let (output, report) = fixture.check();
        assert_eq!(
            observation(&report, "cargo:suite")["outcome"],
            "review-required"
        );
        assert_eq!(
            obligation(&report, "cargo:suite")["assessment"],
            "review-required"
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("À juger par un humain"), "{stdout}");
        assert!(stdout.contains("1 exécuté, 1 ignoré"), "{stdout}");
        assert!(stdout.contains("tests::red"), "{stdout}");
        assert!(stdout.contains("plus tard"), "{stdout}");
    }

    #[test]
    fn a_crate_without_tests_beside_a_real_suite_is_no_hole() {
        let fixture = Fixture::new("completed");
        fixture.tests(REAL);
        fixture.member("");
        let (_, report) = fixture.check();
        assert_eq!(observation(&report, "cargo:suite")["outcome"], "passed");
        assert_eq!(
            obligation(&report, "cargo:suite")["assessment"],
            "satisfied"
        );
    }

    #[test]
    fn a_binding_to_an_inexistent_check_fails_the_real_command() {
        let fixture = Fixture::new("completed");
        fixture.bind_d1("assert_eq!(1, 1);", "tests::replay");
        let contract = fixture.game.join("verification.toml");
        let text = std::fs::read_to_string(&contract).unwrap();
        std::fs::write(
            contract,
            text.replace("checks = [\"replay\"]", "checks = [\"invente\"]"),
        )
        .unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_soiree"))
            .current_dir(&fixture.game)
            .arg("contracts")
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("contrôle inconnu invente"), "{stdout}");
    }

    #[test]
    fn a_loaded_machine_is_said_without_changing_the_verdict() {
        let fixture = Fixture::new("loaded");
        let output = fixture.run();
        assert_success(&output);
        let report = fixture.report();
        assert_eq!(report["evaluation"]["selected_scope_satisfied"], true);
        assert_eq!(obligation(&report, "C2")["assessment"], "satisfied");
        assert_eq!(report["machine_load"]["start"]["cpu_share"], 0.82);
        assert_eq!(report["machine_load"]["end"]["load_1m"], 12.5);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Machine chargée"), "{stdout}");
        assert!(stdout.contains("82 %"), "{stdout}");
        let markdown =
            std::fs::read_to_string(fixture.game.join("certification/verification.md")).unwrap();
        assert!(markdown.contains("Machine chargée"), "{markdown}");
    }

    #[test]
    fn a_calm_machine_keeps_its_measure_and_adds_no_warning() {
        let fixture = Fixture::new("calm");
        let output = fixture.run();
        assert_success(&output);
        assert_eq!(fixture.report()["machine_load"]["state"], "calm");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.contains("Machine chargée"), "{stdout}");
    }

    #[test]
    fn noise_written_during_a_run_does_not_invalidate_it() {
        let fixture = Fixture::new("touch-noise");
        fixture.track();
        let output = fixture.run();
        assert_success(&output);
        let report = fixture.report();
        assert_eq!(report["identity_errors"], json!([]));
        assert_eq!(report["identity_before"], report["identity_after"]);
        assert_eq!(report["evaluation"]["selected_scope_satisfied"], true);
    }

    #[test]
    fn a_real_source_changed_during_a_run_fails_the_real_command_and_is_named() {
        let fixture = Fixture::new("touch-source");
        fixture.track();
        let output = fixture.run();
        assert!(!output.status.success());
        let report = fixture.report();
        assert_eq!(report["evaluation"]["selected_scope_satisfied"], false);
        assert_ne!(report["identity_before"], report["identity_after"]);
        let errors = report["identity_errors"].to_string();
        assert!(errors.contains("src/lib.rs"), "{errors}");
    }

    #[test]
    fn a_followed_noise_or_a_near_note_changed_during_a_run_fails_it_by_name() {
        for (name, follow) in [("._x", true), ("x~", true), ("footnotes.txt", false)] {
            let fixture = Fixture::new(&format!("touch-file:{name}"));
            fixture.track();
            if follow {
                std::fs::write(fixture.game.join(name), "before").unwrap();
                let added = Command::new("git")
                    .current_dir(&fixture.game)
                    .args(["add", "-f", name])
                    .output()
                    .unwrap();
                assert_success(&added);
            }
            let output = fixture.run();
            assert!(!output.status.success(), "{name}");
            let errors = fixture.report()["identity_errors"].to_string();
            assert!(errors.contains(name), "{name} : {errors}");
        }
    }

    #[test]
    fn a_stale_lock_is_refreshed_before_the_snapshot_of_an_ordinary_run() {
        let fixture = Fixture::new("completed");
        fixture.stale_lock();
        assert_success(&fixture.run());
        assert_eq!(fixture.report()["identity_errors"], json!([]));

        let fixture = Fixture::new("completed");
        fixture.stale_lock();
        let _ignored_static_outcome: Output = fixture.check_fast(&[]);
        let report: Value = serde_json::from_str(
            &std::fs::read_to_string(fixture.game.join("certification/check-fast.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(report["identity_errors"], json!([]));
        assert!(
            std::fs::read_to_string(fixture.game.join("Cargo.lock"))
                .unwrap()
                .contains("0.2.0")
        );
    }

    #[test]
    fn a_locked_run_still_refuses_a_stale_lock_and_never_rewrites_it() {
        let fixture = Fixture::new("completed");
        fixture.stale_lock();
        let lock = std::fs::read_to_string(fixture.game.join("Cargo.lock")).unwrap();
        let output = fixture.check_fast(&["--locked"]);
        assert!(!output.status.success());
        let report: Value = serde_json::from_str(
            &std::fs::read_to_string(fixture.game.join("certification/check-fast.json")).unwrap(),
        )
        .unwrap();
        assert_ne!(report["identity_errors"], json!([]));
        assert_eq!(
            std::fs::read_to_string(fixture.game.join("Cargo.lock")).unwrap(),
            lock
        );
    }

    #[test]
    fn a_judgement_outside_its_scenario_is_shown_by_the_real_command() {
        let fixture = Fixture::new("stray-review");
        let output = fixture.run();
        assert_success(&output);
        let report = fixture.report();
        assert_eq!(report["evaluation"]["selected_scope_satisfied"], true);
        assert_eq!(obligation(&report, "H1")["assessment"], "review-required");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("À juger par un humain"), "{stdout}");
        assert!(
            stdout.contains("stray note outside its scenario"),
            "{stdout}"
        );
    }

    #[test]
    fn an_unfinished_check_outside_its_scenario_fails_the_real_command() {
        let fixture = Fixture::new("stray-partial");
        let output = fixture.run();
        assert!(!output.status.success());
        let report = fixture.report();
        assert_eq!(report["evaluation"]["selected_scope_satisfied"], false);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("stray note outside its scenario"),
            "{stdout}"
        );
    }

    #[test]
    fn a_check_that_could_not_finish_fails_the_real_command() {
        let fixture = Fixture::new("partial");
        let output = fixture.run();
        assert!(!output.status.success());
        let report = fixture.report();
        assert_eq!(report["evaluation"]["selected_scope_satisfied"], false);
        assert_eq!(obligation(&report, "C2")["assessment"], "pending");
        assert!(String::from_utf8_lossy(&output.stdout).contains("Pas pu vérifier"));
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
