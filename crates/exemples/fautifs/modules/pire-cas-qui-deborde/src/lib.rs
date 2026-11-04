//! Le module WebAssembly du jeu dont le pire cas sort du repère, qui fait échouer E16 : `jeu::module!` sur
//! `fautifs::jeux::PireCasQuiDeborde` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::PireCasQuiDeborde);
