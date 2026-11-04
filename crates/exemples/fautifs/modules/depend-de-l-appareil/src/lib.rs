//! Le module WebAssembly du jeu qui départage par la valeur d'un appareil, qui fait échouer E10 : `jeu::module!` sur
//! `fautifs::jeux::DependDeLAppareil` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::DependDeLAppareil);
