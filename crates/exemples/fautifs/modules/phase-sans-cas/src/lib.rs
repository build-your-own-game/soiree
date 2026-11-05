//! Le module WebAssembly du fautif dont une phase n'arrive jamais, sans qu'aucun pire
//! cas n'y mène (E3) : `jeu::module!` sur `fautifs::jeux::PhaseSansCas` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::PhaseSansCas);
