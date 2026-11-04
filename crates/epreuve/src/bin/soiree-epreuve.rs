//! `soiree-epreuve <module.wasm>` : l'épreuve d'un module, sans copie native (§ 8.8).
//! Voir `epreuve::ligne_de_commande`.

#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(epreuve::ligne_de_commande(std::env::args().skip(1), None))
}
