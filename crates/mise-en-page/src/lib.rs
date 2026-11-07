//! La mesure des textes de la scène (PHASE-3, § 2.9, P21, P22) : la coupe en lignes, la
//! taille qui rétrécit pour tenir (`Taille::Budget`), les débordements. Elle sert
//! l'épreuve, où E16 mesure les textes des pires cas de l'écran, et, au lot 9, la télé,
//! qui dessine exactement l'agencement mesuré.
//!
//! - [`Polices`] : la police du commun ([`OCTETS_DU_COMMUN`], Liberation Sans sous OFL)
//!   et celles du jeu, validées par [`valider_police`] : des fichiers sfnt, TTF ou OTF,
//!   jamais WOFF ni WOFF2 (P22, E1). Jamais une police du système.
//! - [`MiseEnPage::agencer`] : un texte, sa boîte et son style donnent un [`Agencement`] :
//!   le texte coupé, la taille retenue, les lignes et leurs glyphes, ce qui tient ou non
//!   ([`Tient`], [`Debord`]), les glyphes absents.
//!
//! La règle de l'agencement (taille, interligne, coupes, SQ-46, TL-55) est écrite en tête
//! de [`agencement`]. La mise en forme passe par `cosmic-text`, sans hinting, dans le
//! repère de la scène, avec des polices de repli qui ne dépendent pas de la plateforme ;
//! le jeu ne lit jamais une mesure (CONTRAT, « Le GPU »).
//!
//! Les flottants sont ceux de `cosmic-text` : cette crate n'entre jamais dans un module,
//! et ses résultats ne vont qu'au jugement de E16 et au dessin.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod agencement;
mod polices;

pub use agencement::{
    AGENCEMENTS_GARDES_MAX, Agencement, Debord, GlypheAgence, INTERLIGNE, LigneAgencee, MiseEnPage,
    Tient,
};
pub use polices::{FAMILLE_DU_COMMUN, OCTETS_DU_COMMUN, Polices, RefusPolice, valider_police};
