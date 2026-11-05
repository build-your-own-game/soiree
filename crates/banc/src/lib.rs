//! Le banc des navigateurs, et le binaire `soiree-banc` (PHASE-3, § 2.2, § 8.2 et § 8.3).
//!
//! Le banc joue ce qui demande un vrai navigateur : il sert une page de test, lance le
//! navigateur sans interface sur un profil neuf, le pilote, et juge le rapport que la
//! page lui envoie (`POST /banc/rapport`). Il ne retente jamais une mesure (OUT-91), et
//! ne joue qu'un scénario à la fois, jamais à côté d'une compilation (OUT-89, OUT-115).
//!
//! # Ce qu'il sait faire aujourd'hui (lots 3a et 3b)
//!
//! - [`web`] : la règle « mêmes lints » du second workspace, `web/` (P31) ;
//! - [`serveur`] : un petit serveur HTTP sur la boucle locale, qui sert des fichiers et
//!   reçoit les rapports ;
//! - [`navigateur`] : un même pilote pour Chrome, par le protocole `DevTools`
//!   ([`cdp`], [`chrome`]), et pour Firefox, par `WebDriver` `BiDi` ([`bidi`],
//!   [`firefox`]) ; Safari y est « pas exécuté » jusqu'au lot 3c ;
//! - [`machine`] : le scénario `soiree-banc machine`, les essais de
//!   `moteur_commun::Machine` sur `moteur-navigateur` dans un navigateur, et son
//!   contrôle négatif ;
//! - [`rejouer`] : le scénario `soiree-banc rejouer`, E20 (« Deux moteurs ») : les
//!   journaux de l'épreuve repris dans un Worker, comparés au relevé de wasmtime appel par
//!   appel, et le moteur fautif du contrôle négatif ;
//! - [`rapport`] : le jugement de ce qu'une page rapporte.
//!
//! Safari par `safaridriver` et `etalonner` viennent au lot 3c.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod bidi;
pub mod cdp;
pub mod chrome;
pub mod firefox;
mod horloge;
pub mod machine;
pub mod navigateur;
pub mod rapport;
pub mod rejouer;
pub mod serveur;
pub mod web;

use std::path::PathBuf;

/// Le dossier du commun, `soiree/` : celui de ce dépôt, d'où le banc a été compilé.
pub fn commun() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
