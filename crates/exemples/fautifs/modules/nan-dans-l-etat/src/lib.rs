//! Le module WebAssembly du jeu qui garde un NaN dans son état, qui fait échouer E17 : `jeu::module!` sur
//! `fautifs::jeux::NanDansLEtat` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::NanDansLEtat);
