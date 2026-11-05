//! Le module WebAssembly du jeu dont la vue de la fin fait sept cent mille tours de boucle, qui fait échouer E19 : `jeu::module!` sur
//! `fautifs::jeux::FinTropLourde` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::FinTropLourde);
