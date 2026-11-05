//! Le module WebAssembly du fautif dont un invariant déclaré n'est jamais évalué (E4) :
//! `jeu::module!` sur `fautifs::jeux::InvariantJamaisEvalue` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::InvariantJamaisEvalue);
