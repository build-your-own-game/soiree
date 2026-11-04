//! Les jeux fautifs de l'épreuve (INTERFACE.md, § 7.2 et § 8.8) : un jeu par contrôle
//! négatif, qui doit faire échouer exactement le contrôle qu'il vise, et aucun autre
//! (OUT-27, HIS-03, SAL-168).
//!
//! Tous sont le même petit jeu, [`socle::Socle`], à un défaut près
//! ([`defauts`]) : `Socle<Sain>` passe tous les contrôles, et chaque défaut
//! change une seule règle. Le contrôle négatif mesure ainsi cette règle, et rien
//! d'autre.
//!
//! Chaque jeu a sa crate de module dans `modules/<nom>/`, que le workspace prend par le
//! motif `crates/exemples/fautifs/modules/*`. Leur `build.rs` commun,
//! `modules/build.rs`, pose le maximum de mémoire.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod defauts;
pub mod socle;

pub use socle::Socle;

/// Les jeux, un par défaut, sous le nom de leur crate de module.
pub mod jeux {
    use crate::defauts;
    use crate::socle::Socle;

    /// Le jeu sain.
    pub type Sain = Socle<defauts::Sain>;
    /// E1.
    pub type DeclarationIncoherente = Socle<defauts::DeclarationIncoherente>;
    /// E2.
    pub type JamaisFini = Socle<defauts::JamaisFini>;
    /// E3.
    pub type PhaseInaccessible = Socle<defauts::PhaseInaccessible>;
    /// E4.
    pub type InvariantViole = Socle<defauts::InvariantViole>;
    /// E5.
    pub type AttendUnAbsent = Socle<defauts::AttendUnAbsent>;
    /// E6.
    pub type RefuseApresAvoirEcrit = Socle<defauts::RefuseApresAvoirEcrit>;
    /// E7.
    pub type VueQuiEcrit = Socle<defauts::VueQuiEcrit>;
    /// E8.
    pub type VueQuiFuit = Socle<defauts::VueQuiFuit>;
    /// E9.
    pub type NatifDifferent = Socle<defauts::NatifDifferent>;
    /// E13.
    pub type OctetQuiPanique = Socle<defauts::OctetQuiPanique>;
    /// E14.
    pub type EtatHorsEnveloppe = Socle<defauts::EtatHorsEnveloppe>;
    /// E15.
    pub type FuitLaMemoire = Socle<defauts::FuitLaMemoire>;
    /// E10.
    pub type DependDeLAppareil = Socle<defauts::DependDeLAppareil>;
    /// E11.
    pub type CoupureQuiCompte = Socle<defauts::CoupureQuiCompte>;
    /// Sain, avec une fenêtre.
    pub type AvecFenetre = Socle<defauts::AvecFenetre>;
    /// E12.
    pub type CompteLeRecu = Socle<defauts::CompteLeRecu>;
    /// E16.
    pub type PireCasQuiDeborde = Socle<defauts::PireCasQuiDeborde>;
    /// E17.
    pub type NanDansLEtat = Socle<defauts::NanDansLEtat>;
    /// E18.
    pub type GelQuiEnchaine = Socle<defauts::GelQuiEnchaine>;
    /// E19.
    pub type BoucleAuVote = Socle<defauts::BoucleAuVote>;
}
