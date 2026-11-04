//! Le module WebAssembly de le fautif dont la copie native diffère du module (E9) : `jeu::module!` sur
//! `fautifs::jeux::NatifDifferent` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::NatifDifferent);
