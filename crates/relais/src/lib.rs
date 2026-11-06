//! Le relais de la soirée, `soiree-relais` (PHASE-3, § 2.5) : un petit programme Rust, sur
//! un port fixe, en WebSocket seul, qui fait passer des trames qu'il ne sait pas lire et
//! garde sur disque un état récent chiffré.
//!
//! - [`coeur`] : les salles, les liens, les rôles, le routage, les refus, les échéances,
//!   **sans entrées-sorties**, le temps en paramètre (SAL-01, SAL-04) ;
//! - [`file`] : la file de ce que le relais envoie sur un lien, bornée (SAL-06, SAL-07) ;
//! - [`http`] : la requête, qui parle (SAL-33), les routes (SAL-150), les réponses ;
//! - [`admission`] : les places, décidées avant de créer un fil (SAL-143) ;
//! - [`adresses`] : où écouter, quoi annoncer (SAL-75 à SAL-77) ;
//! - [`garde`] : la garde sur disque, durable (SAL-26, SAL-27) ;
//! - [`enregistrement`] : tout ce que le relais voit, pour le banc (§ 8.1) ;
//! - [`coque`] : les fils, les sockets, l'heure ;
//! - le binaire `soiree-relais`.
//!
//! **C'est une forme, pas une promesse** : `relais` ne dépend que de `liaison`, qui ne
//! décrit que les trames en clair, et d'aucune crate de chiffrement ;
//! `tests/relais/graphe.rs` le vérifie sur `cargo metadata`, avec son témoin.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod admission;
pub mod adresses;
pub mod coeur;
pub mod coque;
pub mod enregistrement;
pub mod file;
pub mod garde;
pub mod horloge;
pub mod http;
