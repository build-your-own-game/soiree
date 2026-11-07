//! Le module WebAssembly du jeu livré au-delà de `TAILLE_MODULE_MAX`, qui fait échouer
//! E1 : `jeu::module!` sur `fautifs::jeux::ModuleTropGros` (§ 2.1, § 8.8 ; PHASE-3, lot 6).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::ModuleTropGros);
