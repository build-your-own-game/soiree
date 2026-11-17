//! Le banc de la console : il joue des parties avec la machine de ce jeu.
//! Ce fichier est le même dans tous les jeux : `soiree rules` l'écrit,
//! `soiree check` le vérifie.

fn main() -> std::process::ExitCode {
    cartouche::bench::main::<machine::Jeu>()
}
