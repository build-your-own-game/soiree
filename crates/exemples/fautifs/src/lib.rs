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
    /// E3 : une phase sans pire cas, jamais atteinte.
    pub type PhaseSansCas = Socle<defauts::PhaseSansCas>;
    /// E4 : un invariant déclaré, jamais évalué.
    pub type InvariantJamaisEvalue = Socle<defauts::InvariantJamaisEvalue>;
    /// E6 : un `static` écrit avant un refus, qui change la tape suivante.
    pub type RetientUnRefus = Socle<defauts::RetientUnRefus>;
    /// E8 : un secret d'un octet, que le choix public montre.
    pub type ChoixQuiFuit = Socle<defauts::ChoixQuiFuit>;
    /// E2 : trois `Temps` qui paniquent ; la partie doit repartir.
    pub type PaniqueAuTemps = Socle<defauts::PaniqueAuTemps>;
    /// E4, dans le monde réel : un choix jugé sur `fait`, pas sur `vu`.
    pub type JugeSurLeFait = Socle<defauts::JugeSurLeFait>;
    /// E13, dans le monde réel : un fichier abîmé lu sans vérifier sa taille.
    pub type FichierQuiPanique = Socle<defauts::FichierQuiPanique>;
    /// E19, sur les pires cas : une vue de l'écran que seuls les pires cas paient.
    pub type PireCasTropLourd = Socle<defauts::PireCasTropLourd>;
    /// E19, sur une lecture courante : la vue de la fin, trop lourde pour une image
    /// de 60 par seconde.
    pub type FinTropLourde = Socle<defauts::FinTropLourde>;
    /// E19, par le coût fixe : un brouillon de 32 Mio remis à zéro à chaque appel.
    pub type ZonesLourdes = Socle<defauts::ZonesLourdes>;
    /// E8 : un secret recopié dans un `static`, que l'écran montre.
    pub type SecretDansUnStatic = Socle<defauts::SecretDansUnStatic>;
    /// E16 : des pires cas plus légers que l'écran d'une soirée.
    pub type PireCasTropLeger = Socle<defauts::PireCasTropLeger>;
}
