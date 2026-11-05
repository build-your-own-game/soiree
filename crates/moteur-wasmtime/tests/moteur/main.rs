//! Les tests de `moteur-wasmtime` (INTERFACE.md, § 8.7), en un seul binaire, sur des
//! modules écrits à la main en `wat`.
//!
//! `cfg(test)` marque tout ce binaire comme du code de test : les fonctions d'aide y
//! ont droit à `unwrap`, `expect`, `panic!` et aux index (CONTRAT, « Les règles
//! d'écriture »).

#![cfg(test)]

mod chargement_borne;
mod charger;
mod commun;
mod etat;
mod fautes;
mod sans_zones;
