//! Le module WebAssembly de le fautif qui attend un appareil parti (E5) : `jeu::module!` sur
//! `fautifs::jeux::AttendUnAbsent` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::AttendUnAbsent);
