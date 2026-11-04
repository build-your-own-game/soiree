//! Le module WebAssembly de le fautif qui panique sur une entrée absurde (E13) : `jeu::module!` sur
//! `fautifs::jeux::OctetQuiPanique` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::OctetQuiPanique);
