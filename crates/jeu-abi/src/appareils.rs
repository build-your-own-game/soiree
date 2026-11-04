//! Appareils et présence (§ 2.5).

use serde::{Deserialize, Serialize};

use crate::valeurs::TexteCourt;

/// Un appareil, créé par le commun seul. Ni `Ord`, ni `Hash` : un jeu ne peut pas
/// départager par l'identité. Ses valeurs sont tirées au hasard pour chaque soirée :
/// elles ne départagent rien (CAT-27).
///
/// L'écran de l'ordinateur de la soirée n'est pas un `Appareil` : un téléphone ne
/// peut pas l'usurper (SAL-33, SQ-21, PUN-48).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Appareil(u64);

impl Appareil {
    /// Réservé au commun : l'appareil de ce numéro, tiré pour la soirée.
    #[doc(hidden)]
    pub const fn du_fil(n: u64) -> Appareil {
        Appareil(n)
    }

    /// Réservé au commun : le numéro sur le fil.
    #[doc(hidden)]
    pub const fn fil(self) -> u64 {
        self.0
    }
}

/// Un appareil arrive, part, revient ou change de capacités. Le commun annonce ;
/// le jeu décide. Il ne libère jamais une place seul.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Presence {
    /// Un appareil jamais vu, ou oublié. Ses capacités présentes, pas seulement
    /// leurs changements (REM-43).
    Arrivee {
        /// L'appareil.
        appareil: Appareil,
        /// Ce qu'il est.
        role: RoleAppareil,
        /// Ce qu'il sait faire, à son arrivée.
        capacites: Capacites,
    },
    /// Un appareil se tait ou se ferme. Il garde sa place jusqu'à ce que le jeu l'oublie.
    Depart {
        /// L'appareil.
        appareil: Appareil,
        /// Pourquoi on le dit parti.
        cause: CauseDepart,
    },
    /// Un appareil parti revient, avec ses capacités présentes.
    Retour {
        /// L'appareil.
        appareil: Appareil,
        /// Ce qu'il sait faire, à son retour.
        capacites: Capacites,
    },
    /// Les capacités d'un appareil présent changent.
    Capacites {
        /// L'appareil.
        appareil: Appareil,
        /// Toutes ses capacités présentes.
        capacites: Capacites,
    },
}

/// Ce qu'est un appareil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoleAppareil {
    /// Un téléphone, dans un navigateur ou une app.
    Telephone,
    /// Une manette branchée à l'ordinateur de la soirée.
    Manette,
}

/// Pourquoi un appareil est dit parti.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CauseDepart {
    /// Plus de nouvelles depuis trop longtemps.
    Silence,
    /// Le lien s'est fermé.
    Fermeture,
    /// Le joueur est parti exprès.
    Voulu,
}

/// Des capacités, jamais une technologie. Toutes optionnelles : un jeu reste
/// jouable sans aucune (REM-146, OUT-211).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capacites {
    /// Faire vibrer.
    pub vibration: Capacite,
    /// Les capteurs de mouvement, dans les axes de l'interface (REM-137).
    pub mouvement: Capacite,
    /// Jouer un son sans geste préalable.
    pub son_sans_geste: Capacite,
    /// Garder l'écran allumé.
    pub ecran_allume: Capacite,
    /// Le micro : jamais sur une adresse locale en http (OUT-211).
    pub micro: Capacite,
    /// Une manette tactile native de l'app.
    pub pad_natif: Capacite,
    /// L'orientation de l'écran telle qu'elle est, demi-tour compris (REM-44).
    pub orientation: Orientation,
    /// Un prénom que l'appareil propose, à nettoyer par le jeu.
    pub prenom_propose: Option<TexteCourt>,
}

impl Capacites {
    /// Aucune capacité, en portrait, sans prénom : celles avec lesquelles l'épreuve joue.
    pub const AUCUNE: Capacites = Capacites {
        vibration: Capacite::Non(Absence::NonSupportee),
        mouvement: Capacite::Non(Absence::NonSupportee),
        son_sans_geste: Capacite::Non(Absence::NonSupportee),
        ecran_allume: Capacite::Non(Absence::NonSupportee),
        micro: Capacite::Non(Absence::NonSupportee),
        pad_natif: Capacite::Non(Absence::NonSupportee),
        orientation: Orientation::Portrait,
        prenom_propose: None,
    };
}

/// Une capacité présente, ou absente avec sa cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Capacite {
    /// Présente.
    Oui,
    /// Absente.
    Non(Absence),
}

/// Pourquoi une capacité manque.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Absence {
    /// L'appareil ne l'a pas.
    NonSupportee,
    /// La page n'est pas dans un contexte sécurisé.
    ContexteNonSecurise,
    /// Le joueur l'a refusée.
    Refusee,
    /// Pas encore accordée : elle viendra peut-être après un geste.
    PasEncore,
}

/// L'orientation de l'écran d'un appareil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Orientation {
    /// Portrait, à l'endroit.
    Portrait,
    /// Portrait, retourné.
    PortraitInverse,
    /// Paysage, tourné vers la gauche.
    PaysageGauche,
    /// Paysage, tourné vers la droite.
    PaysageDroite,
}
