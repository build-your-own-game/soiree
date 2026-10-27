//! Small helpers shared by the commands: running programs, reading files,
//! walking a tree, and the date.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub type Result<T> = std::result::Result<T, String>;

/// Runs `program` with `args` in `dir` and returns its output, or why it
/// could not start.
pub fn run(dir: &Path, program: &str, args: &[&str]) -> Result<Output> {
    Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|error| format!("{program} : {error}"))
}

/// The trimmed standard output of a command, if it ran and succeeded.
pub fn stdout_of(dir: &Path, program: &str, args: &[&str]) -> Option<String> {
    let output = run(dir, program, args).ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

pub fn read(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|error| format!("{} : {error}", path.display()))
}

pub fn write(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{} : {error}", parent.display()))?;
    }
    fs::write(path, contents).map_err(|error| format!("{} : {error}", path.display()))
}

pub fn parse_toml(text: &str, what: &str) -> Result<toml::Table> {
    toml::from_str(text).map_err(|error| format!("{what} illisible : {error}"))
}

/// `path` as shown to the reader: relative to `base` when it is inside.
pub fn shown(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// An absolute version of `path`, without requiring it to exist.
pub fn absolute(path: &Path) -> PathBuf {
    fs::canonicalize(path)
        .or_else(|_| std::path::absolute(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

/// Every file under `root` whose name ends with one of `suffixes`, in a
/// stable order, without entering directories named in `skip`.
pub fn files_under(root: &Path, suffixes: &[&str], skip: &[&str]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.filter_map(std::result::Result::ok).collect();
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if !skip.contains(&name.as_str()) {
                    pending.push(path);
                }
            } else if suffixes.iter().any(|suffix| name.ends_with(suffix)) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// The current date and time in UTC, as `2026-10-27T14:03:00Z`.
pub fn now_utc() -> String {
    #[expect(
        clippy::disallowed_methods,
        reason = "un outil, pas une simulation : la date du rapport est voulue"
    )]
    let now = std::time::SystemTime::now();
    let seconds = now
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    format_utc(seconds)
}

/// Seconds since 1970 as an ISO 8601 date in UTC (Howard Hinnant's
/// `civil_from_days`).
pub fn format_utc(seconds: u64) -> String {
    let days = seconds / 86_400;
    let rest = seconds % 86_400;
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    use super::format_utc;

    #[test]
    fn dates_are_iso_utc() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(format_utc(1_793_102_645), "2026-10-27T12:04:05Z");
    }
}
