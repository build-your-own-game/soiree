//! Le module WebAssembly du jeu dont une règle lit l'instant de réception d'un geste, qui fait échouer E12 : `jeu::module!` sur
//! `fautifs::jeux::CompteLeRecu` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::CompteLeRecu);
