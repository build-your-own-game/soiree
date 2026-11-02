//! `soiree check` and `soiree recherche …` through the real binary, on copies:
//! a broken sheet stops `soiree check .` and nothing else.
#[cfg(test)]
mod commands {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn fresh() -> PathBuf {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/recherche-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn repository() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn soiree(dir: &Path) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_soiree"));
        command.current_dir(dir);
        command
    }

    fn text(output: &Output) -> String {
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    }

    fn copy_folder(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }

    /// The common repository with its own `recherche/` and the rest read
    /// through links; `certification/` stays here, a run writes in it. The
    /// check's identity refuses links, so this run never passes: these tests
    /// judge its lines, and `recherche.rs` counts the same errors in process.
    #[cfg(unix)]
    fn common() -> PathBuf {
        let dir = fresh();
        let real = repository();
        for entry in std::fs::read_dir(&real).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            let skipped = ["recherche", "certification", "target", ".git"];
            if !skipped.iter().any(|s| name == *s) {
                std::os::unix::fs::symlink(entry.path(), dir.join(name)).unwrap();
            }
        }
        copy_folder(&real.join("recherche"), &dir.join("recherche"));
        dir
    }

    fn break_sheet(dir: &Path) {
        let path = dir.join("recherche/reconnexion.toml");
        let sheet = std::fs::read_to_string(&path).unwrap();
        let broken = sheet.replacen("etat = \"couvert\"", "etat = \"trou\"", 1);
        assert_ne!(sheet, broken);
        std::fs::write(path, broken).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_broken_sheet_stops_the_full_check_of_the_common_repository() {
        let dir = common();
        let clean = soiree(&dir).args(["check", "."]).output().unwrap();
        let shown = text(&clean);
        assert!(shown.contains("✓ recherche : 1 fiche(s)"), "{shown}");
        assert!(!shown.contains("✗ recherche"), "{shown}");
        assert!(shown.contains("– dette de recherche : "), "{shown}");

        break_sheet(&dir);
        let failed = soiree(&dir).args(["check", "."]).output().unwrap();
        let shown = text(&failed);
        assert!(
            shown.contains(
                "✗ recherche/reconnexion.toml : reconnexion-01 : `etat = \"trou\"` sans `trou`"
            ),
            "{shown}"
        );
        assert!(!shown.contains("✓ recherche : "), "{shown}");
    }

    #[cfg(unix)]
    #[test]
    fn a_broken_registry_or_source_list_stops_the_full_check() {
        for (file, from, to, expected) in [
            (
                "recherche/sources.toml",
                "\netat = \"consultee\"",
                "\netat = \"lue\"",
                "✗ recherche/sources.toml",
            ),
            (
                "recherche/sous-systemes.toml",
                "id = \"fiabilite\"",
                "id = \"reconnexion\"",
                "sous-système reconnexion en double",
            ),
        ] {
            let dir = common();
            let path = dir.join(file);
            let original = std::fs::read_to_string(&path).unwrap();
            let broken = original.replacen(from, to, 1);
            assert_ne!(original, broken);
            std::fs::write(path, broken).unwrap();
            let failed = soiree(&dir).args(["check", "."]).output().unwrap();
            let shown = text(&failed);
            assert!(shown.contains(expected), "{shown}");
            assert!(!shown.contains("✓ recherche : "), "{shown}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_broken_sheet_only_costs_a_line_in_the_tight_loop() {
        let dir = common();
        let quick = soiree(&dir)
            .args(["check", "--fast", "."])
            .output()
            .unwrap();
        let shown = text(&quick);
        assert_eq!(shown.matches("dette de recherche").count(), 1, "{shown}");
        assert!(shown.contains("– dette de recherche : "), "{shown}");

        std::fs::write(dir.join("recherche/reconnexion.toml"), "niveau = ").unwrap();
        let quick = soiree(&dir)
            .args(["check", "--fast", "."])
            .output()
            .unwrap();
        let shown = text(&quick);
        assert!(shown.contains("– dette de recherche non lue"), "{shown}");
        assert!(!shown.contains("✗ recherche"), "{shown}");
    }

    #[test]
    fn the_debt_lists_and_never_fails_on_a_healthy_folder() {
        let output = soiree(&repository())
            .args(["recherche", "dette"])
            .output()
            .unwrap();
        let shown = text(&output);
        assert!(output.status.success(), "{shown}");
        assert!(shown.contains("niveau 3 sans fiche"), "{shown}");
        assert!(shown.contains("fiabilite"), "{shown}");
        assert!(shown.contains("trous ouverts"), "{shown}");
    }

    #[test]
    fn a_broken_sheet_is_not_a_clean_debt() {
        let dir = fresh();
        copy_folder(&repository().join("recherche"), &dir.join("recherche"));
        std::fs::write(dir.join("recherche/reconnexion.toml"), "niveau = ").unwrap();
        let output = soiree(&dir).args(["recherche", "dette"]).output().unwrap();
        let shown = text(&output);
        assert!(!output.status.success(), "{shown}");
        assert!(shown.contains("illisible"), "{shown}");
    }

    #[test]
    fn an_unknown_research_command_is_refused() {
        let output = soiree(&repository())
            .args(["recherche", "partout"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{}", text(&output));
    }

    fn git(dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@example.org"])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", text(&output));
    }

    /// A repository tagged v0.0.1 with one source file, and the real registry.
    fn tagged() -> PathBuf {
        let dir = fresh();
        copy_folder(&repository().join("recherche"), &dir.join("recherche"));
        let client = dir.join("salon/crates/core/src/client.rs");
        std::fs::create_dir_all(client.parent().unwrap()).unwrap();
        std::fs::write(&client, "pub fn ancienne() {}\n").unwrap();
        git(&dir, &["init", "-q"]);
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-q", "-m", "départ"]);
        git(&dir, &["tag", "v0.0.1"]);
        dir
    }

    #[test]
    fn the_plan_reads_the_diff_since_the_last_tag() {
        let dir = tagged();
        let quiet = soiree(&dir).args(["recherche", "plan"]).output().unwrap();
        let shown = text(&quiet);
        assert!(quiet.status.success(), "{shown}");
        assert!(shown.contains("v0.0.1"), "{shown}");
        assert!(shown.contains("pas de recherche à faire"), "{shown}");

        let client = dir.join("salon/crates/core/src/client.rs");
        std::fs::write(&client, "pub fn ancienne() {}\npub fn nouvelle() {}\n").unwrap();
        std::fs::write(dir.join("salon/crates/core/src/stream.rs"), "// nouveau\n").unwrap();
        git(&dir, &["add", "salon/crates/core/src/client.rs"]);
        git(&dir, &["commit", "-q", "-m", "ajout"]);
        let plan = soiree(&dir).args(["recherche", "plan"]).output().unwrap();
        let shown = text(&plan);
        assert!(plan.status.success(), "{shown}");
        assert!(shown.contains("reconnexion : niveau 3"), "{shown}");
        assert!(
            shown.contains("nouveau : api-publique fn nouvelle"),
            "{shown}"
        );
        assert!(shown.contains("fiabilite : niveau 3"), "{shown}");
        assert!(
            shown.contains("nouveau : fichier salon/crates/core/src/stream.rs"),
            "{shown}"
        );
        assert!(
            shown.contains("fiche : recherche/reconnexion.toml, niveau 3"),
            "{shown}"
        );
        assert!(shown.contains("fiche : aucune"), "{shown}");
        assert!(
            shown.contains("sources candidates : reel : ioquake/ioq3"),
            "{shown}"
        );
        assert!(shown.contains("1. **Déclencheur.**"), "{shown}");
    }

    #[test]
    fn the_plan_can_read_the_working_tree_or_another_base() {
        let dir = tagged();
        git(&dir, &["tag", "v0.0.2"]);
        let client = dir.join("salon/crates/core/src/client.rs");
        std::fs::write(&client, "pub fn ancienne() {}\npub fn nouvelle() {}\n").unwrap();
        for args in [vec!["--git-diff"], vec!["--depuis", "v0.0.1"]] {
            let plan = soiree(&dir)
                .args(["recherche", "plan"])
                .args(&args)
                .output()
                .unwrap();
            let shown = text(&plan);
            assert!(shown.contains("fn nouvelle"), "{args:?} : {shown}");
        }
        let both = soiree(&dir)
            .args(["recherche", "plan", "--git-diff", "--depuis", "v0.0.1"])
            .output()
            .unwrap();
        assert_eq!(both.status.code(), Some(2));
    }

    #[test]
    fn the_plan_without_a_tag_says_what_to_give_it() {
        let dir = fresh();
        git(&dir, &["init", "-q"]);
        let plan = soiree(&dir).args(["recherche", "plan"]).output().unwrap();
        assert_eq!(plan.status.code(), Some(2));
        assert!(text(&plan).contains("--git-diff"), "{}", text(&plan));
    }

    /// A folder where `gh` is signed in and answers with this script.
    #[cfg(unix)]
    fn stub_network(dir: &Path, gh: &str, curl: &str) -> std::ffi::OsString {
        use std::os::unix::fs::PermissionsExt;
        let bin = dir.join("bin");
        std::fs::create_dir(&bin).unwrap();
        for (name, body) in [("gh", gh), ("curl", curl)] {
            let script = bin.join(name);
            std::fs::write(&script, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path = std::iter::once(bin)
            .chain(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            ))
            .collect::<Vec<_>>();
        std::env::join_paths(path).unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn verifie_tells_an_outage_from_a_wrong_sheet() {
        let dir = tagged();
        git(
            &dir,
            &["remote", "add", "origin", "https://github.com/o/soiree.git"],
        );
        let run = |path: &std::ffi::OsString| {
            soiree(&dir)
                .env("PATH", path)
                .args(["recherche", "verifie"])
                .output()
                .unwrap()
        };

        let offline = stub_network(
            &dir,
            "[ \"$1\" = auth ] && exit 0\necho 'error connecting to api.github.com' >&2\nexit 1",
            "echo 'curl: (6) Could not resolve host' >&2\nexit 6",
        );
        let outage = run(&offline);
        let shown = text(&outage);
        assert_eq!(outage.status.code(), Some(2), "{shown}");
        assert!(shown.contains("panne"), "{shown}");
        assert!(shown.contains("0 échec(s), 1 panne(s)"), "{shown}");

        std::fs::remove_dir_all(dir.join("bin")).unwrap();
        let dead = stub_network(
            &dir,
            "[ \"$1\" = auth ] && exit 0\necho 'gh: Not Found (HTTP 404)' >&2\nexit 1",
            "printf 'perdu\\n404'",
        );
        let failed = run(&dead);
        let shown = text(&failed);
        assert_eq!(failed.status.code(), Some(1), "{shown}");
        assert!(shown.contains("n'existe pas chez"), "{shown}");
    }

    /// A game whose `cargo` answers only what `soiree check --fast` asks.
    #[cfg(unix)]
    fn game() -> (PathBuf, std::ffi::OsString) {
        use std::os::unix::fs::PermissionsExt;
        let parent = fresh();
        let created = soiree(&parent)
            .args(["new", "jeu-sans-fiche"])
            .output()
            .unwrap();
        assert!(created.status.success(), "{}", text(&created));
        let dir = parent.join("jeu-sans-fiche");
        let cert = dir.join("cert.json");
        let mut config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&cert).unwrap()).unwrap();
        config["ui"] = serde_json::json!({"not_applicable":{"editing":"fixture","effects":"fixture","lifecycle":"fixture","async":"fixture"}});
        std::fs::write(cert, serde_json::to_string(&config).unwrap()).unwrap();
        let bin = parent.join("bin");
        std::fs::create_dir(&bin).unwrap();
        let metadata = serde_json::json!({"packages":[],"workspace_members":[],"workspace_root":dir,"resolve":{"nodes":[]}});
        std::fs::write(bin.join("metadata.json"), metadata.to_string()).unwrap();
        let script = bin.join("cargo");
        std::fs::write(&script, "#!/bin/sh\ncase \"$1\" in\n -V) echo cargo-fixture ;;\n metadata) cat \"${0%/*}/metadata.json\" ;;\n tree) exit 0 ;;\n *) exit 27 ;;\nesac\n").unwrap();
        std::fs::set_permissions(script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = std::iter::once(bin)
            .chain(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            ))
            .collect::<Vec<_>>();
        (dir, std::env::join_paths(path).unwrap())
    }

    #[cfg(unix)]
    #[test]
    fn a_game_is_never_stopped_by_research() {
        let (dir, path) = game();
        let check = || {
            soiree(&dir)
                .env("PATH", &path)
                .args(["check", "--fast"])
                .output()
                .unwrap()
        };
        let quiet = check();
        let shown = text(&quiet);
        assert!(quiet.status.success(), "{shown}");
        assert!(!shown.contains("dette de recherche"), "{shown}");

        std::fs::create_dir(dir.join("recherche")).unwrap();
        std::fs::write(dir.join("recherche/rendu.toml"), "pas une fiche").unwrap();
        let informed = check();
        let shown = text(&informed);
        assert!(informed.status.success(), "{shown}");
        assert_eq!(shown.matches("dette de recherche").count(), 1, "{shown}");
    }
}
