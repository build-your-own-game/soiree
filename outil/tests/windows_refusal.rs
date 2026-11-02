//! `soiree certify` on Windows: the harness refuses at its first import, and
//! that reason must reach the terminal, not a stack trace or a bare failure.
//! Nothing here runs on Windows: the platform is changed under the real guard.
#[cfg(all(test, unix))]
mod windows {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    #[test]
    fn the_terminal_says_certify_is_not_available_on_windows() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let parent = manifest
            .join("target/windows-refusal")
            .join(std::process::id().to_string());
        std::fs::create_dir_all(&parent).unwrap();
        let soiree = env!("CARGO_BIN_EXE_soiree");
        let created = Command::new(soiree)
            .current_dir(&parent)
            .args(["new", "game"])
            .output()
            .unwrap();
        assert!(created.status.success());
        // The real guard, with the platform changed under it, as `cert.mjs` loads it.
        let observer: PathBuf = parent.join("observer");
        std::fs::create_dir(&observer).unwrap();
        let toxiproxy = manifest.join("../tools/cert/toxiproxy.mjs");
        std::fs::write(
            observer.join("cert.mjs"),
            format!(
                "Object.defineProperty(process, 'platform', {{ value: 'win32' }});\nawait import({:?});\n",
                format!("file://{}", toxiproxy.display())
            ),
        )
        .unwrap();
        let result = Command::new(soiree)
            .current_dir(parent.join("game"))
            .arg("certify")
            .arg("--harness")
            .arg(observer)
            .output()
            .unwrap();
        let output = format!(
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(!result.status.success());
        assert!(
            output.contains("soiree certify n'est pas encore disponible sous Windows"),
            "{output}"
        );
    }
}
