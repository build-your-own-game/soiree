//! Le module WebAssembly de `devine` (INTERFACE.md, § 2.1) : deux lignes, écrites par
//! `soiree new`. Il ne porte que ce que `jeu::module!` écrit : les exports,
//! l'allocateur, le gestionnaire de panique. La crate du jeu reste ainsi
//! `forbid(unsafe_code)` (D8).
//!
//! `no_std` sur `wasm32` seulement : en natif, la crate se compile avec `std` pour que
//! `cargo build --workspace` passe, et `jeu::module!` n'y écrit que la vérification de
//! l'accord de la déclaration et des types.

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(devine::Devine);
