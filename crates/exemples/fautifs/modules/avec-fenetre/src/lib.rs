//! Le module WebAssembly du jeu sain avec une fenêtre, qui passe tous les contrôles : `jeu::module!` sur
//! `fautifs::jeux::AvecFenetre` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::AvecFenetre);
