//! Le module WebAssembly de le fautif dont la vue montre un secret (E8) : `jeu::module!` sur
//! `fautifs::jeux::VueQuiFuit` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::VueQuiFuit);
