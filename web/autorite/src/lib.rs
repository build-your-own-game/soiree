//! Le Worker de l'autorité (PHASE-3, § 2.1, § 2.3, § 2.4 ; lot 10a) : la partie qui fait
//! foi, dans un Worker dédié du navigateur de l'ordinateur de la soirée.
//!
//! Le cœur est `salle::autorite::Autorite`, sans entrées-sorties, éprouvé en natif ; ce
//! Worker n'en est que la coque :
//!
//! - **le démarrage** : la clé de garde G, la clé privée de l'autorité, le secret d'entrée,
//!   le code de salle, le dernier numéro de case écrit et le module joué vivent dans
//!   `IndexedDB` de `http://localhost:7878`, jamais dans l'adresse (P37,
//!   [`enregistrement`]) ; le module se charge par `GET /jeux/<empreinte>.wasm`, son
//!   BLAKE3 vérifié, sa taille bornée, puis réécrit et compilé (§ 2.8) ;
//! - **deux WebSocket** vers le relais, « temps réel » et « garde » (§ 2.4) ;
//! - **la boucle** : le pas se joue au signal de l'image que la page poste
//!   (`DeLaPage::Image`, P6), la minuterie du Worker en secours quand la page est cachée ;
//!   le retard de chaque réveil part au battement (§ 2.3, « L'arrière-plan ») ;
//! - **le geste en cours** posté à la page avant chaque `Entree` (P39,
//!   `salle::en_cours`) ; c'est la page qui tue le Worker (`salle::chien`) ;
//! - **la garde** : le dernier numéro écrit suit chaque `Ecrit` dans `IndexedDB` ; une
//!   reprise lit la garde, reprend la partie et le registre (§ 2.4) ;
//! - **le banc** (§ 8.2) : un rapport par seconde, la forme canonique d'une reprise, la
//!   partie figée et sa garde écrite, un retard injecté (`VersPage::Banc`).
//!
//! Ce qui ne demande pas le navigateur se teste en natif ; le reste se joue dans Chrome et
//! Firefox par `soiree-banc soiree` et `soiree-banc arriere-plan`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod enregistrement;
#[cfg(target_arch = "wasm32")]
mod worker;

#[cfg(target_arch = "wasm32")]
pub use worker::demarrer_le_worker;
