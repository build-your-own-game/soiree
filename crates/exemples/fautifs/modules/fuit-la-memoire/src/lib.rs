//! Le module WebAssembly de le fautif qui retient chaque appareil (E15) : `jeu::module!` sur
//! `fautifs::jeux::FuitLaMemoire` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::FuitLaMemoire);
