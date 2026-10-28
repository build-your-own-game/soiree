//! `soiree new <nom>`: a new game from the embedded model.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::embedded::{self, MODEL, VERSION};
use crate::util::{self, Result};

pub fn run(name: &str, parent: &Path, org: &str) -> ExitCode {
    match create(name, parent, org) {
        Ok(dir) => {
            println!("{} créé, à la version {VERSION} du commun.", dir.display());
            println!();
            println!("Le jeu de départ est jouable : TV, téléphones, simulation et tests.");
            println!();
            println!("Ensuite :");
            println!("  cd {name}");
            println!("  soiree check  # vérifie le jeu et crée Cargo.lock avant le premier commit");
            println!("  git init -b main && git add -A && git commit -m \"Le jeu de départ\"");
            println!("  gh repo create {org}/{name} --private --source . --push");
            println!("puis lis AGENTS.md.");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("soiree new : {error}");
            ExitCode::FAILURE
        }
    }
}

pub fn create(name: &str, parent: &Path, org: &str) -> Result<PathBuf> {
    if matches!(name, "salon" | "remote") {
        return Err(format!(
            "« {name} » : nom réservé au commun ; choisis un nom de jeu distinct (salon-arcade, remote-aventure…)"
        ));
    }
    if !valid_name(name) {
        return Err(format!(
            "« {name} » : attendu des minuscules, des chiffres et des tirets, en commençant par une lettre (course-de-canards)"
        ));
    }
    let dir = parent.join(name);
    if dir.exists() {
        return Err(format!("{} existe déjà", dir.display()));
    }
    for (path, contents) in MODEL {
        let filled = contents
            .replace("{{nom}}", name)
            .replace("{{org}}", org)
            .replace("{{version}}", VERSION);
        util::write(&dir.join(path), &filled)?;
    }
    embedded::write_docs(&dir)?;
    crate::ci::write(&dir)?;
    Ok(dir)
}

/// Lower-case letters, digits and single dashes, starting with a letter: a
/// repository name and a crate prefix at once.
pub fn valid_name(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.ends_with('-')
        && !name.contains("--")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lints;
    use crate::report::{Level, Report};

    #[test]
    fn names() {
        assert!(valid_name("course-de-canards") && valid_name("jeu2"));
        for bad in ["", "Jeu", "2jeu", "jeu-", "a--b", "jeu_x", "jeu x", "../x"] {
            assert!(!valid_name(bad), "{bad}");
        }
    }

    #[test]
    fn canonical_common_names_are_reserved_before_writing_a_game() {
        let parent = crate::testdir::fresh("reserved-game-names");
        for name in ["salon", "remote"] {
            let error = create(name, &parent, "amis").unwrap_err();
            assert!(error.contains("nom réservé au commun"), "{error}");
            assert!(!parent.join(name).exists(), "reserved name wrote a game");
        }
        for name in ["salon-arcade", "remote-aventure"] {
            assert!(create(name, &parent, "amis").is_ok(), "{name}");
        }
    }

    #[test]
    fn a_new_game_is_filled_and_follows_the_rules() {
        let parent = crate::testdir::fresh("new");
        let dir = create("canards", &parent, "amis").unwrap();
        for (path, _) in MODEL {
            let text = util::read(&dir.join(path)).unwrap();
            assert!(!text.contains("{{"), "{path} garde un {{{{…}}}}");
        }
        assert!(
            util::read(&dir.join("Cargo.toml"))
                .unwrap()
                .contains(&format!(
                    "github.com/build-your-own-game/soiree\", tag = \"v{VERSION}\""
                ))
        );
        assert!(dir.join(".soiree/salon/README.md").is_file());
        assert_eq!(
            util::read(&dir.join(".soiree/version")).unwrap().trim(),
            VERSION
        );

        let mut report = Report::quiet();
        lints::check_toolchain(&dir, &mut report);
        let project = util::parse_toml(&util::read(&dir.join("Cargo.toml")).unwrap(), "c").unwrap();
        lints::check_cargo(&project, &mut report);
        lints::check_clippy(&dir, &mut report);
        crate::ci::check(&dir, &mut report);
        crate::gaps::check(&dir, &mut report);
        crate::items::check(&dir, crate::check::Folder::Game, &mut report);
        assert_eq!(report.count(Level::Error), 0, "{:?}", report.lines);

        assert!(create("canards", &parent, "amis").is_err());
    }
}
