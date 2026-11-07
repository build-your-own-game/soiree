//! Le module WebAssembly du jeu à téléphones qui garde la page par défaut, vide, qui fait échouer E21 :
//! `jeu::module!` sur `fautifs::jeux::PageVide` (§ 2.1, § 8.8 ; PHASE-3, § 2.10, lot 7).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::PageVide);
