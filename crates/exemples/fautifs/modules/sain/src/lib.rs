//! Le module WebAssembly de le jeu sain, qui passe tous les contrôles écrits : `jeu::module!` sur
//! `fautifs::jeux::Sain` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::Sain);
