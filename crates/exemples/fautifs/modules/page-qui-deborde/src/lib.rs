//! Le module WebAssembly du jeu dont la page de pire cas écrit un texte qui déborde de tout
//! téléphone, qui fait échouer le scénario `pages` du banc : `jeu::module!` sur
//! `fautifs::jeux::PageQuiDeborde` (PHASE-3, § 8.2 ; lot 12, fin).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::PageQuiDeborde);
