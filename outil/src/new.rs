//! `soiree new <nom>` : un jeu neuf, copié du jeu de départ.

use std::fs;
use std::path::Path;

include!(concat!(env!("OUT_DIR"), "/modele.rs"));

/// Les briques de la console. Dans son dépôt, le jeu de départ les emprunte
/// par leur chemin ; un jeu neuf les emprunte à la version de cet outil.
const BRICKS: [&str; 2] = ["telecommande", "cartouche"];
const TITLE: &str = "Mon jeu";

/// D'où un jeu emprunte une brique : le dépôt de la console, à l'étiquette
/// de cette version.
pub fn brick_source() -> String {
    format!(
        r#"git = "https://github.com/build-your-own-game/soiree", tag = "v{}""#,
        crate::VERSION
    )
}

/// Un nom de jeu, qui est aussi celui de son dépôt : minuscules, chiffres et tirets.
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Le manifeste où chaque brique empruntée par son chemin
/// (`cartouche = { path = "../cartouche", … }`) l'est par l'étiquette de
/// cette version. Le reste de la ligne (les options) ne bouge pas.
fn borrow_bricks_by_tag(text: &str) -> String {
    let source = brick_source();
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let names_a_brick = BRICKS.iter().any(|brick| {
            line.trim_start()
                .strip_prefix(brick)
                .is_some_and(|rest| rest.trim_start().starts_with('='))
        });
        let by_path = line
            .split_once(r#"path = ""#)
            .filter(|_| names_a_brick)
            .and_then(|(before, after)| Some((before, after.split_once('"')?.1)));
        match by_path {
            Some((before, rest)) => {
                out.push_str(before);
                out.push_str(&source);
                out.push_str(rest);
            }
            None => out.push_str(line),
        }
    }
    out
}

pub fn run(args: &[String]) -> Result<(), String> {
    let [name] = args else {
        return Err("usage : soiree new <nom>".to_owned());
    };
    if !is_valid_name(name) {
        return Err(format!(
            "« {name} » : le nom d'un jeu s'écrit en minuscules, chiffres et tirets, comme `course-de-canards`"
        ));
    }
    let dir = Path::new(name);
    if dir.exists() {
        return Err(format!("{name} existe déjà"));
    }
    for (relative, bytes) in FILES {
        let path = dir.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("{} : {error}", parent.display()))?;
        }
        let written = match std::str::from_utf8(bytes) {
            Ok(text) => fs::write(&path, borrow_bricks_by_tag(text).replace(TITLE, name)),
            Err(_) => fs::write(&path, bytes),
        };
        written.map_err(|error| format!("{} : {error}", path.display()))?;
    }
    println!(
        "Le jeu {name} est créé. La suite :\n\n  \
         cd {name}\n  \
         cargo run --release   lance le jeu et affiche les adresses à ouvrir\n  \
         soiree publish        met le jeu sur GitHub, pour le sauvegarder et le proposer à la soirée\n  \
         soiree check          dit si le jeu sera joué à la soirée\n\n\
         Donne AGENTS.md à ton agent de code, et dis-lui quel jeu tu veux."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_game_borrows_the_bricks_by_this_version_s_tag() {
        let text = "[workspace.dependencies]\n\
                    telecommande = { path = \"../telecommande\" }\n\
                    cartouche = { path = \"../cartouche\" }\n\
                    machine = { path = \"machine\" }\n\
                    \n\
                    [dev-dependencies]\n\
                    cartouche = { path = \"../../cartouche\", features = [\"bench\"] }\n";
        let source = brick_source();
        assert_eq!(
            borrow_bricks_by_tag(text),
            format!(
                "[workspace.dependencies]\n\
                 telecommande = {{ {source} }}\n\
                 cartouche = {{ {source} }}\n\
                 machine = {{ path = \"machine\" }}\n\
                 \n\
                 [dev-dependencies]\n\
                 cartouche = {{ {source}, features = [\"bench\"] }}\n"
            )
        );
    }

    #[test]
    fn a_brick_borrowed_from_the_workspace_is_left_alone() {
        let text = "[dev-dependencies]\ncartouche = { workspace = true, features = [\"bench\"] }\n";
        assert_eq!(borrow_bricks_by_tag(text), text);
    }
}
