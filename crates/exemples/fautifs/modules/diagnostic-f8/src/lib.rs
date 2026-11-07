//! Le module WebAssembly du jeu dont la touche du diagnostic est F8, réservée au commun, qui fait échouer E1 :
//! `jeu::module!` sur `fautifs::jeux::DiagnosticF8` (§ 2.1, § 8.8 ; PHASE-3, P33, lot 7).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::DiagnosticF8);
