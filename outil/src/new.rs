//! `soiree new <nom>` : un jeu neuf, copié du jeu de départ.

use std::fs;
use std::path::Path;

include!(concat!(env!("OUT_DIR"), "/modele.rs"));

/// Dans le dépôt de la console, le jeu de départ emprunte la brique par son
/// chemin. Un jeu neuf l'emprunte à la version de cet outil.
const LOCAL_BRICK: &str = r#"telecommande = { path = "../telecommande" }"#;
const TITLE: &str = "Mon jeu";

pub fn run(args: &[String]) -> Result<(), String> {
    let [name] = args else {
        return Err("usage : soiree new <nom>".to_owned());
    };
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !valid {
        return Err(format!(
            "« {name} » : le nom d'un jeu s'écrit en minuscules, chiffres et tirets, comme `course-de-canards`"
        ));
    }
    let dir = Path::new(name);
    if dir.exists() {
        return Err(format!("{name} existe déjà"));
    }
    let brick = format!(
        r#"telecommande = {{ git = "https://github.com/build-your-own-game/soiree", tag = "v{}" }}"#,
        crate::VERSION
    );
    for (relative, bytes) in FILES {
        let path = dir.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("{} : {error}", parent.display()))?;
        }
        let written = match std::str::from_utf8(bytes) {
            Ok(text) => fs::write(
                &path,
                text.replace(LOCAL_BRICK, &brick).replace(TITLE, name),
            ),
            Err(_) => fs::write(&path, bytes),
        };
        written.map_err(|error| format!("{} : {error}", path.display()))?;
    }
    println!(
        "Le jeu {name} est créé.\n\n  cd {name}\n  cargo run --release     lance le jeu\n  soiree check            vérifie qu'il passe\n\nLis AGENTS.md, ou donne-le à ton agent."
    );
    Ok(())
}
