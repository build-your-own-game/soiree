//! Le module WebAssembly du fautif dont le chiffre choisi, gardé secret pour son
//! joueur, se lit aussi dans le choix public (E8) : `jeu::module!` sur
//! `fautifs::jeux::ChoixQuiFuit` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::ChoixQuiFuit);
