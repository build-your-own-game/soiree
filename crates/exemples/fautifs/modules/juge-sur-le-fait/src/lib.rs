//! Le module WebAssembly du fautif qui croit qu'un choix fait après le changement du
//! chiffre affiché l'a vu (E4, dans le monde réel) : `jeu::module!` sur
//! `fautifs::jeux::JugeSurLeFait` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::JugeSurLeFait);
