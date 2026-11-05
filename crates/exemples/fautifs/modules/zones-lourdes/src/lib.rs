//! Le module WebAssembly du jeu au brouillon de 32 Mio, qui fait échouer E19 : `jeu::module!` sur
//! `fautifs::jeux::ZonesLourdes` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::ZonesLourdes);
