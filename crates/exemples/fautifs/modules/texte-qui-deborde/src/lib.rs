//! Le module WebAssembly du jeu dont un pire cas écrit un texte qui ne tient pas dans sa boîte, qui fait échouer E16 :
//! `jeu::module!` sur `fautifs::jeux::TexteQuiDeborde` (§ 2.1, § 8.8 ; PHASE-3, § 2.9, lot 8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::TexteQuiDeborde);
