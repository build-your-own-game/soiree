//! Le module WebAssembly du jeu qui déclare une police WOFF2, que le commun ne lit pas, qui fait échouer E1 :
//! `jeu::module!` sur `fautifs::jeux::PoliceWoff2` (§ 2.1, § 8.8 ; PHASE-3, P22, lot 8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::PoliceWoff2);
