//! `soiree recherche plan`: an aid, not a lock. It reads a diff, names what is
//! new under a triggering path, and prints what to do about it, in the words
//! of `recherche/README.md`. It reads lines, not code: it can miss something
//! and it can see what is not there, and it always exits 0 once it has read.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::ExitCode;

use super::controle::{self, Disk};
use super::fiche::{EtatSource, Nouveau, SousSysteme};
use super::{FOLDER, Folder};
use crate::embedded;
use crate::util::{self, Result};

/// Constants with these in their name are delays.
const DELAY_WORDS: [&str; 9] = [
    "TIMEOUT", "DELAY", "DELAI", "GRACE", "INTERVAL", "DEADLINE", "PERIOD", "_MS", "_SECS",
];

const PUBLIC_KINDS: [&str; 8] = [
    "fn", "struct", "enum", "trait", "type", "const", "static", "mod",
];

const DEPENDENCY_TABLES: [&str; 2] = ["dependencies", "build-dependencies"];

/// Source files, for what falls outside every subsystem.
const SOURCE_FILES: [&str; 5] = [".rs", ".swift", ".mjs", ".js", ".ts"];

/// Folders whose new files are not a new thing of the product.
const NOT_PRODUCT: [&str; 6] = [
    "tests",
    "fuzz",
    "benches",
    "examples",
    "target",
    "node_modules",
];

const STEPS_HEADING: &str = "## Le processus";

/// One thing the diff adds.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Found {
    kind: Nouveau,
    path: String,
    what: String,
}

impl Found {
    fn shown(&self) -> String {
        if self.what == self.path {
            format!("{} {}", self.kind, self.what)
        } else {
            format!("{} {} ({})", self.kind, self.what, self.path)
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Changed {
    path: String,
    added: Vec<String>,
    removed: Vec<String>,
}

pub fn run(args: &[String]) -> Result<ExitCode> {
    let mut since = None;
    let mut worktree = false;
    let mut dirs = Vec::new();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--depuis" => {
                since = Some(
                    rest.next()
                        .ok_or("recherche plan : --depuis sans référence")?
                        .clone(),
                );
            }
            "--git-diff" => worktree = true,
            flag if flag.starts_with('-') => {
                return Err(format!("recherche plan : option inconnue {flag}"));
            }
            _ => dirs.push(arg.clone()),
        }
    }
    if worktree && since.is_some() {
        return Err("recherche plan : --depuis ou --git-diff, pas les deux".to_owned());
    }
    let dir = util::absolute(&crate::one_dir(&dirs)?);
    let folder = Folder::load(&dir)?;
    let base = match (since, worktree) {
        (Some(reference), _) => reference,
        (None, true) => "HEAD".to_owned(),
        (None, false) => last_tag(&dir)?,
    };
    let found = findings(&dir, &base)?;
    let label = if worktree {
        "git diff (l'arbre de travail contre HEAD)".to_owned()
    } else {
        format!("{base} (la base contre l'arbre de travail)")
    };
    print!("{}", render(&dir, &label, &found, &folder));
    Ok(ExitCode::SUCCESS)
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let output = util::run(dir, "git", args)?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "git {} : {}",
            args.first().copied().unwrap_or_default(),
            stderr.lines().next().unwrap_or("échec")
        ))
    }
}

fn last_tag(dir: &Path) -> Result<String> {
    git(
        dir,
        &["describe", "--tags", "--abbrev=0", "--match", "v[0-9]*"],
    )
    .map(|tag| tag.trim().to_owned())
    .map_err(|_| {
        "recherche plan : aucune étiquette vX.Y.Z ici ; précise --depuis REF ou --git-diff"
            .to_owned()
    })
}

fn findings(dir: &Path, base: &str) -> Result<Vec<Found>> {
    const PLAIN: [&str; 3] = ["-c", "core.quotePath=false", "diff"];
    let diff = [
        PLAIN.as_slice(),
        &[
            "--relative",
            "--unified=0",
            "--no-color",
            "--no-ext-diff",
            "-M",
            base,
            "--",
        ],
    ]
    .concat();
    let added = [
        PLAIN.as_slice(),
        &[
            "--relative",
            "--name-only",
            "--diff-filter=A",
            "-M",
            base,
            "--",
        ],
    ]
    .concat();
    let mut found = Vec::new();
    for path in git(dir, &added)?.lines() {
        found.push(new_file(path));
    }
    let untracked = git(dir, &["ls-files", "--others", "--exclude-standard"])?;
    found.extend(untracked.lines().map(new_file));
    for file in parse_diff(&git(dir, &diff)?) {
        found.extend(additions(&file));
        if file.path.rsplit('/').next() == Some("Cargo.toml") {
            found.extend(new_dependencies(dir, base, &file.path));
        }
    }
    Ok(found)
}

fn new_file(path: &str) -> Found {
    Found {
        kind: Nouveau::Fichier,
        path: path.to_owned(),
        what: path.to_owned(),
    }
}

/// The hunks of `git diff --unified=0`, by file. Only a header can start with
/// `diff --git` at the first column: a hunk's lines start with `+` or `-`.
fn parse_diff(text: &str) -> Vec<Changed> {
    let mut files = Vec::new();
    let mut current: Option<Changed> = None;
    let mut in_hunks = false;
    for line in text.lines() {
        if line.starts_with("diff --git ") {
            files.extend(current.take());
            in_hunks = false;
        } else if line.starts_with("@@") {
            in_hunks = true;
        } else if !in_hunks {
            if let Some(path) = line.strip_prefix("+++ b/") {
                current = Some(Changed {
                    path: path.to_owned(),
                    ..Changed::default()
                });
            }
        } else if let Some(file) = current.as_mut() {
            if let Some(added) = line.strip_prefix('+') {
                file.added.push(added.to_owned());
            } else if let Some(removed) = line.strip_prefix('-') {
                file.removed.push(removed.to_owned());
            }
        }
    }
    files.extend(current);
    files
}

/// What a reader finds in these lines that the removed ones did not have: a
/// function whose signature changes is not new.
fn fresh(file: &Changed, read: fn(&str) -> Option<String>) -> BTreeSet<String> {
    let gone: BTreeSet<String> = file.removed.iter().filter_map(|l| read(l)).collect();
    file.added
        .iter()
        .filter_map(|l| read(l))
        .filter(|name| !gone.contains(name))
        .collect()
}

fn additions(file: &Changed) -> Vec<Found> {
    let mut found = Vec::new();
    let rust = Path::new(&file.path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"));
    if !rust {
        return found;
    }
    let mut push = |kind: Nouveau, names: BTreeSet<String>| {
        found.extend(names.into_iter().map(|what| Found {
            kind,
            path: file.path.clone(),
            what,
        }));
    };
    push(Nouveau::ApiPublique, fresh(file, public_item));
    push(Nouveau::Delai, fresh(file, delay));
    if file.path.ends_with("wire.rs") || file.path.contains("/protocol/") {
        push(Nouveau::MessageDeProtocole, fresh(file, variant));
    }
    found
}

/// `pub async fn name(` gives `fn name`. `pub(crate)` is not public.
fn public_item(line: &str) -> Option<String> {
    let words: Vec<&str> = line
        .trim_start()
        .strip_prefix("pub ")?
        .split_whitespace()
        .collect();
    let mut words = words.as_slice();
    while let Some((first, rest)) = words.split_first() {
        let qualifier = matches!(*first, "async" | "unsafe")
            || (*first == "const" && rest.first() == Some(&"fn"));
        if !qualifier {
            break;
        }
        words = rest;
    }
    let (kind, rest) = words.split_first()?;
    let name: String = rest
        .first()?
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (PUBLIC_KINDS.contains(kind) && !name.is_empty()).then(|| format!("{kind} {name}"))
}

/// A delay with a name: a constant, or a local bound to a `Duration`. One
/// written in place reads the same as a test's, and would drown the plan.
fn delay(line: &str) -> Option<String> {
    let code = line.trim_start();
    let ident = |text: &str| -> String {
        text.chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect()
    };
    if let Some(rest) = code.strip_prefix("let ") {
        let rest = rest.strip_prefix("mut ").unwrap_or(rest);
        return code.contains("Duration::from_").then(|| ident(rest));
    }
    let declared = code
        .strip_prefix("pub ")
        .or_else(|| code.strip_prefix("pub(crate) "))
        .unwrap_or(code);
    let name = ident(declared.strip_prefix("const ")?);
    let named = DELAY_WORDS.iter().any(|word| name.contains(word));
    (named || code.contains(": Duration")).then_some(name)
}

/// `Hello(Hello),` in a protocol file gives `Hello`.
fn variant(line: &str) -> Option<String> {
    let code = line.trim();
    let name: String = code
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect();
    let rest = code.get(name.len()..)?.trim_start();
    let shaped = rest.is_empty() || rest.starts_with(['(', '{', ',']);
    let named = name.chars().next().is_some_and(char::is_uppercase);
    (named && shaped).then_some(name)
}

fn dependencies_of(table: &toml::Table, names: &mut BTreeSet<String>) {
    for (key, value) in table {
        if let Some(inner) = value.as_table() {
            if DEPENDENCY_TABLES.contains(&key.as_str()) {
                names.extend(inner.keys().cloned());
            } else {
                dependencies_of(inner, names);
            }
        }
    }
}

fn new_dependencies(dir: &Path, base: &str, path: &str) -> Vec<Found> {
    let names = |text: Option<String>| {
        let mut names = BTreeSet::new();
        if let Some(table) = text.and_then(|text| util::parse_toml(&text, path).ok()) {
            dependencies_of(&table, &mut names);
        }
        names
    };
    let before = names(git(dir, &["show", &format!("{base}:./{path}")]).ok());
    let after = names(util::read(&dir.join(path)).ok());
    after
        .difference(&before)
        .map(|name| Found {
            kind: Nouveau::Dependance,
            path: path.to_owned(),
            what: name.clone(),
        })
        .collect()
}

/// The subsystems a finding belongs to. A dependency belongs to its crate.
fn owners<'a>(found: &Found, folder: &'a Folder) -> Vec<&'a SousSysteme> {
    let crate_dir = found
        .path
        .strip_suffix("Cargo.toml")
        .filter(|dir| dir.contains("crates/"));
    folder
        .registre
        .sous_systemes
        .iter()
        .filter(|s| s.perimetre == folder.perimetre)
        .filter(|s| match found.kind {
            Nouveau::Dependance => {
                crate_dir.is_some_and(|dir| s.chemins.iter().any(|c| c.starts_with(dir)))
            }
            Nouveau::Fichier
            | Nouveau::ApiPublique
            | Nouveau::Delai
            | Nouveau::MessageDeProtocole => s.triggered_by(&found.path),
        })
        .collect()
}

fn product_source(path: &str) -> bool {
    SOURCE_FILES.iter().any(|suffix| path.ends_with(suffix))
        && !path.split('/').any(|part| NOT_PRODUCT.contains(&part))
}

fn orphan(found: &Found) -> bool {
    match found.kind {
        Nouveau::Fichier => product_source(&found.path),
        Nouveau::Dependance => true,
        Nouveau::ApiPublique | Nouveau::Delai | Nouveau::MessageDeProtocole => false,
    }
}

/// The steps, as `recherche/README.md` words them.
fn steps(readme: &str) -> &str {
    let section = readme
        .split_once(STEPS_HEADING)
        .map_or(readme, |(_, section)| section);
    section.split("\n## ").next().unwrap_or(section).trim()
}

fn render(dir: &Path, label: &str, found: &[Found], folder: &Folder) -> String {
    let found: Vec<&Found> = found
        .iter()
        .filter(|f| folder.registre.nouveau.contains(&f.kind))
        .collect();
    let mut touched: Vec<&SousSysteme> = Vec::new();
    let mut orphans: Vec<&Found> = Vec::new();
    for item in &found {
        let mine = owners(item, folder);
        if mine.is_empty() && orphan(item) {
            orphans.push(item);
        }
        for sous_systeme in mine {
            if !touched.iter().any(|s| s.id == sous_systeme.id) {
                touched.push(sous_systeme);
            }
        }
    }
    let mut lines = vec![
        format!("== recherche plan : {}", dir.display()),
        format!("comparé à : {label}"),
    ];
    for sous_systeme in &touched {
        lines.push(String::new());
        section(dir, sous_systeme, &found, folder, &mut lines);
    }
    if !orphans.is_empty() {
        lines.push(String::new());
        lines.push("Nouveau, hors de tout sous-système :".to_owned());
        for item in &orphans {
            lines.push(format!("  - {}", item.shown()));
        }
        lines.push(
            "  S'il s'agit d'un sous-système nouveau, ajoute-le à sous-systemes.toml avec son niveau. Sinon, rien à faire."
                .to_owned(),
        );
    }
    lines.push(String::new());
    if let Some(level) = touched.iter().map(|s| s.niveau).max() {
        lines.push(format!(
            "Niveau à tenir : {level}, le plus haut des sous-systèmes touchés."
        ));
        lines.push(String::new());
        lines.push(format!("Les étapes, mot pour mot ({FOLDER}/README.md) :"));
        lines.push(String::new());
        lines.push(steps(embedded::recherche::PROCESSUS).to_owned());
    } else {
        lines.push(
            "Rien de nouveau sous un chemin déclencheur : pas de recherche à faire.".to_owned(),
        );
    }
    lines.push(String::new());
    lines.push(
        "Un bug échappé, que rien n'avait vu, déclenche aussi une recherche : le diff ne peut pas le voir, ouvre la fiche de son sous-système."
            .to_owned(),
    );
    lines.push(String::new());
    lines.join("\n")
}

fn section(
    dir: &Path,
    sous_systeme: &SousSysteme,
    found: &[&Found],
    folder: &Folder,
    lines: &mut Vec<String>,
) {
    let id = &sous_systeme.id;
    let niveau = folder.registre.niveau(sous_systeme.niveau);
    let intention = niveau.map_or("niveau inconnu", |n| n.intention.as_str());
    lines.push(format!(
        "{id} : niveau {} ({intention})",
        sous_systeme.niveau
    ));
    for item in found
        .iter()
        .filter(|f| owners(f, folder).iter().any(|s| s.id == *id))
    {
        lines.push(format!("  nouveau : {}", item.shown()));
    }
    if let Some(niveau) = niveau {
        let quotas: Vec<String> = sous_systeme
            .marches
            .iter()
            .map(|sorte| {
                let wanted = niveau.quota.get(sorte).copied().unwrap_or(1).max(1);
                format!("{sorte} {wanted}")
            })
            .collect();
        lines.push(format!(
            "  quota : {} références, dont {} ; {} leçons ; {} dernières sources sans leçon",
            niveau.references,
            quotas.join(", "),
            niveau.lecons,
            niveau.saturation
        ));
    }
    match folder.sheets.iter().find(|sheet| &sheet.stem == id) {
        None => {
            lines.push("  fiche : aucune".to_owned());
            lines.push(format!("  il manque : {FOLDER}/{id}.toml, avec son quota"));
        }
        Some(sheet) => match &sheet.parsed {
            Err(error) => {
                lines.push(format!("  fiche : {}, illisible", sheet.file));
                lines.push(format!("  il manque : une fiche lisible : {error}"));
            }
            Ok(fiche) => {
                lines.push(format!(
                    "  fiche : {}, niveau {}, comparée à {}",
                    sheet.file, fiche.niveau, fiche.compare_a
                ));
                let errors = controle::rules(fiche, &sheet.stem, &folder.registre, &Disk::new(dir));
                for error in &errors {
                    lines.push(format!("  il manque : {error}"));
                }
                if errors.is_empty() {
                    lines.push(
                        "  il manque : ce nouveau, relu contre cette fiche : ses leçons, ses trous, puis `compare_a`"
                            .to_owned(),
                    );
                }
            }
        },
    }
    if let Some(Ok(sources)) = &folder.sources {
        let candidates: Vec<String> = sources
            .sources
            .iter()
            .filter(|s| &s.sous_systeme == id && s.etat == EtatSource::Candidate)
            .map(|s| format!("{} : {}", s.marche, s.ids.join(", ")))
            .collect();
        if !candidates.is_empty() {
            lines.push(format!("  sources candidates : {}", candidates.join(" ; ")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::fiche::{self, Perimetre, Registre};
    use super::*;

    const NONE: Vec<String> = Vec::new();

    const DIFF: &str =
        "diff --git a/salon/crates/core/src/client.rs b/salon/crates/core/src/client.rs
index 111..222 100644
--- a/salon/crates/core/src/client.rs
+++ b/salon/crates/core/src/client.rs
@@ -10,0 +11,2 @@ impl Client {
+pub fn nouvelle(&self) {}
+const WAKE_TIMEOUT_MS: u64 = 500;
@@ -20 +22 @@ impl Client {
-pub fn ancienne(&self) {}
+pub fn ancienne(&self, x: u8) {}
diff --git a/ancien.rs b/ancien.rs
deleted file mode 100644
--- a/ancien.rs
+++ /dev/null
@@ -1 +0,0 @@
-pub fn disparue() {}
diff --git a/salon/crates/core/src/wire.rs b/salon/crates/core/src/wire.rs
--- a/salon/crates/core/src/wire.rs
+++ b/salon/crates/core/src/wire.rs
@@ -80,0 +81 @@ pub(crate) enum Control {
+    Resume(Resume),
";

    fn registre() -> Registre {
        fiche::read(embedded::recherche::SOUS_SYSTEMES, "r.toml").unwrap()
    }

    fn folder(perimetre: Perimetre) -> Folder {
        Folder {
            registre: registre(),
            sheets: Vec::new(),
            sources: None,
            perimetre,
        }
    }

    fn found(kind: Nouveau, path: &str) -> Found {
        Found {
            kind,
            path: path.to_owned(),
            what: path.to_owned(),
        }
    }

    #[test]
    fn a_diff_is_read_file_by_file_and_a_deleted_file_is_dropped() {
        let files = parse_diff(DIFF);
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "salon/crates/core/src/client.rs",
                "salon/crates/core/src/wire.rs"
            ]
        );
        assert_eq!(files[0].added.len(), 3);
        assert_eq!(files[0].removed.len(), 1);
    }

    #[test]
    fn only_what_the_removed_lines_did_not_have_is_new() {
        let files = parse_diff(DIFF);
        let client: Vec<(Nouveau, String)> = additions(&files[0])
            .into_iter()
            .map(|f| (f.kind, f.what))
            .collect();
        assert_eq!(
            client,
            [
                (Nouveau::ApiPublique, "fn nouvelle".to_owned()),
                (Nouveau::Delai, "WAKE_TIMEOUT_MS".to_owned()),
            ]
        );
        let wire = additions(&files[1]);
        assert_eq!(wire.len(), 1);
        assert_eq!(wire[0].kind, Nouveau::MessageDeProtocole);
        assert_eq!(wire[0].what, "Resume");
    }

    #[test]
    fn the_readers_know_a_public_item_a_delay_and_a_variant() {
        assert_eq!(
            public_item("    pub async fn a(x: u8)").as_deref(),
            Some("fn a")
        );
        assert_eq!(public_item("pub const fn b()").as_deref(), Some("fn b"));
        assert_eq!(
            public_item("pub const LIMIT: u8 = 3;").as_deref(),
            Some("const LIMIT")
        );
        assert_eq!(
            public_item("pub struct C<T>(T);").as_deref(),
            Some("struct C")
        );
        assert_eq!(public_item("pub(crate) fn d()"), None);
        assert_eq!(public_item("pub use x::y;"), None);
        assert_eq!(
            delay("const FINAL_GRACE: u64 = 3;").as_deref(),
            Some("FINAL_GRACE")
        );
        assert_eq!(
            delay("pub const TICK: Duration = x;").as_deref(),
            Some("TICK")
        );
        assert_eq!(
            delay("let wait = Duration::from_millis(40);").as_deref(),
            Some("wait")
        );
        assert_eq!(delay("// Duration::from_millis(40)"), None);
        assert_eq!(delay("const LIMIT: u8 = 3;"), None);
        assert_eq!(variant("    Hello(Hello),").as_deref(), Some("Hello"));
        assert_eq!(variant("    Bye,").as_deref(), Some("Bye"));
        assert_eq!(variant("    Other {").as_deref(), Some("Other"));
        assert_eq!(variant("    /// Doc"), None);
        assert_eq!(variant("    field: u8,"), None);
        assert_eq!(variant("    Self::Bye => 3,"), None);
    }

    #[test]
    fn a_changed_value_is_not_a_new_delay() {
        let file = Changed {
            path: "a.rs".to_owned(),
            added: vec!["let wait = Duration::from_millis(40);".to_owned()],
            removed: vec!["let wait = Duration::from_millis(30);".to_owned()],
        };
        assert_eq!(additions(&file), Vec::new());
    }

    #[test]
    fn a_new_dependency_is_found_in_every_table() {
        let table: toml::Table = toml::from_str(
            "[dependencies]\na = \"1\"\n[build-dependencies]\nb = \"1\"\n[workspace.dependencies]\nc = \"1\"\n[target.'cfg(x)'.dependencies]\nd = \"1\"\n[lints]\ne = 1\n",
        )
        .unwrap();
        let mut names = BTreeSet::new();
        dependencies_of(&table, &mut names);
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        assert_eq!(names, ["a", "b", "c", "d"]);
    }

    #[test]
    fn a_finding_belongs_to_the_subsystems_of_its_path() {
        let commun = folder(Perimetre::Commun);
        let ids = |item: &Found| -> Vec<String> {
            owners(item, &commun).iter().map(|s| s.id.clone()).collect()
        };
        assert_eq!(
            ids(&found(Nouveau::Fichier, "salon/crates/core/src/wire.rs")),
            ["reconnexion", "fiabilite"]
        );
        assert_eq!(ids(&found(Nouveau::Fichier, "outil/src/main.rs")), NONE);
        assert_eq!(
            ids(&found(Nouveau::Fichier, "crates/screen/src/a.rs")),
            NONE
        );
        let dependency = found(Nouveau::Dependance, "salon/crates/crypto/Cargo.toml");
        assert_eq!(ids(&dependency), ["transport-webrtc"]);
        assert_eq!(ids(&found(Nouveau::Dependance, "salon/Cargo.toml")), NONE);
        let game = folder(Perimetre::Jeu);
        let item = found(Nouveau::Fichier, "crates/screen/src/a.rs");
        assert_eq!(owners(&item, &game).len(), 1);
    }

    #[test]
    fn the_plan_says_the_level_the_sheet_and_the_steps() {
        let commun = folder(Perimetre::Commun);
        let item = found(Nouveau::Fichier, "salon/crates/core/src/client.rs");
        let text = render(Path::new("."), "v0.0.0", &[item], &commun);
        assert!(text.contains("reconnexion : niveau 3"), "{text}");
        assert!(text.contains("quota : 12 références"), "{text}");
        assert!(text.contains("fiche : aucune"), "{text}");
        assert!(
            text.contains("il manque : recherche/reconnexion.toml"),
            "{text}"
        );
        assert!(text.contains("Niveau à tenir : 3"), "{text}");
        assert!(text.contains("1. **Déclencheur.**"), "{text}");
        assert!(text.contains("11. **Vérifie.**"), "{text}");
    }

    #[test]
    fn nothing_new_means_no_research_and_an_ordinary_file_stays_silent() {
        let commun = folder(Perimetre::Commun);
        let text = render(Path::new("."), "v0.0.0", &[], &commun);
        assert!(text.contains("pas de recherche à faire"), "{text}");
        assert!(!text.contains("1. **Déclencheur.**"), "{text}");
        let doc = found(Nouveau::Fichier, "README.md");
        let unit = found(Nouveau::Fichier, "salon/crates/core/tests/a.rs");
        let text = render(Path::new("."), "v0.0.0", &[doc, unit], &commun);
        assert!(!text.contains("hors de tout sous-système"), "{text}");
        let new = found(Nouveau::Fichier, "salon/crates/nouveau/src/lib.rs");
        let text = render(Path::new("."), "v0.0.0", &[new], &commun);
        assert!(text.contains("hors de tout sous-système"), "{text}");
    }

    #[test]
    fn a_kind_the_registry_does_not_list_is_not_new() {
        let mut commun = folder(Perimetre::Commun);
        commun
            .registre
            .nouveau
            .retain(|kind| *kind != Nouveau::Fichier);
        let item = found(Nouveau::Fichier, "salon/crates/core/src/client.rs");
        let text = render(Path::new("."), "v0.0.0", &[item], &commun);
        assert!(text.contains("pas de recherche à faire"), "{text}");
    }

    #[test]
    fn the_steps_stop_at_the_next_section() {
        assert_eq!(
            steps("# A\n\n## Le processus\n\n1. Un.\n\n## Après\nx"),
            "1. Un."
        );
    }
}
