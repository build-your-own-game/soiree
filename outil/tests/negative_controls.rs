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
        let metadata = serde_json::json!({"packages":[], "workspace_members":[], "workspace_root":parent.join("negative-game"), "resolve":{"nodes":[]}});
        std::fs::write(
            bin.join("metadata.json"),
            serde_json::to_string(&metadata).unwrap(),
        )
        .unwrap();
        std::fs::write(&script, "#!/bin/sh\ncase \"$1\" in\n -V) echo cargo-fixture ;;\n metadata) cat \"${0%/*}/metadata.json\" ;;\n tree) exit 0 ;;\n *) echo 'error: intentional negative control' >&2; exit 27 ;;\nesac\n").unwrap();
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
        let observed: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("certification/check.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(observed["evaluation"]["selected_scope_satisfied"], false);
        assert!(
            observed["observations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|o| o["control_id"] == "cargo:suite" && o["outcome"] != "passed")
        );
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
            &std::fs::read_to_string(dir.join("certification/verification.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(report["evaluation"]["selected_scope_satisfied"], false);
        assert_eq!(report["controller_exit_code"], 23);
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

    fn shared_repository(parent: &Path, name: &str, target: Option<&str>) -> PathBuf {
        let repository = parent.join(name);
        std::fs::create_dir(&repository).unwrap();
        std::fs::write(
            repository.join("Cargo.toml"),
            "[workspace]\nresolver = \"3\"\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();
        for package in [
            "salon-core",
            "salon-host",
            "salon-web",
            "remote-web",
            "remote-protocol",
        ] {
            let directory = repository.join("crates").join(package);
            std::fs::create_dir_all(directory.join("src")).unwrap();
            std::fs::write(
                directory.join("Cargo.toml"),
                format!(
                    "[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"
                ),
            )
            .unwrap();
            std::fs::write(
                directory.join("src/lib.rs"),
                "#![no_std]\npub fn ready() {}\n",
            )
            .unwrap();
        }
        for args in [
            vec!["init", "--quiet"],
            vec!["add", "."],
            vec![
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.test",
                "commit",
                "--quiet",
                "-m",
                "shared fixture",
            ],
            vec!["tag", "v0.1.0"],
        ] {
            success(
                &Command::new("git")
                    .current_dir(&repository)
                    .args(args)
                    .output()
                    .unwrap(),
            );
        }
        if let Some(target) = target {
            success(
                &Command::new("git")
                    .current_dir(&repository)
                    .args(["tag", target])
                    .output()
                    .unwrap(),
            );
        }
        repository
    }

    fn workspace_upgrade_game(root_repository: &Path, member_repository: &Path) -> PathBuf {
        let (_, dir) = game();
        let root_url = format!("file://{}", root_repository.display());
        let member_url = format!("file://{}", member_repository.display());
        let manifest = dir.join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest)
            .unwrap()
            .replace("https://github.com/build-your-own-game/soiree", &root_url)
            .replace(
                "members = [\"crates/core\", \"crates/host\", \"crates/web\"]",
                "members = [\"crates/*\"]",
            )
            .replace(
                &format!("tag = \"v{}\"", env!("CARGO_PKG_VERSION")),
                "tag = \"v0.1.0\"",
            );
        std::fs::write(manifest, text).unwrap();
        std::fs::write(
            dir.join("soiree.toml"),
            "version = \"0.1.0\"\nname = \"negative-game\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("README.md"), "Fixture du test public upgrade.\n").unwrap();
        std::fs::write(dir.join("verification.toml"), "schema = 1\n").unwrap();
        std::fs::write(dir.join("CERTIFICATION.md"), "# Fixture upgrade sans mécanique de jeu\n\n<!-- soiree:guarantees:start -->\n<!-- soiree:guarantees:end -->\n").unwrap();
        success(
            &command(&dir)
                .args(["contracts", "--write-view"])
                .output()
                .unwrap(),
        );
        std::fs::remove_dir_all(dir.join("crates")).unwrap();
        for member in ["core", "host", "web"] {
            let directory = dir.join("crates").join(member);
            std::fs::create_dir_all(directory.join("src")).unwrap();
            let dependency = if member == "host" {
                format!(
                    "[dependencies]\nsalon-host.workspace = true\nremote-protocol = {{ git = \"{member_url}\", tag = \"v0.1.0\" }} # keep host comment\n[build-dependencies]\nbridge = {{ package = \"remote-web\", git = \"{member_url}\", tag = \"v0.1.0\" }}\n"
                )
            } else if member == "web" {
                format!(
                    "[dependencies]\nsalon-web.workspace = true\nremote-web = {{ git = \"{member_url}\", tag = \"v0.1.0\" }} # keep web comment\n[dev-dependencies]\nbridge = {{ package = \"remote-protocol\", git = \"{member_url}\", tag = \"v0.1.0\" }}\n[target.'cfg(unix)'.dependencies]\nremote-web = {{ git = \"{member_url}\", tag = \"v0.1.0\" }}\n"
                )
            } else {
                "[dependencies]\nsalon-core.workspace = true\n".into()
            };
            std::fs::write(directory.join("Cargo.toml"), format!("# owned member {member}\n[package]\nname = \"negative-game-{member}\"\nversion = \"0.1.0\"\nedition.workspace = true\nrust-version.workspace = true\n[lints]\nworkspace = true\n{dependency}")).unwrap();
            std::fs::write(
                directory.join("src/lib.rs"),
                "#![no_std]\npub fn ready() {}\n",
            )
            .unwrap();
        }
        let separate = dir.join("separate");
        std::fs::create_dir_all(separate.join("src")).unwrap();
        std::fs::write(separate.join("Cargo.toml"), format!("[package]\nname = \"separate-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\nremote-web = {{ git = \"{member_url}\", tag = \"v0.1.0\" }}\n")).unwrap();
        std::fs::write(separate.join("src/lib.rs"), "pub fn ready() {}\n").unwrap();
        dir
    }

    fn file_contents(directory: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
        let mut files = std::collections::BTreeMap::new();
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files.extend(file_contents(&path));
            } else {
                files.insert(path.clone(), std::fs::read(path).unwrap());
            }
        }
        files
    }

    #[test]
    fn public_upgrade_aligns_direct_dependencies_of_real_workspace_members() {
        let parent = fresh();
        let target = format!("v{}", env!("CARGO_PKG_VERSION"));
        let repository = shared_repository(&parent, "published", Some(&target));
        let dir = workspace_upgrade_game(&repository, &repository);
        let before = file_contents(&dir);
        assert!(!dir.join("Cargo.lock").exists());
        let preview = command(&dir)
            .args(["upgrade", "--dry-run"])
            .output()
            .unwrap();
        success(&preview);
        assert!(String::from_utf8_lossy(&preview.stdout).contains("crates/web/Cargo.toml"));
        assert_eq!(
            file_contents(&dir),
            before,
            "metadata discovery changed an unlocked workspace"
        );
        success(&command(&dir).arg("upgrade").output().unwrap());
        for name in [
            "Cargo.toml",
            "crates/host/Cargo.toml",
            "crates/web/Cargo.toml",
        ] {
            let text = std::fs::read_to_string(dir.join(name)).unwrap();
            assert!(text.contains(&format!("tag = \"{target}\"")), "{name}");
            assert!(!text.contains("tag = \"v0.1.0\""), "{name}");
        }
        let host = std::fs::read_to_string(dir.join("crates/host/Cargo.toml")).unwrap();
        let web = std::fs::read_to_string(dir.join("crates/web/Cargo.toml")).unwrap();
        assert!(host.contains("# keep host comment") && host.contains("# owned member host"));
        assert!(web.contains("# keep web comment") && web.contains("[dev-dependencies]"));
        assert!(web.contains("[target.'cfg(unix)'.dependencies]"));
        assert!(
            std::fs::read_to_string(dir.join("separate/Cargo.toml"))
                .unwrap()
                .contains("tag = \"v0.1.0\"")
        );
        let locked = std::fs::read(dir.join("Cargo.lock")).unwrap();
        success(
            &command(&dir)
                .args(["upgrade", "--dry-run"])
                .output()
                .unwrap(),
        );
        assert_eq!(
            std::fs::read(dir.join("Cargo.lock")).unwrap(),
            locked,
            "metadata discovery changed an existing lock"
        );
    }

    #[test]
    fn public_upgrade_checks_a_members_missing_tag_before_any_file_changes() {
        let parent = fresh();
        let target = format!("v{}", env!("CARGO_PKG_VERSION"));
        let root_repository = shared_repository(&parent, "published", Some(&target));
        let member_repository = shared_repository(&parent, "unpublished", None);
        let dir = workspace_upgrade_game(&root_repository, &member_repository);
        let before = file_contents(&dir);
        let refused = command(&dir).arg("upgrade").output().unwrap();
        assert!(!refused.status.success());
        assert!(String::from_utf8_lossy(&refused.stderr).contains("aucun fichier modifié"));
        assert_eq!(
            file_contents(&dir),
            before,
            "a missing member tag changed source files or created Cargo.lock"
        );
    }

    #[cfg(unix)]
    #[test]
    fn public_upgrade_refuses_an_external_workspace_member_before_writing() {
        let parent = fresh();
        let target = format!("v{}", env!("CARGO_PKG_VERSION"));
        let repository = shared_repository(&parent, "published", Some(&target));
        let dir = workspace_upgrade_game(&repository, &repository);
        let outside = parent.join("external-member");
        std::fs::rename(dir.join("crates/web"), &outside).unwrap();
        std::fs::write(
            outside.join("Cargo.toml"),
            "[package]\nname = \"external-member\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        std::os::unix::fs::symlink(&outside, dir.join("crates/web")).unwrap();
        let before = file_contents(&dir);
        let outside_before = file_contents(&outside);
        let refused = command(&dir).arg("upgrade").output().unwrap();
        assert!(!refused.status.success());
        assert!(
            String::from_utf8_lossy(&refused.stderr)
                .contains("membre du workspace hors du dossier du jeu")
        );
        assert_eq!(file_contents(&dir), before);
        assert_eq!(file_contents(&outside), outside_before);
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
