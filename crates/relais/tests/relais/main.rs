//! Les tests du relais (PHASE-3, § 8.1 ; lot 5), en un seul binaire : le cœur sur une
//! horloge simulée (`coeur`), la coque sur de vraies connexions (`coque`), le HTTP et qui
//! parle (`http`), la garde sur disque (`garde`), et la forme de ses dépendances
//! (`graphe`). Chaque test cite sa leçon.

//!
//! `cfg(test)` marque tout ce binaire comme du code de test : les fonctions d'aide y ont
//! droit à `unwrap`, `expect`, `panic!` et aux index (CONTRAT, « Les règles d'écriture »).

#![cfg(test)]
#![expect(
    clippy::arithmetic_side_effects,
    reason = "code de test : des instants, des tailles et des numéros petits ; un débordement y serait l'échec du test"
)]
#![expect(
    clippy::disallowed_methods,
    reason = "les tests sur de vraies connexions attendent un temps mural, hors de toute partie"
)]

mod coeur;
mod commun;
mod coque;
mod garde;
mod graphe;
mod http;
mod jour_j;
