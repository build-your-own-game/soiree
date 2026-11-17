//! Le programme du jeu : la console fait tourner la machine et sert les pages.
//! Ce fichier est le même dans tous les jeux : `soiree rules` l'écrit,
//! `soiree check` le vérifie.

fn main() -> std::process::ExitCode {
    telecommande::console::run::<machine::Jeu>(env!("CARGO_MANIFEST_DIR"))
}
