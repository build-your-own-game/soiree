//! Le module WebAssembly du jeu dont la vue d'un pire cas fait deux millions de tours de boucle, qui fait échouer E19 : `jeu::module!` sur
//! `fautifs::jeux::PireCasTropLourd` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::PireCasTropLourd);
