//! Le journal de la partie qui fait foi (§ 4.5, D3).
//!
//! Le journal suffit à rejouer la partie sans horloge : il garde, pour chaque pas
//! joué sur la définitive, les arrivées placées à ce pas, puis chaque entrée
//! appliquée avec son ordre et son issue, refus compris (D3). Ce que le cœur
//! synthétise (un relâchement, un `Temps`) y est écrit comme une entrée ordinaire.
//! Les sauts du temps et les pauses du commun, qui viennent de l'horloge, y sont
//! écrits aussi, comme chaque vue d'un pas neuf produite pour un regard, qui borne
//! ce que ses gestes peuvent prétendre avoir vu. Toutes les 256 entrées, il note l'empreinte de l'état (SAL-158).
//!
//! Le cœur est une fonction de son ouverture et de ce journal : reprendre, c'est
//! restaurer un point, puis rejouer le cœur entier sur les arrivées du journal, en
//! vérifiant que chaque entrée, chaque issue et chaque empreinte se retrouvent.

use alloc::vec::Vec;

use jeu_abi::{Appareil, IdGeste, Instant, Manette, Oui, Pas, Presence, SortieFil};

use crate::arrivee::{EcranArrivee, ReponseArrivee};
use crate::instance::Faute;
use crate::regles::{Ordonnable, Origine, Placee, ValeurTenue};

/// Le journal d'une soirée : les lignes de chaque partie, à la suite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Journal {
    pub(crate) lignes: Vec<Ligne>,
    /// Le dernier pas définitif : le rejeu joue jusque-là, pas vides compris.
    pub(crate) pas: Pas,
}

impl Journal {
    /// Un journal vide, au pas de la création.
    pub(crate) const fn vide(pas: Pas) -> Journal {
        Journal {
            lignes: Vec::new(),
            pas,
        }
    }

    /// Les lignes, dans l'ordre.
    pub fn lignes(&self) -> &[Ligne] {
        &self.lignes
    }

    /// Le dernier pas définitif.
    pub const fn pas(&self) -> Pas {
        self.pas
    }

    /// Les entrées appliquées, dans l'ordre.
    pub fn entrees(&self) -> impl Iterator<Item = &LigneEntree> {
        self.lignes.iter().filter_map(|l| match l {
            Ligne::Entree(e) => Some(e),
            Ligne::Arrivees { .. }
            | Ligne::Empreinte { .. }
            | Ligne::Saut { .. }
            | Ligne::Pause { .. }
            | Ligne::Vue { .. }
            | Ligne::Partie { .. }
            | Ligne::Reprise { .. } => None,
        })
    }
}

/// Une ligne du journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ligne {
    /// Les arrivées jouées à ce pas, dans l'ordre de leur numéro d'arrivée, avant
    /// [`crate::regles::ordonner`]. Écrite au début du pas, s'il en a.
    Arrivees {
        /// Le pas.
        pas: Pas,
        /// Les arrivées, placées.
        arrivees: Vec<ArriveePlacee>,
    },
    /// Une entrée appliquée à la définitive.
    Entree(LigneEntree),
    /// L'empreinte de la définitive, après l'entrée de cet ordre (SAL-158).
    Empreinte {
        /// La partie.
        partie: u32,
        /// L'ordre de l'entrée après laquelle elle est prise.
        ordre: u64,
        /// L'empreinte, telle que l'instance la donne.
        empreinte: [u8; 32],
    },
    /// Le numéro de pas a sauté après un gel : de `de`, joué, à `a`, sans jouer
    /// les pas entre deux (§ 4.1, D14).
    Saut {
        /// Le dernier pas joué.
        de: Pas,
        /// Le pas où reprend le numéro.
        a: Pas,
    },
    /// Une pause du commun commence ou finit, après ce pas (§ 4.1).
    Pause {
        /// Le dernier pas joué avant le changement.
        apres: Pas,
        /// La partie est-elle désormais en pause ?
        en_pause: Oui,
    },
    /// Une vue d'un pas neuf a été produite pour ce regard, après le pas définitif
    /// `apres` : ce qu'un geste de ce regard peut désormais prétendre avoir vu (§ 4.3,
    /// PUN-38).
    Vue {
        /// Le regard.
        regard: jeu_abi::RegardFil,
        /// Le pas de la vue : le dernier pas définitif pour un téléphone, celui de la
        /// tête pour l'écran quand elle est en avance (§ 4.4).
        pas: Pas,
        /// Le dernier pas définitif quand la vue a été produite.
        apres: Pas,
    },
    /// Une nouvelle partie commence, sur une instance neuve (CAT-73, CAT-75).
    Partie {
        /// Son numéro dans la soirée.
        numero: u32,
        /// Sa graine.
        graine: [u8; 32],
    },
    /// Après trois `Temps` écartés, la définitive repart du dernier point sain, sans
    /// les entrées qui le suivent (§ 4.6) : [`crate::Partie::repartir`].
    Reprise {
        /// Le dernier pas joué avant la reprise.
        apres: Pas,
        /// L'indice, dans le journal, du point sain d'où la définitive repart.
        depuis: usize,
    },
}

/// Une entrée appliquée à la définitive : son ordre, sa place, ses octets et son issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LigneEntree {
    /// La partie, dans la soirée.
    pub partie: u32,
    /// Son rang dans le journal de la partie : celui que reçoit `Appel::Entree`.
    pub ordre: u64,
    /// Le pas où elle s'applique.
    pub place: Pas,
    /// L'[`jeu_abi::EntreeFil`] encodée, telle que l'instance l'a reçue.
    pub octets: Vec<u8>,
    /// Ce que le jeu en a fait.
    pub issue: IssueJournal,
}

/// L'issue d'une entrée, au journal (§ 4.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueJournal {
    /// Acceptée, avec ses sorties.
    Acceptee {
        /// Les sorties, dans l'ordre.
        sorties: Vec<SortieFil>,
    },
    /// Refusée, avec l'avis encodé par le jeu.
    Refusee {
        /// L'avis.
        avis: Vec<u8>,
    },
    /// Ignorée.
    Ignoree,
    /// Le jeu a fauté : l'entrée est écartée, et ni un rejeu ni une reprise ne la
    /// rejouent (§ 4.6).
    Ecartee {
        /// La faute.
        faute: Faute,
    },
}

/// Une arrivée placée : ce que le cœur garde jusqu'à son pas, puis au journal.
///
/// Tout ce qui dépend de l'horloge ou du réseau (la demi-latence, la dernière vue
/// envoyée) est déjà appliqué : le rejeu n'en a pas besoin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArriveePlacee {
    /// Son numéro d'arrivée, unique dans la soirée.
    pub sequence: u64,
    /// Le pas où elle s'applique.
    pub place: Pas,
    /// L'instant borné où elle a été faite.
    pub fait: Instant,
    /// L'instant où la partie qui fait foi l'a reçue.
    pub recu: Instant,
    /// Le pas de la vue sur laquelle elle a été faite, borné.
    pub vu: Pas,
    /// D'où elle vient.
    pub origine: Origine,
    /// Ce qu'elle porte.
    pub quoi: QuoiPlace,
}

impl Ordonnable for ArriveePlacee {
    fn placee(&self) -> Placee {
        Placee {
            fait: self.fait,
            origine: self.origine,
            sequence: self.sequence,
        }
    }
}

/// Ce que porte une arrivée placée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuoiPlace {
    /// Un geste.
    Geste {
        /// L'appareil.
        de: Appareil,
        /// Le geste, encodé par le jeu, validé.
        octets: Vec<u8>,
        /// Son identité.
        id: IdGeste,
    },
    /// Un tenu de téléphone.
    Tenu {
        /// L'appareil.
        de: Appareil,
        /// Son numéro.
        numero: u64,
        /// Sa valeur, avec ce que le jeu en a dit à la validation.
        valeur: ValeurTelephone,
    },
    /// L'état d'une manette.
    Manette {
        /// L'appareil.
        de: Appareil,
        /// Son numéro.
        numero: u64,
        /// Son état.
        etat: Manette,
    },
    /// Une entrée de l'écran, admise par la scène.
    Ecran(EcranArrivee),
    /// Une présence.
    Presence(Presence),
    /// Une réponse.
    Reponse {
        /// La demande, par son identité.
        id: u64,
        /// La réponse.
        reponse: ReponseArrivee,
    },
}

/// La valeur d'un tenu de téléphone, telle que le jeu l'a validée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValeurTelephone {
    /// La valeur, encodée par le jeu.
    pub octets: Vec<u8>,
    /// Sa clé, encodée : un tenu par clé et par appareil.
    pub cle: Vec<u8>,
    /// Sa valeur relâchée, encodée, si elle se relâche seule.
    pub relache: Option<Vec<u8>>,
    /// Est-elle déjà relâchée ?
    pub est_relache: Oui,
}

impl ValeurTenue for ValeurTelephone {
    fn relachee(&self) -> Option<Self> {
        self.relache.as_ref().map(|r| ValeurTelephone {
            octets: r.clone(),
            cle: self.cle.clone(),
            relache: Some(r.clone()),
            est_relache: Oui::Oui,
        })
    }

    fn est_relachee(&self) -> Oui {
        self.est_relache
    }
}

impl ValeurTenue for Manette {
    fn relachee(&self) -> Option<Self> {
        Some(Manette::relachee(self))
    }

    fn est_relachee(&self) -> Oui {
        Manette::est_relachee(self)
    }
}
