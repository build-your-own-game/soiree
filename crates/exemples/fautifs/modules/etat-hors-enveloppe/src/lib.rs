//! Le module WebAssembly de le fautif dont l'état sort de l'enveloppe (E14) : `jeu::module!` sur
//! `fautifs_statiques::jeux::EtatHorsEnveloppe` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs_statiques::jeux::EtatHorsEnveloppe);
