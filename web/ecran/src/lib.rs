//! La page de la télé (PHASE-3, § 2.1, § 2.9) : elle reçoit la scène du Worker et la
//! dessine. Au lot 9, elle dessine une scène reçue ; l'autorité, les entrées, le son et
//! les composants du DOM viennent aux lots 10a à 13.
//!
//! - [`messages`] : ce qu'un message du Worker (`salle::ecran::VersPage`) apporte, sans
//!   navigateur ;
//! - `toile` (dans le navigateur seulement) : [`Ecran`], la toile, `wgpu` sur WebGPU ou
//!   WebGL2, le rendu de `web/rendu` copié sur la toile, la mesure du travail du GPU
//!   fini, la relecture des pixels, la reprise quand le GPU est perdu ;
//! - `page` (dans le navigateur seulement, lot 10a) : la page reliée au Worker de
//!   l'autorité, son chien de garde, ses relances, F8, « Remplacée », les crochets du
//!   banc (`demarrer_la_page`, que `web/demarrage.js` appelle) ;
//! - au lot 11, sans navigateur : [`deplacables`] (le glisser, TL-36 à TL-47), [`saisie`]
//!   (les champs, TL-27 à TL-34), [`cibles`] (le bouton d'un clic, la modale, le
//!   raccourci d'une touche) ; dans le navigateur : `composants` (les éléments du DOM
//!   posés sur la toile : `Champ`, `EnPlace`, `Selecteur`, `Lien`), `demandes` (`Rendu`,
//!   `Image`, `Lire`, `Telecharger`), et la page qui défile (`FormeEcran::Page`).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod cibles;
#[cfg(target_arch = "wasm32")]
mod composants;
#[cfg(target_arch = "wasm32")]
mod demandes;
pub mod deplacables;
#[cfg(target_arch = "wasm32")]
mod interaction;
pub mod messages;
#[cfg(target_arch = "wasm32")]
mod page;
pub mod saisie;
#[cfg(target_arch = "wasm32")]
mod toile;

#[cfg(target_arch = "wasm32")]
pub use page::demarrer_la_page;
#[cfg(target_arch = "wasm32")]
pub use toile::{ChoixMoteur, Ecran, attendre, maintenant_ms, texte_d_erreur};
