//! Le module WebAssembly d'`ardoise` (INTERFACE.md, § 2.1) : deux lignes.

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(ardoise::Ardoise);
