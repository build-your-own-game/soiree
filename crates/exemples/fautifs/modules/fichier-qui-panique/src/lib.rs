//! Le module WebAssembly du fautif qui prend le quatrième octet d'un fichier sans
//! vérifier qu'il en a quatre (E13, dans le monde réel) : `jeu::module!` sur
//! `fautifs::jeux::FichierQuiPanique` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::FichierQuiPanique);
