//! Le banc des navigateurs, et le binaire `soiree-banc` (PHASE-3, § 2.2, § 8.2 et § 8.3).
//!
//! Le banc joue ce qui demande un vrai navigateur : il sert une page de test, lance le
//! navigateur sans interface sur un profil neuf, le pilote, et juge le rapport que la
//! page lui envoie (`POST /banc/rapport`). Il ne retente jamais une mesure (OUT-91), et
//! ne joue qu'un scénario à la fois, jamais à côté d'une compilation (OUT-89, OUT-115).
//!
//! # Ce qu'il sait faire aujourd'hui (lots 3a, 3b et 3c)
//!
//! - [`web`] : la règle « mêmes lints » du second workspace, `web/` (P31) ;
//! - [`serveur`] : un petit serveur HTTP sur la boucle locale, qui sert des fichiers et
//!   reçoit les rapports ;
//! - [`navigateur`] : un même pilote pour Chrome, par le protocole `DevTools`
//!   ([`cdp`], [`chrome`]), pour Firefox, par `WebDriver` `BiDi` ([`bidi`],
//!   [`firefox`]), et pour Safari, par `safaridriver` ([`safari`]), « pas exécuté »
//!   tant que l'hôte n'autorise pas l'automatisation à distance ;
//! - [`machine`] : le scénario `soiree-banc machine`, les essais de
//!   `moteur_commun::Machine` sur `moteur-navigateur` dans un navigateur, et son
//!   contrôle négatif ;
//! - [`rejouer`] : le scénario `soiree-banc rejouer`, E20 (« Deux moteurs ») : les
//!   journaux de l'épreuve repris dans un Worker, comparés au relevé de wasmtime appel par
//!   appel, et le moteur fautif du contrôle négatif ;
//! - [`etalonner`] : le scénario `soiree-banc etalonner`, M1 (la trace de la capture du
//!   harnais rejouée sur le protocole du commun, appel par appel), M10 (la boucle vide)
//!   et la profondeur que tient chaque navigateur (M9) ;
//! - [`rapport`] : le jugement de ce qu'une page rapporte ;
//! - [`soiree_native`] : le scénario `soiree-banc soiree-native` (lot 5), une soirée par
//!   le vrai relais, l'autorité et des téléphones natifs, sous les pannes du
//!   [`mandataire`], sur des WebSocket non bloquantes ([`ws`]).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod bidi;
pub mod cdp;
pub mod chrome;
pub mod etalonner;
pub mod firefox;
mod horloge;
pub mod machine;
pub mod mandataire;
pub mod navigateur;
pub mod rapport;
pub mod rejouer;
pub mod safari;
pub mod serveur;
pub mod soiree_native;
pub mod web;
pub mod ws;

use std::path::PathBuf;

/// Le dossier du commun, `soiree/` : celui de ce dépôt, d'où le banc a été compilé.
pub fn commun() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
