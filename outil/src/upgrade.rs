//! `soiree upgrade`: a stub. It says what it will do, and changes nothing.

use std::process::ExitCode;

use crate::embedded::{ORG, VERSION};

pub fn run() -> ExitCode {
    println!("soiree upgrade n'est pas encore écrit : rien n'a été modifié.");
    println!();
    println!("Ce qu'il fera, dans le dossier du jeu :");
    println!("  1. Lire la version de soiree.toml, et la dernière étiquette de");
    println!("     https://github.com/{ORG}/soiree (git ls-remote --tags).");
    println!("  2. Si ce binaire ({VERSION}) n'est pas celui de cette étiquette, donner la");
    println!("     commande cargo install qui l'installe, et s'arrêter là.");
    println!("  3. Afficher, de CHANGELOG.md, chaque section « Passer à cette version »");
    println!("     entre la version du jeu et la nouvelle.");
    println!("  4. Changer la version de soiree.toml et l'étiquette des dépendances de");
    println!("     salon et de la télécommande dans Cargo.toml, puis cargo update sur elles.");
    println!("  5. Réécrire rust-toolchain.toml et clippy.toml depuis les règles embarquées,");
    println!("     et ajouter à Cargo.toml les lints et profils qui manquent, sans");
    println!("     retirer ce que le jeu a durci.");
    println!("  6. Réécrire .soiree/, puis lancer soiree check.");
    println!();
    println!("D'ici là, à la main : les étapes 3 à 6.");
    ExitCode::FAILURE
}
