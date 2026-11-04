//! Le module WebAssembly de le fautif dont une phase n'arrive jamais (E3) : `jeu::module!` sur
//! `fautifs::jeux::PhaseInaccessible` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::PhaseInaccessible);
