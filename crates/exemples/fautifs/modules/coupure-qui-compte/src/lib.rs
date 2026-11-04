//! Le module WebAssembly du jeu qui retire un joueur à sa première coupure, qui fait échouer E11 : `jeu::module!` sur
//! `fautifs::jeux::CoupureQuiCompte` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::CoupureQuiCompte);
