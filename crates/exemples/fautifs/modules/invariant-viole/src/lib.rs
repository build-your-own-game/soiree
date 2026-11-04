//! Le module WebAssembly de le fautif qui viole un invariant (E4) : `jeu::module!` sur
//! `fautifs::jeux::InvariantViole` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::InvariantViole);
