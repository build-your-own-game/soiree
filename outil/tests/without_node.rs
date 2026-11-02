//! `soiree check` on a machine without Node: the harness needs it, the check
//! does not, unless a game declares a pattern only JavaScript can compile.
#[cfg(test)]
#[cfg(unix)]
mod without_node {
    use serde_json::{Value, json};
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn fresh() -> PathBuf {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/without-node")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn script(path: &Path, text: &str) {
        std::fs::write(path, text).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// A `PATH` of one folder with what a check runs, and no Node to find.
    fn tools(parent: &Path) -> PathBuf {
        let bin = parent.join("bin");
        std::fs::create_dir(&bin).unwrap();
        let metadata = json!({"packages":[], "workspace_members":[], "workspace_root":parent.join("game"), "resolve":{"nodes":[]}});
        std::fs::write(
            bin.join("metadata.json"),
            serde_json::to_string(&metadata).unwrap(),
        )
        .unwrap();
        script(
            &bin.join("cargo"),
            "#!/bin/sh\ncase \"$1\" in\n -V) echo cargo-fixture ;;\n metadata) /bin/cat \"${0%/*}/metadata.json\" ;;\n tree) exit 0 ;;\n *) exit 27 ;;\nesac\n",
        );
        script(&bin.join("rustc"), "#!/bin/sh\necho rustc-fixture\n");
        for tool in ["git", "rustfmt"] {
            let real = std::env::split_paths(&std::env::var_os("PATH").unwrap())
                .map(|folder| folder.join(tool))
                .find(|candidate| candidate.is_file())
                .unwrap();
            script(
                &bin.join(tool),
                &format!("#!/bin/sh\nexec '{}' \"$@\"\n", real.display()),
            );
        }
        bin
    }

    fn game(parent: &Path) -> PathBuf {
        let created = Command::new(env!("CARGO_BIN_EXE_soiree"))
            .current_dir(parent)
            .args(["new", "game"])
            .output()
            .unwrap();
        assert!(created.status.success());
        parent.join("game")
    }

    fn set_ui(dir: &Path, ui: Value) {
        let cert = dir.join("cert.json");
        let mut config: Value =
            serde_json::from_str(&std::fs::read_to_string(&cert).unwrap()).unwrap();
        config["ui"] = ui;
        std::fs::write(cert, serde_json::to_string(&config).unwrap()).unwrap();
    }

    fn check_fast(dir: &Path, path: &Path) -> Output {
        Command::new(env!("CARGO_BIN_EXE_soiree"))
            .current_dir(dir)
            .env("PATH", path)
            .args(["check", "--fast"])
            .output()
            .unwrap()
    }

    fn text(output: &Output) -> String {
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    }

    fn not_applicable() -> Value {
        json!({"not_applicable":{"editing":"fixture has no input","effects":"fixture has no effects","lifecycle":"fixture has page lifetime","async":"fixture has no tasks"}})
    }

    #[test]
    fn a_check_runs_without_node_and_keeps_its_identity() {
        let parent = fresh();
        let dir = game(&parent);
        set_ui(&dir, not_applicable());
        let output = check_fast(&dir, &tools(&parent));
        assert!(output.status.success(), "{}", text(&output));
        let report: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("certification/check-fast.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(report["identity_errors"], json!([]));
        assert!(report["identity_before"].is_string());
        assert_eq!(report["identity_before"], report["identity_after"]);
    }

    #[test]
    fn a_pattern_only_javascript_can_compile_still_fails_without_node() {
        let parent = fresh();
        let dir = game(&parent);
        let mut ui = not_applicable();
        ui["not_applicable"]
            .as_object_mut()
            .unwrap()
            .remove("editing");
        ui["checks"] = json!([{"name":"pattern","command":["node","check.mjs"],"covers":["editing"],"success":"ok"}]);
        set_ui(&dir, ui);
        let output = check_fast(&dir, &tools(&parent));
        assert!(!output.status.success());
        assert!(
            text(&output).contains("cert.json : node"),
            "{}",
            text(&output)
        );
    }
}
