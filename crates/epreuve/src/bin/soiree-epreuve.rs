//! `soiree-epreuve <dossier du jeu>` : l'épreuve d'un jeu depuis ses sources, avec sa
//! copie native ; `soiree-epreuve --wasm <module.wasm>` : celle d'un module seul
//! (§ 8.8 ; CONTRAT, « L'épreuve »). Voir `epreuve::ligne_de_commande`.

#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(epreuve::ligne_de_commande(std::env::args().skip(1), None))
}
