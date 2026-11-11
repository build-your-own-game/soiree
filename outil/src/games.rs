//! `soiree games` : pour l'hôte. Récupère tous les jeux de l'organisation et
//! dit lesquels passent la vérification, donc lesquels seront joués.

use std::path::Path;

use serde::Deserialize;

use crate::publish::ORG;
use crate::sh;

/// Le dépôt de la console : il est dans l'organisation, mais ce n'est pas un jeu.
const CONSOLE: &str = "soiree";

#[derive(Deserialize)]
struct Repository {
    name: String,
    #[serde(rename = "isArchived")]
    archived: bool,
}

pub fn run(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|error| format!("{} : {error}", dir.display()))?;
    let listing = sh::output(
        dir,
        "gh",
        &[
            "repo",
            "list",
            ORG,
            "--limit",
            "200",
            "--json",
            "name,isArchived",
        ],
    )
    .ok_or_else(|| {
        format!(
            "impossible de lister les dépôts de {ORG}. Il faut l'outil de GitHub (`gh`, \
             https://cli.github.com) et être connecté : `gh auth login`."
        )
    })?;
    let mut repositories: Vec<Repository> =
        serde_json::from_str(&listing).map_err(|error| format!("gh repo list : {error}"))?;
    repositories.retain(|repository| !repository.archived && repository.name != CONSOLE);
    repositories.sort_by(|a, b| a.name.cmp(&b.name));

    let tool = std::env::current_exe().map_err(|error| format!("soiree : {error}"))?;
    let tool = tool.to_string_lossy();
    let (mut playable, mut refused) = (Vec::new(), Vec::new());
    for repository in &repositories {
        let name = repository.name.as_str();
        let game = dir.join(name);
        println!("── {name}");
        if game.is_dir() {
            if !sh::quiet(&game, "git", &["pull", "--ff-only"]) {
                println!(
                    "  pas mis à jour : il a peut-être des changements locaux. Vérifié tel qu'il est ici."
                );
            }
        } else if !sh::quiet(
            dir,
            "gh",
            &["repo", "clone", &format!("{ORG}/{name}"), name],
        ) {
            println!("  impossible de le récupérer.");
            refused.push(name);
            continue;
        }
        if !game.join("Cargo.toml").is_file() {
            println!("  pas un jeu (pas de Cargo.toml) : ignoré.");
            continue;
        }
        println!("  vérification… (la première fois, la compilation prend quelques minutes)");
        if sh::quiet(&game, &tool, &["check"]) {
            println!("  ✔ passe la vérification");
            playable.push(name);
        } else {
            println!(
                "  ✘ ne passe pas : lance `soiree check` dans {} pour voir pourquoi",
                game.display()
            );
            refused.push(name);
        }
    }
    println!(
        "\n{} jeu(x) pour la soirée : {}",
        playable.len(),
        playable.join(", ")
    );
    if !refused.is_empty() {
        println!(
            "{} jeu(x) à corriger : {}",
            refused.len(),
            refused.join(", ")
        );
    }
    Ok(())
}
