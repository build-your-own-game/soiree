//! Le module WebAssembly du fautif dont le `Temps` panique trois pas de suite à
//! l'échéance d'une manche, pour que l'épreuve fasse repartir la partie (E2, § 4.6) :
//! `jeu::module!` sur `fautifs::jeux::PaniqueAuTemps` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::PaniqueAuTemps);
