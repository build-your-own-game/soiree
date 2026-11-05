//! Le module WebAssembly du fautif dont les pires cas, sans joueur, sont plus légers
//! que l'écran d'une soirée (E16) : `jeu::module!` sur
//! `fautifs::jeux::PireCasTropLeger` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::PireCasTropLeger);
