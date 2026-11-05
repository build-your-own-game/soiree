//! Les modules d'essai que les moteurs et le banc partagent, écrits en `wat` dans le
//! dossier `essais/` de la crate : un seul exemplaire, que les tests de
//! `moteur-wasmtime`, ses mesures et `soiree-banc` lisent (PHASE-3, lot 2).

/// Le module de `Bonjour` qui boucle, en `wat` (`essais/bonjour-qui-boucle.wat`) : la
/// faute `Budget` du chargement, et la boucle vide de M10.
pub const BONJOUR_QUI_BOUCLE: &str = include_str!("../essais/bonjour-qui-boucle.wat");
