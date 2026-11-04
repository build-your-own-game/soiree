//! Le module WebAssembly de le fautif qui écrit avant de refuser (E6) : `jeu::module!` sur
//! `fautifs::jeux::RefuseApresAvoirEcrit` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::RefuseApresAvoirEcrit);
