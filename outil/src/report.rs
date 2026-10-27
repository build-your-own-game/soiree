//! What a command found, printed as it goes, in the style of `check.sh`:
//! ✓ held, ✗ broken (an error), ! to look at (a warning), – for information.

use std::process::ExitCode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Ok,
    Error,
    Warning,
    Info,
}

#[derive(Debug, Default)]
pub struct Report {
    /// Every line, kept for the tests.
    pub lines: Vec<(Level, String)>,
    quiet: bool,
}

impl Report {
    pub fn new() -> Self {
        Self::default()
    }

    /// A report that prints nothing, for the tests.
    pub fn quiet() -> Self {
        Self {
            lines: Vec::new(),
            quiet: true,
        }
    }

    pub fn section(&self, title: &str) {
        if !self.quiet {
            println!("\n== {title}");
        }
    }

    fn push(&mut self, level: Level, message: String) {
        if !self.quiet {
            let mark = match level {
                Level::Ok => '✓',
                Level::Error => '✗',
                Level::Warning => '!',
                Level::Info => '–',
            };
            println!("  {mark} {message}");
        }
        self.lines.push((level, message));
    }

    pub fn ok(&mut self, message: impl Into<String>) {
        self.push(Level::Ok, message.into());
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.push(Level::Error, message.into());
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        self.push(Level::Warning, message.into());
    }

    pub fn info(&mut self, message: impl Into<String>) {
        self.push(Level::Info, message.into());
    }

    /// An indented line under the previous one: a compiler message, a file.
    pub fn detail(&self, line: &str) {
        if !self.quiet {
            println!("      {line}");
        }
    }

    pub fn count(&self, level: Level) -> usize {
        self.lines.iter().filter(|(l, _)| *l == level).count()
    }

    /// Prints the totals and turns them into the process's exit code.
    pub fn finish(&self) -> ExitCode {
        let errors = self.count(Level::Error);
        if !self.quiet {
            println!(
                "\n{errors} erreur(s), {} avertissement(s)",
                self.count(Level::Warning)
            );
        }
        if errors == 0 {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    }
}
