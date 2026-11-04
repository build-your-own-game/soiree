//! Le module WebAssembly de le fautif dont la vue écrit (E7) : `jeu::module!` sur
//! `fautifs::jeux::VueQuiEcrit` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::VueQuiEcrit);
