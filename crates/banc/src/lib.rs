//! Le banc des navigateurs, et le binaire `soiree-banc` (PHASE-3, § 2.2, § 8.2 et § 8.3).
//!
//! Le banc joue ce qui demande un vrai navigateur : il sert une page de test, lance le
//! navigateur sans interface sur un profil neuf, le pilote, et juge le rapport que la
//! page lui envoie (`POST /banc/rapport`). Il ne retente jamais une mesure (OUT-91), et
//! ne joue qu'un scénario à la fois, jamais à côté d'une compilation (OUT-89, OUT-115).
//!
//! # Ce qu'il sait faire aujourd'hui (lot 3a)
//!
//! - [`web`] : la règle « mêmes lints » du second workspace, `web/` (P31) ;
//! - [`serveur`] : un petit serveur HTTP sur la boucle locale, qui sert des fichiers et
//!   reçoit les rapports ;
//! - [`cdp`] et [`chrome`] : un pilote minimal de Chrome par le protocole `DevTools` ;
//! - [`machine`] : le scénario `soiree-banc machine`, les essais de
//!   `moteur_commun::Machine` sur `moteur-navigateur` dans Chrome, et son contrôle négatif ;
//! - [`rapport`] : le jugement de ce qu'une page rapporte.
//!
//! Firefox par `WebDriver` `BiDi`, la capture des traces et E20 viennent au lot 3b ; Safari
//! par `safaridriver` et `etalonner` au lot 3c.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod cdp;
pub mod chrome;
mod horloge;
pub mod machine;
pub mod rapport;
pub mod serveur;
pub mod web;

use std::path::PathBuf;

/// Le dossier du commun, `soiree/` : celui de ce dépôt, d'où le banc a été compilé.
pub fn commun() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
