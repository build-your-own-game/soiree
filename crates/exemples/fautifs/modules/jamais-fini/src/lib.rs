//! Le module WebAssembly de le fautif qui ne finit qu'une fois (E2) : `jeu::module!` sur
//! `fautifs::jeux::JamaisFini` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::JamaisFini);
