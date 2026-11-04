//! Le module WebAssembly du jeu qui chaîne une échéance sur la précédente, qui fait échouer E18 : `jeu::module!` sur
//! `fautifs::jeux::GelQuiEnchaine` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::GelQuiEnchaine);
