//! `soiree` : l'outil de la console. Voir `soiree help`.

mod check;
mod new;
mod plug;
mod rules;

use std::path::PathBuf;
use std::process::ExitCode;

/// La version de la console : celle de ce binaire.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const MANIFESTE: &str = include_str!("../../MANIFESTE.md");

const HELP: &str = "soiree : l'outil de la console

  soiree check [dossier]   vérifie qu'un jeu suit les règles et se branche
  soiree rules [dossier]   écrit dans un jeu les règles de cette version
  soiree new <nom>         crée un jeu depuis le jeu de départ
  soiree manifeste         affiche le manifeste de cette version
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

/// Le dossier du jeu : le seul argument, ou le dossier courant.
fn dir(args: &[String]) -> Result<PathBuf, String> {
    match args {
        [] => Ok(PathBuf::from(".")),
        [dir] => Ok(PathBuf::from(dir)),
        [_, ..] => Err(format!("un seul dossier attendu\n\n{HELP}")),
    }
}
