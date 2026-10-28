//! Exercise the public CLI, including error propagation through wrappers.
#[cfg(test)]
mod checks {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn fresh() -> PathBuf {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/negative-controls")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }
    fn command(dir: &Path) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_soiree"));
        command.current_dir(dir);
        command
    }
    fn success(output: &Output) {
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fn game() -> (PathBuf, PathBuf) {
        let parent = fresh();
        success(
            &command(&parent)
                .args(["new", "negative-game"])
                .output()
                .unwrap(),
        );
        let dir = parent.join("negative-game");
        let cert = dir.join("cert.json");
        let mut config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&cert).unwrap()).unwrap();
        config["ui"] = serde_json::json!({"not_applicable":{"editing":"fixture has no input","effects":"fixture has no effects","lifecycle":"fixture has page lifetime","async":"fixture has no tasks"}});
        std::fs::write(cert, serde_json::to_string(&config).unwrap()).unwrap();
        (parent, dir)
    }

    #[cfg(unix)]
    fn controlled_cargo(parent: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let bin = parent.join("bin");
        std::fs::create_dir(&bin).unwrap();
        let script = bin.join("cargo");
        std::fs::write(&script, "#!/bin/sh\ncase \"$1\" in\n metadata) printf '%s' '{\"packages\":[],\"workspace_members\":[]}' ;;\n tree) exit 0 ;;\n *) echo 'error: intentional negative control' >&2; exit 27 ;;\nesac\n").unwrap();
        std::fs::set_permissions(script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let paths = std::iter::once(bin)
            .chain(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            ))
            .collect::<Vec<_>>();
        PathBuf::from(std::env::join_paths(paths).unwrap())
    }

    #[cfg(unix)]
    #[test]
    fn public_check_rejects_stale_dom_bad_css_and_failed_subprocesses() {
        let (parent, dir) = game();
        let path = controlled_cargo(&parent);
        let check = |fast: bool| {
            let mut child = command(&dir);
            child.env("PATH", &path).arg("check");
            if fast {
                child.arg("--fast");
            }
            child.output().unwrap()
        };
        success(&check(true));
        let workflow = dir.join(".github/workflows/soiree.yml");
        let original_workflow = std::fs::read_to_string(&workflow).unwrap();
        std::fs::remove_file(&workflow).unwrap();
        let missing_ci = check(true);
        assert!(!missing_ci.status.success());
        assert!(String::from_utf8_lossy(&missing_ci.stdout).contains("CI commune absente"));
        std::fs::write(&workflow, original_workflow).unwrap();
        let failed = check(false);
        assert!(!failed.status.success());
        assert!(String::from_utf8_lossy(&failed.stdout).contains("cargo test"));
        let module = dir.join("crates/web/src/page.rs");
        let original = std::fs::read_to_string(&module).unwrap();
        std::fs::write(&module, format!("{original}\n// stale\n")).unwrap();
        assert!(!check(true).status.success());
        std::fs::write(&module, original).unwrap();
        let css = dir.join("web/style.css");
        let text = std::fs::read_to_string(&css).unwrap();
        std::fs::write(
            css,
            format!("{text}\n/* soiree-type: color length */\n.invalid {{ color:red; }}\n"),
        )
        .unwrap();
        let failed = check(true);
        assert!(!failed.status.success());
        assert!(String::from_utf8_lossy(&failed.stdout).contains("attend color"));
    }

    #[test]
    fn public_certify_preserves_a_failed_observers_exit_code() {
        let (parent, dir) = game();
        let observer = parent.join("observer");
        std::fs::create_dir(&observer).unwrap();
        std::fs::write(
            observer.join("cert.mjs"),
            "console.log('A fake green line cannot hide the failure'); process.exit(23);\n",
        )
        .unwrap();
        let result = command(&dir)
            .arg("certify")
            .arg("--harness")
            .arg(observer)
            .output()
            .unwrap();
        assert!(!result.status.success());
        let report: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("certification/rapport.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(report["passed"], false);
        assert_eq!(report["exit_code"], 23);
    }

    #[test]
    fn upgrade_dry_run_and_missing_tag_leave_sources_untouched() {
        let (parent, dir) = game();
        let repository = parent.join("unpublished.git");
        success(
            &Command::new("git")
                .args(["init", "--bare", "--quiet"])
                .arg(&repository)
                .output()
                .unwrap(),
        );
        let manifest = dir.join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest).unwrap().replace(
            "https://github.com/build-your-own-game/soiree",
            &format!("file://{}", repository.display()),
        );
        std::fs::write(&manifest, &text).unwrap();
        success(
            &command(&dir)
                .args(["upgrade", "--dry-run"])
                .output()
                .unwrap(),
        );
        assert_eq!(std::fs::read_to_string(&manifest).unwrap(), text);
        let refused = command(&dir).arg("upgrade").output().unwrap();
        assert!(!refused.status.success());
        assert!(String::from_utf8_lossy(&refused.stderr).contains("aucun fichier modifié"));
        assert_eq!(std::fs::read_to_string(&manifest).unwrap(), text);
    }

    #[test]
    fn custom_game_organisation_never_redirects_shared_dependencies_or_ci() {
        let parent = fresh();
        let output = command(&parent)
            .args(["new", "canards", "--org", "amis"])
            .output()
            .unwrap();
        success(&output);
        assert!(String::from_utf8_lossy(&output.stdout).contains("gh repo create amis/canards"));
        let dir = parent.join("canards");
        for file in ["Cargo.toml", ".github/workflows/soiree.yml", "AGENTS.md"] {
            let contents = std::fs::read_to_string(dir.join(file)).unwrap();
            assert!(
                contents.contains("https://github.com/build-your-own-game/soiree"),
                "{file}"
            );
            assert!(!contents.contains("github.com/amis/soiree"), "{file}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn published_cannot_drop_the_game_version_to_bypass_ci_and_ui() {
        let (parent, dir) = game();
        let path = controlled_cargo(&parent);
        std::fs::remove_file(dir.join("soiree.toml")).unwrap();
        std::fs::remove_file(dir.join(".github/workflows/soiree.yml")).unwrap();
        let cert = dir.join("cert.json");
        let mut config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&cert).unwrap()).unwrap();
        config.as_object_mut().unwrap().remove("ui");
        std::fs::write(cert, serde_json::to_string(&config).unwrap()).unwrap();
        let result = command(&dir)
            .env("PATH", path)
            .args(["check", "--fast", "--published"])
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stdout)
                .contains("check --published attend un jeu versionné")
        );
    }
}
