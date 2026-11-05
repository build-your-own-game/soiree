//! Le module WebAssembly du fautif dont un `static`, écrit avant un refus, change la
//! tape acceptée qui le suit (E6) : `jeu::module!` sur `fautifs_statiques::jeux::RetientUnRefus`
//! (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs_statiques::jeux::RetientUnRefus);
