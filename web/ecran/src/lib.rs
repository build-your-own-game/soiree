//! La page de la télé (PHASE-3, § 2.1, § 2.9) : elle reçoit la scène du Worker et la
//! dessine. Au lot 9, elle dessine une scène reçue ; l'autorité, les entrées, le son et
//! les composants du DOM viennent aux lots 10a à 13.
//!
//! - [`messages`] : ce qu'un message du Worker (`salle::ecran::VersPage`) apporte, sans
//!   navigateur ;
//! - `toile` (dans le navigateur seulement) : [`Ecran`], la toile, `wgpu` sur WebGPU ou
//!   WebGL2, le rendu de `web/rendu` copié sur la toile, la mesure du travail du GPU
//!   fini, la relecture des pixels, la reprise quand le GPU est perdu.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod messages;
#[cfg(target_arch = "wasm32")]
mod toile;

#[cfg(target_arch = "wasm32")]
pub use toile::{ChoixMoteur, Ecran, attendre, maintenant_ms, texte_d_erreur};
