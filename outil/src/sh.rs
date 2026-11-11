//! Lancer une commande et savoir comment elle a fini.

use std::path::Path;
use std::process::{Command, Stdio};

/// Lance la commande en laissant voir sa sortie. Vrai si elle réussit.
pub fn run(dir: &Path, program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .current_dir(dir)
        .args(args)
        .status()
        .is_ok_and(|status| status.success())
}

/// Lance la commande sans rien montrer. Vrai si elle réussit.
pub fn quiet(dir: &Path, program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .current_dir(dir)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Ce que la commande écrit, si elle réussit.
pub fn output(dir: &Path, program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .current_dir(dir)
        .args(args)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
