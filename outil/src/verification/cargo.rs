//! Execute compiled libtest targets and bind observations to exact test identities.
//!
//! Cargo supplies the executable inventory; libtest supplies test discovery and
//! outcomes. Source text and a successful Cargo exit alone are not evidence that
//! an individual test ran.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

pub const BUILD_TIMEOUT_SECONDS: u64 = 900;
pub const DISCOVERY_TIMEOUT_SECONDS: u64 = 30;
pub const EXECUTION_TIMEOUT_SECONDS: u64 = 900;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TestRef {
    pub package: String,
    pub target_kind: String,
    pub target: String,
    pub name: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default = "default_features")]
    pub default_features: bool,
    #[serde(default = "default_profile")]
    pub profile: String,
}

const fn default_features() -> bool {
    true
}

fn default_profile() -> String {
    "dev".into()
}

impl TestRef {
    #[must_use]
    pub fn canonicalized(&self) -> Self {
        let mut test = self.clone();
        test.features.sort();
        test.features.dedup();
        test
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TestOutcome {
    Passed,
    Failed,
    Ignored,
    NotRun,
    InfrastructureError,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TestExecution {
    pub test: TestRef,
    pub outcome: TestOutcome,
    pub command: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SuitePhase {
    Inventory,
    Build,
    Discovery,
    Execution,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SuiteExecution {
    pub package: String,
    pub target_kind: String,
    pub target: String,
    pub features: Vec<String>,
    pub resolved_features: Vec<String>,
    pub default_features: bool,
    pub profile: String,
    pub phase: SuitePhase,
    pub outcome: TestOutcome,
    pub command: Vec<String>,
    pub detail: String,
    pub discovered: usize,
    pub executed: usize,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct RunReport {
    pub executions: Vec<TestExecution>,
    pub suites: Vec<SuiteExecution>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    pub locked: bool,
    pub all_workspace: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Config {
    package: Option<String>,
    features: Vec<String>,
    default_features: bool,
    profile: String,
}

impl Config {
    fn of(test: &TestRef, options: Options) -> Self {
        Self {
            package: (!options.all_workspace).then(|| test.package.clone()),
            features: test.features.clone(),
            default_features: test.default_features,
            profile: test.profile.clone(),
        }
    }

    fn workspace_default() -> Self {
        Self {
            package: None,
            features: Vec::new(),
            default_features: true,
            profile: default_profile(),
        }
    }

    fn suite(&self, phase: SuitePhase, outcome: TestOutcome) -> SuiteExecution {
        SuiteExecution {
            package: self.package.clone().unwrap_or_else(|| "workspace".into()),
            target_kind: "build".into(),
            target: "cargo-test".into(),
            features: self.features.clone(),
            resolved_features: Vec::new(),
            default_features: self.default_features,
            profile: self.profile.clone(),
            phase,
            outcome,
            command: Vec::new(),
            detail: String::new(),
            discovered: 0,
            executed: 0,
        }
    }
}

#[derive(Clone, Debug)]
struct Package {
    name: String,
    manifest: PathBuf,
    targets: BTreeSet<(String, String)>,
}

struct Workspace {
    packages: BTreeMap<String, Package>,
    target_directory: PathBuf,
    rust_library: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SuiteKey {
    package: String,
    target_kind: String,
    target: String,
}

impl SuiteKey {
    fn of(test: &TestRef) -> Self {
        Self {
            package: test.package.clone(),
            target_kind: test.target_kind.clone(),
            target: test.target.clone(),
        }
    }
}

struct Artifact {
    key: SuiteKey,
    executable: PathBuf,
    cwd: PathBuf,
    manifest: PathBuf,
    resolved_features: Vec<String>,
    library_paths: Vec<PathBuf>,
}

struct SuiteReport {
    suite: SuiteExecution,
    tests: BTreeMap<String, TestOutcome>,
}

pub fn run_with_suites(dir: &Path, tests: &[TestRef], options: Options) -> RunReport {
    let tests: BTreeSet<_> = tests.iter().map(TestRef::canonicalized).collect();
    let mut report = RunReport::default();
    let mut valid = Vec::new();
    for test in tests {
        if let Some(detail) = invalid_reference(&test) {
            report.executions.push(TestExecution {
                test,
                outcome: TestOutcome::NotRun,
                command: Vec::new(),
                detail,
            });
        } else {
            valid.push(test);
        }
    }
    if valid.is_empty() && !options.all_workspace {
        return report;
    }
    let inventory_command = metadata_command(options);
    let workspace = match inventory(dir, &inventory_command) {
        Ok(workspace) => workspace,
        Err(detail) => {
            let mut suite = Config::workspace_default()
                .suite(SuitePhase::Inventory, TestOutcome::InfrastructureError);
            suite.command.clone_from(&inventory_command);
            suite.detail.clone_from(&detail);
            report.suites.push(suite);
            for test in valid {
                report.executions.push(TestExecution {
                    test,
                    outcome: TestOutcome::InfrastructureError,
                    command: inventory_command.clone(),
                    detail: detail.clone(),
                });
            }
            return report;
        }
    };
    let mut configurations: BTreeMap<Config, Vec<TestRef>> = BTreeMap::new();
    if options.all_workspace {
        configurations.insert(Config::workspace_default(), Vec::new());
    }
    for test in valid {
        if !workspace.packages.values().any(|package| {
            package.name == test.package
                && package
                    .targets
                    .contains(&(test.target_kind.clone(), test.target.clone()))
        }) {
            report.executions.push(TestExecution {
                test,
                outcome: TestOutcome::NotRun,
                command: inventory_command.clone(),
                detail: "required package or target does not exist in the Cargo workspace".into(),
            });
            continue;
        }
        configurations
            .entry(Config::of(&test, options))
            .or_default()
            .push(test);
    }
    for (config, references) in configurations {
        run_configuration(dir, &workspace, &config, &references, options, &mut report);
    }
    report.executions.sort_by(|a, b| a.test.cmp(&b.test));
    report
}

fn invalid_reference(test: &TestRef) -> Option<String> {
    if test.package.is_empty() || test.target.is_empty() || test.name.is_empty() {
        return Some("test reference requires a package, target and complete test name".into());
    }
    if !matches!(test.target_kind.as_str(), "lib" | "bin" | "test") {
        return Some(format!(
            "unsupported test target kind: {}",
            test.target_kind
        ));
    }
    if test.profile.is_empty()
        || test.features.iter().any(|feature| {
            feature.is_empty() || feature.chars().any(|c| c.is_whitespace() || c == ',')
        })
    {
        return Some("invalid Cargo profile or feature name in test reference".into());
    }
    None
}

fn metadata_command(options: Options) -> Vec<String> {
    let mut command = vec![
        "cargo".into(),
        "metadata".into(),
        "--no-deps".into(),
        "--format-version=1".into(),
    ];
    if options.locked {
        command.push("--locked".into());
    }
    command
}

fn execute(dir: &Path, command: &[String], timeout_seconds: u64) -> Result<Output, String> {
    execute_with_env(dir, command, timeout_seconds, &BTreeMap::new())
}

#[expect(
    clippy::disallowed_methods,
    reason = "the native verification controller needs a wall-clock deadline for child processes"
)]
fn execute_with_env(
    dir: &Path,
    command: &[String],
    timeout_seconds: u64,
    environment: &BTreeMap<&str, OsString>,
) -> Result<Output, String> {
    let Some((program, args)) = command.split_first() else {
        return Err("empty test runner command".into());
    };
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(dir)
        .envs(environment)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("{program}: {error}"))?;
    let stdout = child.stdout.take().ok_or("test runner stdout missing")?;
    let stderr = child.stderr.take().ok_or("test runner stderr missing")?;
    let read = |mut stream: Box<dyn std::io::Read + Send>| {
        std::thread::Builder::new().spawn(move || {
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).map(|_| bytes)
        })
    };
    let readers = if let (Ok(stdout), Ok(stderr)) = (read(Box::new(stdout)), read(Box::new(stderr)))
    {
        (stdout, stderr)
    } else {
        terminate(&mut child);
        return Err("could not start test output readers".into());
    };
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if started.elapsed() < Duration::from_secs(timeout_seconds) => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(None) => {
                break Err(format!(
                    "{program}: timeout after {timeout_seconds} seconds"
                ));
            }
            Err(error) => break Err(format!("{program}: could not wait for process: {error}")),
        }
    };
    // A target may leave children holding stdout open even after its own exit.
    // Terminate the complete group before joining its pipe readers.
    terminate(&mut child);
    let stdout = readers
        .0
        .join()
        .map_err(|_| "test stdout reader panicked")?
        .map_err(|error| format!("test stdout: {error}"))?;
    let stderr = readers
        .1
        .join()
        .map_err(|_| "test stderr reader panicked")?
        .map_err(|error| format!("test stderr: {error}"))?;
    status.map(|status| Output {
        status,
        stdout,
        stderr,
    })
}

fn terminate(child: &mut Child) {
    #[cfg(unix)]
    {
        let _: std::io::Result<_> = Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{}", child.id())])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(windows)]
    {
        let _: std::io::Result<_> = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _: std::io::Result<()> = child.kill();
    let _: std::io::Result<_> = child.wait();
}

fn output_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn inventory(dir: &Path, command: &[String]) -> Result<Workspace, String> {
    let output = execute(dir, command, DISCOVERY_TIMEOUT_SECONDS)?;
    if !output.status.success() {
        return Err(format!(
            "Cargo workspace inventory failed: {}",
            output_text(&output)
        ));
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid Cargo workspace inventory: {error}"))?;
    let members: BTreeSet<_> = metadata
        .get("workspace_members")
        .and_then(Value::as_array)
        .ok_or("Cargo workspace_members missing")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let mut packages = BTreeMap::new();
    for package in metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or("Cargo packages missing")?
    {
        let id = package
            .get("id")
            .and_then(Value::as_str)
            .ok_or("Cargo package id missing")?;
        if !members.contains(id) {
            continue;
        }
        let mut targets = BTreeSet::new();
        for target in package
            .get("targets")
            .and_then(Value::as_array)
            .ok_or("Cargo targets missing")?
        {
            if let Some(kind) = target_kind(target) {
                let name = target
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or("Cargo target name missing")?;
                targets.insert((kind.to_owned(), name.to_owned()));
            }
        }
        packages.insert(
            id.to_owned(),
            Package {
                name: package
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or("Cargo package name missing")?
                    .to_owned(),
                manifest: PathBuf::from(
                    package
                        .get("manifest_path")
                        .and_then(Value::as_str)
                        .ok_or("Cargo manifest_path missing")?,
                ),
                targets,
            },
        );
    }
    if packages.len() != members.len() {
        return Err("Cargo workspace inventory is incomplete".into());
    }
    let target_directory = metadata
        .get("target_directory")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or("Cargo target_directory missing")?;
    let compiler = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let compiler_command = vec![compiler, "--print=target-libdir".into()];
    let output = execute(dir, &compiler_command, DISCOVERY_TIMEOUT_SECONDS)?;
    if !output.status.success() {
        return Err(format!(
            "compiler runtime inventory failed: {}",
            output_text(&output)
        ));
    }
    let rust_library = PathBuf::from(
        std::str::from_utf8(&output.stdout)
            .map_err(|error| format!("invalid compiler runtime path: {error}"))?
            .trim(),
    );
    if !rust_library.is_absolute() {
        return Err("compiler runtime library directory must be absolute".into());
    }
    Ok(Workspace {
        packages,
        target_directory,
        rust_library,
    })
}

fn build_command(config: &Config, references: &[TestRef], options: Options) -> Vec<String> {
    let mut command = vec![
        "cargo".into(),
        "test".into(),
        "--no-run".into(),
        "--message-format=json".into(),
        format!("--profile={}", config.profile),
    ];
    if let Some(package) = &config.package {
        command.push(format!("--package={package}"));
    } else {
        command.push("--workspace".into());
    }
    if !config.default_features {
        command.push("--no-default-features".into());
    }
    if !config.features.is_empty() {
        command.push(format!("--features={}", config.features.join(",")));
    }
    if options.locked {
        command.push("--locked".into());
    }
    if !options.all_workspace {
        let targets: BTreeSet<_> = references
            .iter()
            .map(|test| (test.target_kind.as_str(), test.target.as_str()))
            .collect();
        for (kind, name) in targets {
            command.push(if kind == "lib" {
                "--lib".into()
            } else {
                format!("--{kind}={name}")
            });
        }
    }
    command
}

fn run_configuration(
    dir: &Path,
    workspace: &Workspace,
    config: &Config,
    references: &[TestRef],
    options: Options,
    report: &mut RunReport,
) {
    let command = build_command(config, references, options);
    let output = execute(dir, &command, BUILD_TIMEOUT_SECONDS);
    let artifacts = match output {
        Ok(output) if output.status.success() => artifacts(workspace, config, &output.stdout),
        Ok(output) => {
            let rust_error = String::from_utf8_lossy(&output.stdout).lines().any(|line| {
                serde_json::from_str::<Value>(line).is_ok_and(|value| {
                    value.get("reason").and_then(Value::as_str) == Some("compiler-message")
                        && value.pointer("/message/level").and_then(Value::as_str) == Some("error")
                })
            });
            let outcome = if rust_error {
                TestOutcome::Failed
            } else {
                TestOutcome::InfrastructureError
            };
            let detail = format!("Cargo test compilation failed: {}", output_text(&output));
            build_failed(config, references, &command, &detail, outcome, report);
            return;
        }
        Err(detail) => {
            build_failed(
                config,
                references,
                &command,
                &detail,
                TestOutcome::InfrastructureError,
                report,
            );
            return;
        }
    };
    let artifacts = match artifacts {
        Ok(artifacts) => artifacts,
        Err(detail) => {
            build_failed(
                config,
                references,
                &command,
                &detail,
                TestOutcome::InfrastructureError,
                report,
            );
            return;
        }
    };
    let mut suites = BTreeMap::new();
    for artifact in artifacts {
        if !options.all_workspace
            && !references
                .iter()
                .any(|test| SuiteKey::of(test) == artifact.key)
        {
            continue;
        }
        let suite = run_suite(&artifact, config);
        report.suites.push(suite.suite.clone());
        suites.insert(artifact.key, suite);
    }
    for test in references {
        let Some(suite) = suites.get(&SuiteKey::of(test)) else {
            report.executions.push(TestExecution {
                test: test.clone(),
                outcome: TestOutcome::NotRun,
                command: command.clone(),
                detail: "required target was not produced by Cargo for this configuration".into(),
            });
            continue;
        };
        let outcome = suite.tests.get(&test.name).copied().unwrap_or_else(|| {
            if suite.suite.outcome == TestOutcome::InfrastructureError {
                TestOutcome::InfrastructureError
            } else {
                TestOutcome::NotRun
            }
        });
        let detail = if suite.tests.contains_key(&test.name) {
            format!("{}: {outcome:?}", test.name)
        } else {
            format!(
                "required test was not observed: {}\n{}",
                test.name, suite.suite.detail
            )
        };
        report.executions.push(TestExecution {
            test: test.clone(),
            outcome,
            command: suite.suite.command.clone(),
            detail,
        });
    }
}

fn build_failed(
    config: &Config,
    references: &[TestRef],
    command: &[String],
    detail: &str,
    outcome: TestOutcome,
    report: &mut RunReport,
) {
    let mut suite = config.suite(SuitePhase::Build, outcome);
    command.clone_into(&mut suite.command);
    detail.clone_into(&mut suite.detail);
    report.suites.push(suite);
    for test in references {
        report.executions.push(TestExecution {
            test: test.clone(),
            outcome: if outcome == TestOutcome::InfrastructureError {
                outcome
            } else {
                TestOutcome::NotRun
            },
            command: command.to_owned(),
            detail: detail.to_owned(),
        });
    }
}

fn target_kind(target: &Value) -> Option<&'static str> {
    let kinds = target.get("kind")?.as_array()?;
    if kinds.iter().any(|kind| kind == "test") {
        Some("test")
    } else if kinds.iter().any(|kind| kind == "bin") {
        Some("bin")
    } else if kinds.iter().any(|kind| {
        matches!(
            kind.as_str(),
            Some("lib" | "proc-macro" | "rlib" | "dylib" | "cdylib" | "staticlib")
        )
    }) {
        Some("lib")
    } else {
        None
    }
}

fn artifacts(
    workspace: &Workspace,
    config: &Config,
    output: &[u8],
) -> Result<Vec<Artifact>, String> {
    let mut artifacts: BTreeMap<SuiteKey, Artifact> = BTreeMap::new();
    let messages: Vec<Value> = String::from_utf8_lossy(output)
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .map_err(|error| format!("invalid Cargo JSON message: {error}"))
        })
        .collect::<Result<_, _>>()?;
    let native_paths = native_library_paths(&messages, &workspace.target_directory);
    for value in &messages {
        if value.get("reason").and_then(Value::as_str) != Some("compiler-artifact")
            || value.pointer("/profile/test").and_then(Value::as_bool) != Some(true)
        {
            continue;
        }
        let Some(package) = value
            .get("package_id")
            .and_then(Value::as_str)
            .and_then(|id| workspace.packages.get(id))
        else {
            continue;
        };
        if config
            .package
            .as_ref()
            .is_some_and(|name| name != &package.name)
        {
            continue;
        }
        let Some(executable) = value.get("executable").and_then(Value::as_str) else {
            continue;
        };
        let Some(kind) = value.get("target").and_then(target_kind) else {
            continue;
        };
        let key = SuiteKey {
            package: package.name.clone(),
            target_kind: kind.into(),
            target: value
                .pointer("/target/name")
                .and_then(Value::as_str)
                .ok_or("Cargo artifact target name missing")?
                .into(),
        };
        let path = PathBuf::from(executable);
        let directory = path.parent().ok_or("test executable directory missing")?;
        let mut library_paths = native_paths.clone();
        library_paths.push(directory.to_owned());
        if let Some(base) = directory.parent() {
            library_paths.push(base.to_owned());
        }
        library_paths.push(workspace.rust_library.clone());
        if artifacts
            .get(&key)
            .is_some_and(|previous| previous.executable != path)
        {
            return Err(format!("ambiguous Cargo test artifact for {key:?}"));
        }
        artifacts.insert(
            key.clone(),
            Artifact {
                key,
                executable: path,
                cwd: package
                    .manifest
                    .parent()
                    .ok_or("package directory missing")?
                    .to_path_buf(),
                manifest: package.manifest.clone(),
                resolved_features: value
                    .get("features")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
                library_paths,
            },
        );
    }
    Ok(artifacts.into_values().collect())
}

fn native_library_paths(messages: &[Value], target_directory: &Path) -> Vec<PathBuf> {
    messages
        .iter()
        .filter(|message| {
            message.get("reason").and_then(Value::as_str) == Some("build-script-executed")
        })
        .flat_map(|message| {
            message
                .get("linked_paths")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(Value::as_str)
        .map(|path| match path.split_once('=') {
            Some(("native" | "framework" | "dependency" | "crate" | "all", value)) => value,
            Some(_) | None => path,
        })
        .map(PathBuf::from)
        .filter(|path| path.starts_with(target_directory))
        .collect()
}

// Follow Cargo's runtime library lookup, rather than relying on the caller
// having launched soiree through Cargo with a suitable loader environment.
// https://doc.rust-lang.org/cargo/reference/environment-variables.html#dynamic-library-paths
fn runtime_environment(artifact: &Artifact) -> Result<BTreeMap<&'static str, OsString>, String> {
    let variable = if cfg!(target_os = "macos") {
        "DYLD_FALLBACK_LIBRARY_PATH"
    } else if cfg!(windows) {
        "PATH"
    } else if cfg!(target_os = "aix") {
        "LIBPATH"
    } else {
        "LD_LIBRARY_PATH"
    };
    let mut paths = artifact.library_paths.clone();
    if let Some(existing) = std::env::var_os(variable) {
        paths.extend(std::env::split_paths(&existing));
    } else if cfg!(target_os = "macos") {
        if let Some(home) = std::env::var_os("HOME") {
            paths.push(PathBuf::from(home).join("lib"));
        }
        paths.extend([PathBuf::from("/usr/local/lib"), PathBuf::from("/usr/lib")]);
    }
    let value =
        std::env::join_paths(paths).map_err(|error| format!("runtime library path: {error}"))?;
    Ok(BTreeMap::from([
        (variable, value),
        ("CARGO_MANIFEST_DIR", artifact.cwd.as_os_str().to_owned()),
        (
            "CARGO_MANIFEST_PATH",
            artifact.manifest.as_os_str().to_owned(),
        ),
    ]))
}

fn custom_harness(artifact: &Artifact) -> Result<bool, String> {
    let text = std::fs::read_to_string(&artifact.manifest)
        .map_err(|error| format!("{}: {error}", artifact.manifest.display()))?;
    let document: toml::Value = toml::from_str(&text)
        .map_err(|error| format!("{}: {error}", artifact.manifest.display()))?;
    let Some(section) = document.get(&artifact.key.target_kind) else {
        return Ok(false);
    };
    if artifact.key.target_kind == "lib" {
        return Ok(section.get("harness").and_then(toml::Value::as_bool) == Some(false));
    }
    Ok(section.as_array().into_iter().flatten().any(|target| {
        let name = target.get("name").and_then(toml::Value::as_str);
        let matching = name.is_none_or(|name| name == artifact.key.target);
        matching && target.get("harness").and_then(toml::Value::as_bool) == Some(false)
    }))
}

fn run_suite(artifact: &Artifact, config: &Config) -> SuiteReport {
    let mut suite = config.suite(SuitePhase::Discovery, TestOutcome::InfrastructureError);
    suite.package.clone_from(&artifact.key.package);
    suite.target_kind.clone_from(&artifact.key.target_kind);
    suite.target.clone_from(&artifact.key.target);
    suite
        .resolved_features
        .clone_from(&artifact.resolved_features);
    suite.command = vec![
        artifact.executable.to_string_lossy().into_owned(),
        "--test".into(),
        "--list".into(),
        "--format=terse".into(),
    ];
    match custom_harness(artifact) {
        Ok(false) => {}
        Ok(true) => {
            suite.detail = "custom test harness requires an explicit runner adapter".into();
            return SuiteReport {
                suite,
                tests: BTreeMap::new(),
            };
        }
        Err(detail) => {
            suite.detail = detail;
            return SuiteReport {
                suite,
                tests: BTreeMap::new(),
            };
        }
    }
    let environment = match runtime_environment(artifact) {
        Ok(environment) => environment,
        Err(detail) => {
            suite.detail = detail;
            return SuiteReport {
                suite,
                tests: BTreeMap::new(),
            };
        }
    };
    let listed = match execute_with_env(
        &artifact.cwd,
        &suite.command,
        DISCOVERY_TIMEOUT_SECONDS,
        &environment,
    ) {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            suite.detail = format!("libtest discovery failed: {}", output_text(&output));
            return SuiteReport {
                suite,
                tests: BTreeMap::new(),
            };
        }
        Err(detail) => {
            suite.detail = detail;
            return SuiteReport {
                suite,
                tests: BTreeMap::new(),
            };
        }
    };
    let discovered: BTreeSet<_> = String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter_map(|line| line.strip_suffix(": test").map(str::to_owned))
        .collect();
    suite.discovered = discovered.len();
    suite.phase = SuitePhase::Execution;
    suite.command = vec![
        artifact.executable.to_string_lossy().into_owned(),
        "--test".into(),
        "--format=pretty".into(),
        "--color=never".into(),
    ];
    match execute_with_env(
        &artifact.cwd,
        &suite.command,
        EXECUTION_TIMEOUT_SECONDS,
        &environment,
    ) {
        Ok(output) => completed_suite(suite, &output, &discovered),
        Err(detail) => {
            suite.detail = detail;
            SuiteReport {
                suite,
                tests: BTreeMap::new(),
            }
        }
    }
}

fn completed_suite(
    mut suite: SuiteExecution,
    output: &Output,
    discovered: &BTreeSet<String>,
) -> SuiteReport {
    let mut tests = parse_results(&output.stdout, discovered);
    suite.executed = tests
        .values()
        .filter(|outcome| matches!(outcome, TestOutcome::Passed | TestOutcome::Failed))
        .count();
    suite.outcome = if !output.status.success()
        || tests
            .values()
            .any(|outcome| *outcome == TestOutcome::Failed)
    {
        TestOutcome::Failed
    } else if discovered.is_empty() {
        TestOutcome::NotRun
    } else if tests.len() != discovered.len() || !valid_summary(&output.stdout, &tests) {
        TestOutcome::InfrastructureError
    } else if tests
        .values()
        .all(|outcome| *outcome == TestOutcome::Ignored)
    {
        TestOutcome::Ignored
    } else {
        TestOutcome::Passed
    };
    if suite.outcome == TestOutcome::InfrastructureError {
        for outcome in tests.values_mut() {
            *outcome = TestOutcome::InfrastructureError;
        }
    }
    suite.detail = output_text(output);
    SuiteReport { suite, tests }
}

fn parse_results(output: &[u8], discovered: &BTreeSet<String>) -> BTreeMap<String, TestOutcome> {
    let mut tests = BTreeMap::new();
    for line in String::from_utf8_lossy(output).lines() {
        let Some((name, status)) = line
            .strip_prefix("test ")
            .and_then(|line| line.rsplit_once(" ... "))
        else {
            continue;
        };
        if !discovered.contains(name) {
            continue;
        }
        let outcome = match status.split_whitespace().next() {
            Some("ok") => TestOutcome::Passed,
            Some("FAILED") => TestOutcome::Failed,
            Some("ignored" | "ignored,") => TestOutcome::Ignored,
            _ => continue,
        };
        if tests.insert(name.to_owned(), outcome).is_some() {
            tests.insert(name.to_owned(), TestOutcome::InfrastructureError);
        }
    }
    tests
}

fn valid_summary(output: &[u8], tests: &BTreeMap<String, TestOutcome>) -> bool {
    let text = String::from_utf8_lossy(output);
    let summaries: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("test result: "))
        .collect();
    let [summary] = summaries.as_slice() else {
        return false;
    };
    let Some((_, counts)) = summary.split_once(". ") else {
        return false;
    };
    let mut observed = BTreeMap::new();
    for part in counts.split(';') {
        let mut words = part.split_whitespace();
        let Some(count) = words.next().and_then(|value| value.parse::<usize>().ok()) else {
            continue;
        };
        let Some(label) = words.next() else {
            continue;
        };
        observed.insert(label, count);
    }
    [
        ("passed", TestOutcome::Passed),
        ("failed", TestOutcome::Failed),
        ("ignored", TestOutcome::Ignored),
    ]
    .iter()
    .all(|(label, outcome)| {
        observed.get(label).copied()
            == Some(tests.values().filter(|result| *result == outcome).count())
    }) && observed.get("filtered").copied() == Some(0)
        && !tests
            .values()
            .any(|outcome| *outcome == TestOutcome::InfrastructureError)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "soiree-libtest-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(root.join("src")).unwrap();
            std::fs::write(
                root.join("Cargo.toml"),
                r#"
[package]
name = "libtest-fixture"
version = "0.1.0"
edition = "2024"
[workspace]
[features]
default = ["standard"]
standard = []
special = []
broken = []
[profile.contract-test]
inherits = "dev"
"#,
            )
            .unwrap();
            std::fs::write(
                root.join("src/lib.rs"),
                r#"
pub fn not_a_test() {}
#[cfg(feature = "broken")]
compile_error!("negative compilation control");
#[cfg(test)]
mod tests {
    #[test]
    fn passes() { assert_eq!(2 + 2, 4); }
    #[test]
    #[ignore = "intentional negative control"]
    fn ignored() { panic!("must remain ignored"); }
    #[test]
    fn count_execution() {
        use std::io::Write;
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/executions.txt");
        let mut file = std::fs::OpenOptions::new().append(true).create(true).open(path).unwrap();
        writeln!(file, "executed").unwrap();
    }
    #[test]
    #[cfg(feature = "standard")]
    fn standard_feature() {}
    #[test]
    #[cfg(feature = "special")]
    fn special_feature() {}
}
"#,
            )
            .unwrap();
            Self { root }
        }

        fn reference(name: &str) -> TestRef {
            TestRef {
                package: "libtest-fixture".into(),
                target_kind: "lib".into(),
                target: "libtest_fixture".into(),
                name: name.into(),
                features: Vec::new(),
                default_features: true,
                profile: "dev".into(),
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _: std::io::Result<()> = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn only_discovered_executed_tests_satisfy_exact_references() {
        let fixture = Fixture::new();
        let cases = [
            ("tests::passes", TestOutcome::Passed),
            ("tests::ignored", TestOutcome::Ignored),
            ("not_a_test", TestOutcome::NotRun),
            ("other_module::passes", TestOutcome::NotRun),
            ("", TestOutcome::NotRun),
        ];
        let mut references: Vec<_> = cases
            .iter()
            .map(|(name, _)| Fixture::reference(name))
            .collect();
        references.push(Fixture::reference("tests::passes"));
        let mut wrong_target = Fixture::reference("tests::passes");
        wrong_target.target = "missing_target".into();
        references.push(wrong_target.clone());
        let report = run_with_suites(&fixture.root, &references, Options::default());
        for (name, expected) in cases {
            let result = report
                .executions
                .iter()
                .find(|result| result.test.name == name)
                .unwrap();
            assert_eq!(result.outcome, expected, "{name}: {}", result.detail);
        }
        assert_eq!(
            report
                .executions
                .iter()
                .find(|result| result.test == wrong_target)
                .unwrap()
                .outcome,
            TestOutcome::NotRun
        );
        assert_eq!(report.suites.len(), 1, "one target must run only once");
        assert_eq!(
            report.suites.first().unwrap().outcome,
            TestOutcome::Passed,
            "{}",
            report.suites.first().unwrap().detail
        );
        assert_eq!(
            std::fs::read_to_string(fixture.root.join("target/executions.txt")).unwrap(),
            "executed\n"
        );
    }

    #[test]
    fn workspace_run_records_failures_in_unreferenced_targets() {
        let fixture = Fixture::new();
        std::fs::create_dir(fixture.root.join("tests")).unwrap();
        std::fs::write(
            fixture.root.join("tests/unreferenced.rs"),
            "#[test]\nfn negative_control() { assert_eq!(1, 2); }\n",
        )
        .unwrap();
        let reference = Fixture::reference("tests::passes");
        let report = run_with_suites(
            &fixture.root,
            &[reference],
            Options {
                locked: false,
                all_workspace: true,
            },
        );
        assert_eq!(
            report.executions.first().unwrap().outcome,
            TestOutcome::Passed
        );
        let failed = report
            .suites
            .iter()
            .find(|suite| suite.target == "unreferenced")
            .unwrap();
        assert_eq!(failed.outcome, TestOutcome::Failed, "{}", failed.detail);
        assert_eq!(failed.executed, 1);
        assert_eq!(
            report
                .suites
                .iter()
                .filter(|suite| suite.target_kind == "lib")
                .count(),
            1
        );
    }

    #[test]
    fn configuration_and_target_kind_are_part_of_test_identity() {
        let fixture = Fixture::new();
        let mut special = Fixture::reference("tests::special_feature");
        special.features = vec!["special".into(), "special".into()];
        special.default_features = false;
        special.profile = "contract-test".into();
        let mut standard_disabled = special.clone();
        standard_disabled.name = "tests::standard_feature".into();
        let mut wrong_kind = special.clone();
        wrong_kind.target_kind = "test".into();
        let report = run_with_suites(
            &fixture.root,
            &[
                special.canonicalized(),
                standard_disabled.clone(),
                wrong_kind.clone(),
            ],
            Options::default(),
        );
        assert_eq!(
            report
                .executions
                .iter()
                .find(|result| result.test.name == special.name && result.test.target_kind == "lib")
                .unwrap()
                .outcome,
            TestOutcome::Passed
        );
        assert_eq!(
            report
                .executions
                .iter()
                .find(|result| result.test.name == standard_disabled.name)
                .unwrap()
                .outcome,
            TestOutcome::NotRun
        );
        assert_eq!(
            report
                .executions
                .iter()
                .find(|result| result.test.target_kind == wrong_kind.target_kind)
                .unwrap()
                .outcome,
            TestOutcome::NotRun
        );
        assert_eq!(report.suites.len(), 1);
        let suite = report.suites.first().unwrap();
        assert_eq!(suite.features, vec!["special"]);
        assert_eq!(suite.resolved_features, vec!["special"]);
        assert!(!suite.default_features);
        assert_eq!(suite.profile, "contract-test");
    }

    #[test]
    fn compilation_failure_is_not_a_successful_or_executed_test() {
        let fixture = Fixture::new();
        let mut reference = Fixture::reference("tests::passes");
        reference.features.push("broken".into());
        let report = run_with_suites(&fixture.root, &[reference], Options::default());
        assert_eq!(
            report.executions.first().unwrap().outcome,
            TestOutcome::NotRun
        );
        let suite = report.suites.first().unwrap();
        assert_eq!(suite.phase, SuitePhase::Build);
        assert_eq!(suite.outcome, TestOutcome::Failed, "{}", suite.detail);
    }

    #[test]
    fn zero_discovered_tests_never_satisfy_a_reference() {
        let fixture = Fixture::new();
        std::fs::write(fixture.root.join("src/lib.rs"), "pub fn not_a_test() {}\n").unwrap();
        let report = run_with_suites(
            &fixture.root,
            &[Fixture::reference("not_a_test")],
            Options::default(),
        );
        assert_eq!(
            report.executions.first().unwrap().outcome,
            TestOutcome::NotRun
        );
        let suite = report.suites.first().unwrap();
        assert_eq!(suite.outcome, TestOutcome::NotRun);
        assert_eq!(suite.discovered, 0);
        assert_eq!(suite.executed, 0);
    }

    #[test]
    fn a_custom_harness_cannot_impersonate_libtest() {
        let fixture = Fixture::new();
        let manifest = fixture.root.join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest).unwrap();
        std::fs::write(
            manifest,
            format!("{text}\n[[test]]\nname = \"custom\"\nharness = false\n"),
        )
        .unwrap();
        std::fs::create_dir(fixture.root.join("tests")).unwrap();
        std::fs::write(
            fixture.root.join("tests/custom.rs"),
            "fn main() { println!(\"test fake ... ok\"); }\n",
        )
        .unwrap();
        let mut reference = Fixture::reference("fake");
        reference.target_kind = "test".into();
        reference.target = "custom".into();
        let report = run_with_suites(&fixture.root, &[reference], Options::default());
        assert_eq!(
            report.executions.first().unwrap().outcome,
            TestOutcome::InfrastructureError
        );
        assert!(
            report
                .suites
                .first()
                .unwrap()
                .detail
                .contains("custom test harness")
        );
    }

    #[test]
    fn dynamic_dependencies_and_member_runtime_paths_are_available() {
        let fixture = Fixture::new();
        let manifest = fixture.root.join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest).unwrap();
        std::fs::write(
            manifest,
            format!("{text}\n[dependencies]\ndynamic = {{ path = \"dynamic\" }}\n"),
        )
        .unwrap();
        std::fs::create_dir_all(fixture.root.join("dynamic/src")).unwrap();
        std::fs::write(
            fixture.root.join("dynamic/Cargo.toml"),
            r#"
[package]
name = "dynamic"
version = "0.0.0"
edition = "2024"
[lib]
crate-type = ["dylib"]
"#,
        )
        .unwrap();
        std::fs::write(
            fixture.root.join("dynamic/src/lib.rs"),
            "pub fn value() -> u8 { 42 }\n",
        )
        .unwrap();
        let source = fixture.root.join("src/lib.rs");
        let text = std::fs::read_to_string(&source).unwrap();
        std::fs::write(source, format!("{text}\n#[test]\nfn dynamic_lookup() {{ assert_eq!(dynamic::value(), 42); assert_eq!(std::env::var(\"CARGO_MANIFEST_DIR\").unwrap(), env!(\"CARGO_MANIFEST_DIR\")); }}\n")).unwrap();
        let report = run_with_suites(
            &fixture.root,
            &[Fixture::reference("dynamic_lookup")],
            Options::default(),
        );
        let observed = report.executions.first().unwrap();
        assert_eq!(observed.outcome, TestOutcome::Passed, "{}", observed.detail);
        assert!(
            report
                .suites
                .iter()
                .all(|suite| suite.outcome == TestOutcome::Passed)
        );
    }

    #[test]
    fn native_loader_paths_are_scoped_to_cargo_output() {
        let messages = vec![serde_json::json!({
            "reason": "build-script-executed",
            "linked_paths": ["native=/work/target/debug/build/native", "/work/target/debug/build/plain", "native=/outside/untrusted"]
        })];
        assert_eq!(
            native_library_paths(&messages, Path::new("/work/target")),
            vec![
                PathBuf::from("/work/target/debug/build/native"),
                PathBuf::from("/work/target/debug/build/plain"),
            ]
        );
    }

    #[test]
    fn summary_counts_and_duplicate_outcomes_are_checked() {
        let names = BTreeSet::from(["tests::one".into()]);
        let output = b"test tests::one ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0s\n";
        let tests = parse_results(output, &names);
        assert!(valid_summary(output, &tests));
        assert!(!valid_summary(b"test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0s\n", &tests));
        let duplicate = b"test tests::one ... ok\ntest tests::one ... ok\n";
        assert_eq!(
            parse_results(duplicate, &names)["tests::one"],
            TestOutcome::InfrastructureError
        );
    }

    #[cfg(unix)]
    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "this test checks the native controller's wall-clock timeout"
    )]
    fn timeout_terminates_children_that_hold_output_open() {
        let fixture = Fixture::new();
        let command = vec!["/bin/sh".into(), "-c".into(), "sleep 60 & wait".into()];
        let started = Instant::now();
        let error = execute(&fixture.root, &command, 1).unwrap_err();
        assert!(error.contains("timeout"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
