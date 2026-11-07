//! La page de la télé (PHASE-3, § 2.1, § 2.9) : elle reçoit la scène du Worker et la
//! dessine. Au lot 9, elle dessine une scène reçue ; l'autorité, les entrées, le son et
//! les composants du DOM viennent aux lots 10a à 13.
//!
//! - [`messages`] : ce qu'un message du Worker (`salle::ecran::VersPage`) apporte, sans
//!   navigateur ;
//! - [`demarrage`] (lot 10b) : l'écran de démarrage, une scène du commun ; [`aide`] : ce
//!   que l'écran dit quand les téléphones n'arrivent pas ; [`clavier`] : ce qu'une touche
//!   devient pour le jeu, et Échap en plein écran (P38) ; [`son`] : ce que la page fait
//!   des effets d'une scène (perdus avant le geste, une fois chacun) ; tous sans
//!   navigateur, testés en natif ;
//! - `toile` (dans le navigateur seulement) : [`Ecran`], la toile, `wgpu` sur WebGPU ou
//!   WebGL2, le rendu de `web/rendu` copié sur la toile, la mesure du travail du GPU
//!   fini, la relecture des pixels, la reprise quand le GPU est perdu ;
//! - `page` (dans le navigateur seulement, lot 10a) : la page reliée au Worker de
//!   l'autorité, son chien de garde, ses relances, F8, « Remplacée », les crochets du
//!   banc (`demarrer_la_page`, que `web/demarrage.js` appelle) ; au lot 10b, l'écran de
//!   démarrage, l'aide, le plein écran et Échap, le verrou d'écran, le son, « Cliquer pour
//!   reprendre », « NON ÉPROUVÉ » ;
//! - au lot 11, sans navigateur : [`deplacables`] (le glisser, TL-36 à TL-47), [`saisie`]
//!   (les champs, TL-27 à TL-34), [`cibles`] (le bouton d'un clic, la modale, le
//!   raccourci d'une touche) ; dans le navigateur : `composants` (les éléments du DOM
//!   posés sur la toile : `Champ`, `EnPlace`, `Selecteur`, `Lien`), `demandes` (`Rendu`,
//!   `Image`, `Lire`, `Telecharger`), et la page qui défile (`FormeEcran::Page`).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod aide;
pub mod cibles;
pub mod clavier;
#[cfg(target_arch = "wasm32")]
mod composants;
#[cfg(target_arch = "wasm32")]
mod demandes;
pub mod demarrage;
pub mod deplacables;
#[cfg(target_arch = "wasm32")]
mod interaction;
pub mod messages;
#[cfg(target_arch = "wasm32")]
mod page;
pub mod saisie;
pub mod son;
#[cfg(target_arch = "wasm32")]
mod toile;

#[cfg(target_arch = "wasm32")]
pub use page::demarrer_la_page;
#[cfg(target_arch = "wasm32")]
pub use toile::{ChoixMoteur, Ecran, attendre, maintenant_ms, texte_d_erreur};
