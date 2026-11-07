//! Le dessin de la scène de la télé sur le GPU (PHASE-3, § 2.9, lot 9).
//!
//! - [`cadre`] : où la scène se pose sur la toile (zone sûre, échelle entière, page).
//! - [`liste`] : la scène mise à plat, en pixels, dans l'ordre du peintre, sans GPU.
//! - [`qr`] : le QR code de l'adresse, en modules (SAL-191).
//! - [`gpu`] : les structures `#[repr(C)]` envoyées au GPU et leur programme WGSL, une
//!   seule définition (CONTRAT, « Les types de bout en bout »).
//! - [`rendu`] : le dessin sur `wgpu` dans une texture à soi, les textes par `glyphon`
//!   d'après l'agencement de `mise-en-page`, la relecture des pixels, la reprise après
//!   la perte du GPU.
//!
//! En natif, le rendu tourne sur Metal pour ses tests (`tests/`) ; dans le navigateur, sur
//! WebGPU ou WebGL2, pour la page de la télé (`web/ecran`). Le jeu ne lit jamais rien de
//! ce qui est dessiné (CONTRAT, « Le GPU »).
//!
//! **Les règles d'écriture.** Le code de rendu s'exempte, avec sa raison, des règles
//! d'arithmétique et de conversion des nombres (CONTRAT, « Les règles d'écriture » : « Le
//! code de rendu peut s'exempter des règles de déterminisme et d'arithmétique, en tête de
//! crate ou de module, avec sa raison ») : des pixels se calculent en flottants et en
//! petits entiers, et rien de ce qu'ils deviennent ne revient à la partie.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![expect(
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "code de rendu (CONTRAT, « Les règles d'écriture ») : des pixels en flottants et en petits entiers, jamais une décision de la partie"
)]

pub mod cadre;
pub mod gpu;
pub mod liste;
pub mod qr;
pub mod rendu;

pub use cadre::{Affine, Cadre};
pub use liste::{Contexte, Element, Fautif, Liste, Sorte, mettre_a_plat};
pub use rendu::{Dessin, FORMAT, IMAGE_ABSENTE, ImageRgba, Relecture, Rendu, TexteDessine};

pub use glyphon;
pub use wgpu;
