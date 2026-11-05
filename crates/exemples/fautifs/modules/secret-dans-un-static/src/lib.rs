//! Le module WebAssembly du fautif qui recopie le mot tiré, secret, dans un `static`
//! que l'écran montre sans regard (E8) : `jeu::module!` sur
//! `fautifs_statiques::jeux::SecretDansUnStatic` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs_statiques::jeux::SecretDansUnStatic);
