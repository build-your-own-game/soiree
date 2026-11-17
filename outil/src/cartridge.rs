//! Le jeu est une cartouche (voir `CONTRAT.md`) : une machine, des pages, et
//! un programme qui tient en une ligne, la même pour tous. Deux fichiers sont
//! les mêmes dans tous les jeux, au nom de la machine près : le banc de la
//! machine et le programme. `soiree rules` les écrit, `soiree check` les
//! vérifie.

use std::fs;
use std::path::PathBuf;

use crate::rules::{self, Metadata, Package, Target};

/// L'exemple de la machine qui joue des parties sur le banc de la console.
pub const BENCH: &str = "banc";
/// Le crate des pages : l'écran et la télécommande, pour le navigateur.
const PAGES: &str = "pages";

const BENCH_TEMPLATE: &str =
    "//! Le banc de la console : il joue des parties avec la machine de ce jeu.
//! Ce fichier est le même dans tous les jeux : `soiree rules` l'écrit,
//! `soiree check` le vérifie.

fn main() -> std::process::ExitCode {
    cartouche::bench::main::<{lib}::Jeu>()
}
";

const MAIN_TEMPLATE: &str =
    "//! Le programme du jeu : la console fait tourner la machine et sert les pages.
//! Ce fichier est le même dans tous les jeux : `soiree rules` l'écrit,
//! `soiree check` le vérifie.

fn main() -> std::process::ExitCode {
    telecommande::console::run::<{lib}::Jeu>(env!(\"CARGO_MANIFEST_DIR\"))
}
";

/// `examples/banc.rs` de la machine dont la bibliothèque s'appelle `library`.
pub fn bench_file(library: &str) -> String {
    BENCH_TEMPLATE.replace("{lib}", &library.replace('-', "_"))
}

/// `src/main.rs` du programme, pour la machine dont la bibliothèque s'appelle
/// `library`.
pub fn main_file(library: &str) -> String {
    MAIN_TEMPLATE.replace("{lib}", &library.replace('-', "_"))
}

/// Où est le banc de la machine.
pub fn bench_path(machine: &Package) -> PathBuf {
    machine.dir().join("examples").join(format!("{BENCH}.rs"))
}

/// Où est le programme.
fn main_path(program: &Package) -> PathBuf {
    program.dir().join("src").join("main.rs")
}

/// La machine du jeu, s'il en déclare une et une seule.
pub fn machine(metadata: &Metadata) -> Option<&Package> {
    let mut machines = metadata.machines();
    let one = machines.next()?;
    machines.next().is_none().then_some(one)
}

/// Le programme du jeu, s'il en a un et un seul.
fn program(metadata: &Metadata) -> Option<(&Package, &Target)> {
    match metadata.binaries().as_slice() {
        [one] => Some(*one),
        [] | [_, _, ..] => None,
    }
}

/// `soiree rules` : écrit le banc de la machine et le programme. Ce qui
/// manque pour les écrire est dit, sans arrêter le reste.
pub fn write(metadata: &Metadata) -> Result<(), String> {
    let names: Vec<&str> = metadata
        .machines()
        .map(|package| package.name.as_str())
        .collect();
    let library = match (machine(metadata), names.as_slice()) {
        (Some(machine), _) => write_bench(machine)?,
        (None, []) => {
            println!(
                "  Aucun crate n'est déclaré comme la machine du jeu : ni son banc \
                 (`examples/banc.rs`) ni le programme (`src/main.rs`) ne sont écrits. \
                 `soiree check` dit comment la déclarer."
            );
            None
        }
        (None, several) => {
            println!(
                "  Plusieurs crates sont déclarés comme la machine du jeu ({}) : un jeu n'en a \
                 qu'une. Ni son banc ni le programme ne sont écrits.",
                several.join(", ")
            );
            None
        }
    };
    let Some(library) = library else {
        return Ok(());
    };
    let Some((package, target)) = program(metadata) else {
        println!(
            "  Le jeu n'a pas un programme et un seul (le crate dont `cargo run --release` lance \
             `src/main.rs`) : il n'est pas écrit. `soiree check` dit quoi faire."
        );
        return Ok(());
    };
    let path = main_path(package);
    if target.src_path != path {
        println!(
            "  Le programme du crate {} est {}, pas `src/main.rs` : il n'est pas écrit. Déplace-le, \
             puis relance `soiree rules`.",
            package.name,
            metadata.shown(&target.src_path)
        );
        return Ok(());
    }
    rules::write(&path, &main_file(&library))
}

/// Écrit le banc de la machine. Rend le nom de sa bibliothèque, que le
/// programme nomme.
fn write_bench(machine: &Package) -> Result<Option<String>, String> {
    let Some(library) = machine.library() else {
        println!(
            "  La machine {} n'a pas de bibliothèque (`src/lib.rs`) : ni son banc ni le \
             programme ne sont écrits.",
            machine.name
        );
        return Ok(None);
    };
    let path = bench_path(machine);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|error| format!("{} : {error}", dir.display()))?;
    }
    rules::write(&path, &bench_file(&library))?;
    Ok(Some(library))
}

/// `soiree check` : le jeu a des pages, et un seul programme, celui de la
/// console.
pub fn program_and_pages(metadata: &Metadata) -> Result<(), String> {
    let wrong: Vec<String> = [the_program(metadata), the_pages(metadata)]
        .into_iter()
        .filter_map(Result::err)
        .collect();
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(wrong.join("\n"))
    }
}

/// Le seul programme du jeu est `src/main.rs`, celui de la console.
fn the_program(metadata: &Metadata) -> Result<(), String> {
    let binaries = metadata.binaries();
    let (package, target) = match binaries.as_slice() {
        [] => {
            return Err(
                "  Aucun crate de ce jeu n'a de programme : `cargo run --release` n'a rien à lancer.\n  \
                 Le programme est un crate à part (par exemple `jeu/`) qui emprunte la machine et \
                 la brique `telecommande`, et dont le seul fichier est `src/main.rs`. Crée-le, \
                 ajoute-le aux `members` du workspace, puis lance `soiree rules` : il écrit \
                 `src/main.rs`."
                    .to_owned(),
            );
        }
        [one] => *one,
        [_, _, ..] => {
            let names: Vec<String> = binaries
                .iter()
                .map(|(package, target)| {
                    format!("{} ({})", package.name, metadata.shown(&target.src_path))
                })
                .collect();
            return Err(format!(
                "  Ce jeu a plusieurs programmes : {}.\n  \
                 La console est le programme : un jeu n'en a qu'un, `src/main.rs` d'un seul crate, \
                 qui la lance. Retire les autres (`src/bin/`, `[[bin]]`, un autre `src/main.rs`) : \
                 ce qu'ils font va dans la machine (ce qui décide de la partie), dans les pages \
                 (ce qui se montre) ou dans les tests.",
                names.join(", ")
            ));
        }
    };
    let path = main_path(package);
    if target.src_path != path {
        return Err(format!(
            "  Le programme du crate {} est {} : il doit être `src/main.rs`. Déplace-le, retire \
             la section `[[bin]]` du Cargo.toml de ce crate s'il y en a une, puis lance \
             `soiree rules`.",
            package.name,
            metadata.shown(&target.src_path)
        ));
    }
    let mut extra: Vec<String> = Vec::new();
    crate::check::walk(&package.dir().join("src"), &mut |file| {
        let hidden = file
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with('.'));
        if file != path && !hidden {
            extra.push(metadata.shown(file));
        }
    })
    .map_err(|error| format!("  {error}"))?;
    let mut wrong: Vec<String> = Vec::new();
    if !extra.is_empty() {
        extra.sort();
        wrong.push(format!(
            "  Le programme {} a d'autres fichiers que `src/main.rs` :\n    {}\n  \
             La console fait tourner la machine et sert les pages : le programme n'a rien \
             d'autre à faire. Ce que ces fichiers font va dans la machine (ce qui décide de la \
             partie) ou dans les pages (ce qui se montre), puis supprime-les.",
            package.name,
            extra.join("\n    ")
        ));
    }
    // Sans machine, le programme ne peut pas la nommer : l'étape de la
    // machine le dit déjà.
    if let Some(library) = machine(metadata).and_then(Package::library)
        && fs::read_to_string(&path).ok() != Some(main_file(&library))
    {
        wrong.push(format!(
            "  {} n'est pas le programme de la console. Il est le même dans tous les jeux, au nom \
             de la machine près :\n\n{}\n  Lance `soiree rules` : il l'écrit.",
            metadata.shown(&path),
            indent(&main_file(&library))
        ));
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(wrong.join("\n"))
    }
}

/// Le jeu a un crate `pages`, compilé pour le navigateur.
fn the_pages(metadata: &Metadata) -> Result<(), String> {
    let fix = "Dans son Cargo.toml :\n\n      [lib]\n      crate-type = [\"cdylib\", \"rlib\"]";
    match metadata
        .packages
        .iter()
        .find(|package| package.name == PAGES)
    {
        Some(pages) if pages.is_page() => Ok(()),
        Some(_) => Err(format!(
            "  Le crate `{PAGES}` n'est pas compilé pour le navigateur. {fix}"
        )),
        None => Err(format!(
            "  Ce jeu n'a pas de crate `{PAGES}`. Les pages (l'écran, et la télécommande s'il y \
             en a une) sont un crate nommé `{PAGES}`, en Rust, compilé pour le navigateur : la \
             console les sert. {fix}"
        )),
    }
}

/// Un fichier montré dans un refus, en retrait.
fn indent(text: &str) -> String {
    text.lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("      {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine;
    use serde_json::{Value, json};
    use std::path::Path;

    /// Un jeu pour les tests, dans un dossier à lui : une machine
    /// `ma-machine`, un programme `jeu` et des pages, tels que cargo les
    /// décrirait. `change` retouche la description avant qu'on la lise.
    fn game(test: &str, change: impl FnOnce(&mut Value)) -> (PathBuf, Metadata) {
        let root = std::env::temp_dir().join(format!("soiree-{test}-{}", std::process::id()));
        let _: std::io::Result<()> = fs::remove_dir_all(&root);
        for dir in ["machine/src", "machine/examples", "jeu/src", "pages/src"] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        let at = |path: &str| root.join(path).to_string_lossy().into_owned();
        let mut description = json!({
            "workspace_root": at(""),
            "target_directory": at("target"),
            "packages": [
                {
                    "name": "ma-machine",
                    "manifest_path": at("machine/Cargo.toml"),
                    "targets": [
                        { "name": "ma_machine", "kind": ["lib"], "crate_types": ["lib"], "src_path": at("machine/src/lib.rs") },
                        { "name": "banc", "kind": ["example"], "crate_types": ["bin"], "src_path": at("machine/examples/banc.rs") }
                    ],
                    "dependencies": [
                        { "name": "cartouche", "features": [], "kind": null },
                        { "name": "cartouche", "features": ["bench"], "kind": "dev" }
                    ],
                    "metadata": { "soiree": { "machine": true } }
                },
                {
                    "name": "jeu",
                    "manifest_path": at("jeu/Cargo.toml"),
                    "targets": [
                        { "name": "jeu", "kind": ["bin"], "crate_types": ["bin"], "src_path": at("jeu/src/main.rs") }
                    ],
                    "dependencies": [],
                    "metadata": null
                },
                {
                    "name": "pages",
                    "manifest_path": at("pages/Cargo.toml"),
                    "targets": [
                        { "name": "pages", "kind": ["cdylib", "rlib"], "crate_types": ["cdylib", "rlib"], "src_path": at("pages/src/lib.rs") }
                    ],
                    "dependencies": [],
                    "metadata": null
                }
            ]
        });
        change(&mut description);
        (root, serde_json::from_value(description).unwrap())
    }

    /// Le jeu tel que `soiree rules` le laisse.
    fn written(root: &Path) {
        fs::write(
            root.join("machine/examples/banc.rs"),
            bench_file("ma_machine"),
        )
        .unwrap();
        fs::write(root.join("jeu/src/main.rs"), main_file("ma_machine")).unwrap();
    }

    #[test]
    fn the_rules_write_the_bench_and_the_program() {
        let (root, metadata) = game("write", |_| {});
        write(&metadata).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("machine/examples/banc.rs")).unwrap(),
            bench_file("ma_machine")
        );
        assert_eq!(
            fs::read_to_string(root.join("jeu/src/main.rs")).unwrap(),
            main_file("ma_machine")
        );
        assert_eq!(machine::declared(&metadata), Ok(()));
        assert_eq!(program_and_pages(&metadata), Ok(()));
    }

    #[test]
    fn without_a_machine_the_rules_write_nothing_and_do_not_fail() {
        let (root, metadata) = game("no-machine", |game| {
            game["packages"][0]["metadata"] = Value::Null;
        });
        write(&metadata).unwrap();
        assert!(!root.join("machine/examples/banc.rs").exists());
        assert!(!root.join("jeu/src/main.rs").exists());
        let refusal = machine::declared(&metadata).unwrap_err();
        assert!(refusal.contains("machine = true"));
        assert!(refusal.contains("soiree contrat"));
    }

    #[test]
    fn a_changed_bench_or_program_is_refused() {
        let (root, metadata) = game("changed", |_| {});
        written(&root);
        fs::write(root.join("machine/examples/banc.rs"), "fn main() {}\n").unwrap();
        fs::write(root.join("jeu/src/main.rs"), "fn main() {}\n").unwrap();
        let refusal = machine::declared(&metadata).unwrap_err();
        assert!(refusal.contains("n'est pas le banc de la console"));
        let refusal = program_and_pages(&metadata).unwrap_err();
        assert!(refusal.contains("n'est pas le programme de la console"));
        assert!(refusal.contains("telecommande::console::run::<ma_machine::Jeu>"));
    }

    #[test]
    fn the_machine_borrows_the_contract_without_the_bench_and_its_example_with() {
        let (root, metadata) = game("borrows", |game| {
            game["packages"][0]["dependencies"] =
                json!([{ "name": "cartouche", "features": ["bench"], "kind": null }]);
        });
        written(&root);
        let refusal = machine::declared(&metadata).unwrap_err();
        assert!(refusal.contains("avec l'option `bench` dans `[dependencies]`"));
        assert!(refusal.contains("[dev-dependencies]"));
        let (root, metadata) = game("borrows-nothing", |game| {
            game["packages"][0]["dependencies"] = json!([]);
        });
        written(&root);
        let refusal = machine::declared(&metadata).unwrap_err();
        assert!(refusal.contains("n'emprunte pas la brique `cartouche`"));
    }

    #[test]
    fn the_program_is_alone_and_has_only_its_main() {
        let (root, metadata) = game("alone", |game| {
            let outil = json!({ "name": "outil", "kind": ["bin"], "crate_types": ["bin"], "src_path": "/ailleurs/src/bin/outil.rs" });
            game["packages"][1]["targets"]
                .as_array_mut()
                .unwrap()
                .push(outil);
        });
        written(&root);
        let refusal = program_and_pages(&metadata).unwrap_err();
        assert!(refusal.contains("plusieurs programmes"));

        let (root, metadata) = game("only-main", |_| {});
        written(&root);
        fs::write(root.join("jeu/src/regles.rs"), "").unwrap();
        let refusal = program_and_pages(&metadata).unwrap_err();
        assert!(refusal.contains("d'autres fichiers que `src/main.rs`"));
        assert!(refusal.contains("jeu/src/regles.rs"));

        let (root, metadata) = game("no-program", |game| {
            game["packages"][1]["targets"] = json!([]);
        });
        written(&root);
        let refusal = program_and_pages(&metadata).unwrap_err();
        assert!(refusal.contains("n'a de programme"));
    }

    #[test]
    fn the_pages_are_a_crate_for_the_browser() {
        let (root, metadata) = game("pages", |game| {
            game["packages"][2]["targets"][0]["crate_types"] = json!(["lib"]);
        });
        written(&root);
        let refusal = program_and_pages(&metadata).unwrap_err();
        assert!(refusal.contains("n'est pas compilé pour le navigateur"));
        let (root, metadata) = game("no-pages", |game| {
            game["packages"][2]["name"] = json!("ecran");
        });
        written(&root);
        let refusal = program_and_pages(&metadata).unwrap_err();
        assert!(refusal.contains("pas de crate `pages`"));
    }

    #[test]
    fn the_bench_names_the_machine_s_library() {
        assert_eq!(
            bench_file("ma-machine"),
            "//! Le banc de la console : il joue des parties avec la machine de ce jeu.\n\
             //! Ce fichier est le même dans tous les jeux : `soiree rules` l'écrit,\n\
             //! `soiree check` le vérifie.\n\
             \n\
             fn main() -> std::process::ExitCode {\n    \
             cartouche::bench::main::<ma_machine::Jeu>()\n\
             }\n"
        );
    }

    #[test]
    fn the_program_names_the_machine_s_library() {
        assert_eq!(
            main_file("ma-machine"),
            "//! Le programme du jeu : la console fait tourner la machine et sert les pages.\n\
             //! Ce fichier est le même dans tous les jeux : `soiree rules` l'écrit,\n\
             //! `soiree check` le vérifie.\n\
             \n\
             fn main() -> std::process::ExitCode {\n    \
             telecommande::console::run::<ma_machine::Jeu>(env!(\"CARGO_MANIFEST_DIR\"))\n\
             }\n"
        );
    }
}
