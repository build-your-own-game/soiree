//! Le module WebAssembly de le fautif dont la déclaration contredit le module (E1) : `jeu::module!` sur
//! `fautifs::jeux::DeclarationIncoherente` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::DeclarationIncoherente);
