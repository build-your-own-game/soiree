//! `soiree` : l'outil de la console. Voir `soiree help`.

mod check;
mod games;
mod new;
mod plug;
mod publish;
mod rules;
mod sh;

use std::path::PathBuf;
use std::process::ExitCode;

/// La version de la console : celle de ce binaire.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const MANIFESTE: &str = include_str!("../../MANIFESTE.md");

const HELP: &str = "soiree : l'outil de la console

Pour faire un jeu :
  soiree new <nom>           crée un jeu depuis le jeu de départ
  cargo run --release        lance le jeu (dans son dossier) et affiche les adresses à ouvrir
  soiree check [dossier]     vérifie que le jeu suit les règles et se branche :
                             un jeu qui ne passe pas n'est pas joué à la soirée
  soiree publish [dossier]   met le jeu sur GitHub, dans l'organisation de la soirée :
                             c'est sa sauvegarde, et c'est ainsi qu'il est proposé
  soiree rules [dossier]     écrit dans le jeu les règles de cette version de la console
  soiree manifeste           affiche le manifeste : le but et les règles

Pour l'hôte :
  soiree games [dossier]     récupère tous les jeux de l'organisation (dans `games/`
                             par défaut) et dit lesquels passent la vérification

  soiree version

Sans dossier : le dossier courant.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, rest) = args
        .split_first()
        .map_or(("help", [].as_slice()), |(command, rest)| {
            (command.as_str(), rest)
        });
    let outcome = match command {
        "check" => dir(rest).and_then(|dir| check::run(&dir)),
        "rules" => dir(rest).and_then(|dir| rules::run(&dir)),
        "publish" => dir(rest).and_then(|dir| publish::run(&dir)),
        "games" => games_dir(rest).and_then(|dir| games::run(&dir)),
        "new" => new::run(rest),
        "manifeste" => {
            println!("{MANIFESTE}");
            Ok(())
        }
        "version" | "--version" | "-V" => {
            println!("soiree {VERSION}");
            Ok(())
        }
        "help" | "--help" | "-h" => {
            println!("{HELP}");
            Ok(())
        }
        other => Err(format!("commande inconnue : {other}\n\n{HELP}")),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

/// Le dossier où ranger les jeux : le seul argument, sinon `games/`.
fn games_dir(args: &[String]) -> Result<PathBuf, String> {
    match args {
        [] => Ok(PathBuf::from("games")),
        [dir] => Ok(PathBuf::from(dir)),
        [_, ..] => Err(format!("un seul dossier attendu\n\n{HELP}")),
    }
}

/// Le dossier du jeu : le seul argument, ou le dossier courant.
fn dir(args: &[String]) -> Result<PathBuf, String> {
    match args {
        [] => Ok(PathBuf::from(".")),
        [dir] => Ok(PathBuf::from(dir)),
        [_, ..] => Err(format!("un seul dossier attendu\n\n{HELP}")),
    }
}
