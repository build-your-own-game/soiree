//! Le module WebAssembly de la page dont un bouton porte un geste trop gros, qui fait échouer E21 : `jeu::module!` sur
//! `fautifs::jeux::PageQuiEnvoieUnGesteInvalide` (§ 2.1, § 8.8 ; PHASE-3, lot 6).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::PageQuiEnvoieUnGesteInvalide);
