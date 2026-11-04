//! Le module WebAssembly du jeu dont chaque vote coûte deux millions de tours de boucle, qui fait échouer E19 : `jeu::module!` sur
//! `fautifs::jeux::BoucleAuVote` (§ 2.1, § 8.8).

#![cfg_attr(target_arch = "wasm32", no_std)]
jeu::module!(fautifs::jeux::BoucleAuVote);
