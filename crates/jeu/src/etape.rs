//! L'étape (§ 2.6 et § 2.7) : les entrées, et la décision en deux temps.
//!
//! [`Avant`] ne donne que `&J` : avant d'accepter, le jeu ne peut ni écrire l'état, ni
//! tirer du hasard, ni émettre une sortie. [`Avant::refuser`] et [`Avant::accepter`]
//! consomment `Avant` : après avoir accepté, refuser ne s'écrit plus. Un refus ne change
//! rien, générateur compris, par la forme (D1 ; PUN-27, HIS-44, OUT-171, CAT-94, SQ-25,
//! TL-02). [`Issue`] n'a pas de constructeur public : on ne l'obtient qu'en refusant, en
//! ignorant, en attendant ou en finissant.

use alloc::vec::Vec;

use jeu_abi::{
    Appareil, CauseTenu, Composant, Duree, HeureMurale, IdDemande, IdGeste, Instant, Manette, Oui,
    Pas, Pointeur, Presence, Reponse, SortieFil, Touche,
};

use crate::forme::{Aucun, Message};
use crate::hasard::Hasard;
use crate::secret::{Absent, Autorite, Qui, Secret};
use crate::temps::Maintenant;
use crate::traits::Jeu;

/// Une entrée, une à une, dans l'ordre fixé par le commun.
#[derive(Debug)]
pub enum Entree<'a, J: Jeu> {
    /// Le temps : le pas en cours, livré seulement si `reveil()` le demande.
    Temps(Temps),
    /// L'écran de l'ordinateur de la soirée : sa scène, son clavier, sa souris.
    Ecran(DeEcran<'a, J::ActionEcran>),
    /// Un événement d'un téléphone : compté exactement une fois, dans l'ordre fixé.
    Geste(Geste<J::GesteTelephone>),
    /// Le changement d'un état tenu d'un téléphone : le dernier gagne.
    Tenu(Changement<J::TenuTelephone>),
    /// Le changement de l'état d'une manette.
    Manette(Changement<Manette>),
    /// Un appareil arrive, part, revient ou change.
    Presence(Presence),
    /// La réponse à une demande encore listée par le jeu. Jamais pour une demande retirée.
    Reponse(IdDemande, Reponse<'a>),
}

/// Le temps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Temps {
    /// Le temps jeté après un gel, au-delà du rattrapage déclaré. `Duree::pas(0)` d'habitude.
    pub saut: Duree,
    /// L'heure murale, pour étiqueter, jamais pour décider.
    pub heure: HeureMurale,
}

/// Quand et sur quoi un geste a été fait (D16).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quand {
    /// Le pas où le commun l'applique : celui de l'entrée en cours.
    pub place: Pas,
    /// L'instant estimé du geste, sur l'horloge de la partie, borné (§ 4.3).
    pub fait: Instant,
    /// Le pas où la partie qui fait foi l'a reçu.
    pub recu: Pas,
    /// Le pas de la dernière vue que l'appareil avait reçue. Borné par ce qui lui a été envoyé.
    pub vu: Pas,
    /// L'identité du geste, stable à travers renvois, rejeux et prédiction (§ 5.4).
    pub id: IdGeste,
}

/// Un geste d'un téléphone.
#[derive(Debug, Clone, PartialEq)]
pub struct Geste<G> {
    /// L'appareil.
    pub de: Appareil,
    /// Le geste.
    pub geste: G,
    /// Quand.
    pub quand: Quand,
}

/// Le changement d'un état tenu.
#[derive(Debug, Clone, PartialEq)]
pub struct Changement<T: Tenu> {
    /// L'appareil.
    pub de: Appareil,
    /// La valeur, complète.
    pub valeur: T,
    /// Pourquoi elle change.
    pub cause: CauseTenu,
    /// Quand.
    pub quand: Quand,
}

/// Un état tenu : complet, le dernier gagne, un par clé et par appareil (D17).
pub trait Tenu: Message + PartialEq {
    /// La clé : un tenu par clé et par appareil.
    type Cle: Message + Ord + Copy;
    /// La clé de cette valeur.
    fn cle(&self) -> Self::Cle;
    /// Cette valeur relâchée (le stick au centre, le bouton levé, compteur d'appuis
    /// gardé). `None` : ne se relâche jamais seul (un brouillon).
    fn relache(&self) -> Option<Self>;
}

impl Tenu for Aucun {
    type Cle = ();
    fn cle(&self) {
        match *self {}
    }
    fn relache(&self) -> Option<Self> {
        match *self {}
    }
}

impl Tenu for Manette {
    type Cle = ();
    fn cle(&self) {}
    fn relache(&self) -> Option<Self> {
        Some(self.relachee())
    }
}

/// Ce qui vient de l'écran de l'ordinateur de la soirée.
#[derive(Debug, Clone, PartialEq)]
pub enum DeEcran<'a, A> {
    /// Un bouton ou un raccourci que la scène affichée portait. Rien d'autre ne
    /// s'envoie (SAL-152).
    Action {
        /// L'action.
        action: A,
        /// Quand.
        quand: Quand,
    },
    /// Une touche brute, seulement si la scène l'a demandée.
    Touche {
        /// La touche.
        touche: Touche,
        /// Quand.
        quand: Quand,
    },
    /// Un pointeur brut, sur une surface que la scène a déclarée.
    Pointeur {
        /// Le pointeur.
        pointeur: Pointeur,
        /// Quand.
        quand: Quand,
    },
    /// Le résultat d'un composant de la scène.
    Composant {
        /// Ce qui arrive.
        evenement: Composant<'a>,
        /// Quand.
        quand: Quand,
    },
}

/// Ce que l'étape peut faire après avoir accepté : tirer du hasard, nommer une
/// demande, oublier ou retirer un appareil, commencer une nouvelle partie (D10).
#[derive(Debug, Clone)]
pub struct Sorties {
    pub(crate) hasard: Secret<Hasard>,
    /// Le prochain identifiant de demande.
    pub(crate) compteur: u64,
    /// Les sorties de l'entrée en cours, dans l'ordre.
    pub(crate) emises: Vec<SortieFil>,
    /// `hasard: Oui` dans la déclaration.
    pub(crate) hasard_declare: Oui,
    /// Le jeu a appelé `hasard` sans l'avoir déclaré : une faute.
    pub(crate) hasard_non_declare: Oui,
}

impl Sorties {
    pub(crate) fn new(hasard: Hasard, hasard_declare: Oui) -> Sorties {
        Sorties {
            hasard: Secret::cacher(Qui::Personne, hasard),
            compteur: 1,
            emises: Vec::new(),
            hasard_declare,
            hasard_non_declare: Oui::Non,
        }
    }

    /// Le générateur de la partie. C'est un secret : il manque sur un appareil, et un
    /// tirage y arrête la prédiction de ce geste. Un jeu qui déclare `hasard: SansObjet`
    /// et l'appelle commet une faute : l'issue devient `attendre`.
    pub fn hasard(&mut self) -> Result<&mut Hasard, Absent> {
        if self.hasard_declare == Oui::Non {
            self.hasard_non_declare = Oui::Oui;
            return Err(Absent);
        }
        self.hasard.lire_mut(&Autorite::new())
    }

    /// Un identifiant neuf, jamais redonné par cette partie, même par une annulation du
    /// jeu (TL-23).
    pub fn id(&mut self) -> IdDemande {
        let id = IdDemande::du_fil(self.compteur);
        self.compteur = self.compteur.saturating_add(1);
        id
    }

    /// Oublie un appareil parti : il reviendra comme un neuf (PUN-15, SQ-76).
    pub fn oublier(&mut self, a: Appareil) {
        self.emises.push(SortieFil::Oublier(a.fil()));
    }

    /// Retire un appareil présent : il lit « Tu as été retiré », et peut revenir comme
    /// un neuf.
    pub fn retirer(&mut self, a: Appareil) {
        self.emises.push(SortieFil::Retirer(a.fil()));
    }

    /// Une nouvelle partie : une instance neuve, avec `garder()` et les mêmes appareils
    /// (CAT-73, CAT-75). Sa graine est tirée du générateur de la partie ; sans hasard
    /// déclaré, elle est nulle et la nouvelle partie n'en a pas besoin.
    pub fn nouvelle_partie(&mut self) {
        let mut graine = [0u8; 32];
        if self.hasard_declare == Oui::Oui
            && let Ok(h) = self.hasard.lire_mut(&Autorite::new())
        {
            for mot in graine.as_chunks_mut::<8>().0 {
                *mot = h.entier().to_le_bytes();
            }
        }
        self.emises.push(SortieFil::NouvellePartie { graine });
    }
}

/// Ce que le jeu a décidé.
#[derive(Debug)]
pub(crate) enum Decision<J: Jeu> {
    Acceptee,
    Refusee(J::Avis),
    Ignoree,
    Attente,
}

/// Le résultat d'une étape. Sans constructeur public.
#[derive(Debug)]
pub struct Issue<J: Jeu> {
    pub(crate) decision: Decision<J>,
}

/// Avant la décision : lecture seule.
pub struct Avant<'e, J: Jeu> {
    jeu: &'e mut J,
    sorties: &'e mut Sorties,
    autorite: &'e Autorite,
    maintenant: Maintenant<'e>,
}

impl<'e, J: Jeu> Avant<'e, J> {
    pub(crate) fn new(
        jeu: &'e mut J,
        sorties: &'e mut Sorties,
        autorite: &'e Autorite,
        maintenant: Maintenant<'e>,
    ) -> Avant<'e, J> {
        Avant {
            jeu,
            sorties,
            autorite,
            maintenant,
        }
    }

    /// L'état, en lecture.
    pub fn jeu(&self) -> &J {
        self.jeu
    }

    /// Le jeton qui ouvre les secrets.
    pub fn autorite(&self) -> &Autorite {
        self.autorite
    }

    /// L'instant de l'entrée.
    pub fn maintenant(&self) -> Maintenant<'_> {
        self.maintenant.preter()
    }

    /// Accepter donne l'écriture. On ne peut plus refuser ensuite.
    pub fn accepter(self) -> Apres<'e, J> {
        Apres {
            jeu: self.jeu,
            sorties: self.sorties,
            autorite: self.autorite,
            maintenant: self.maintenant,
        }
    }

    /// L'état ne change pas ; l'avis va au seul appareil du geste, sans changer sa vue.
    pub fn refuser(self, avis: J::Avis) -> Issue<J> {
        Issue {
            decision: Decision::Refusee(avis),
        }
    }

    /// L'état ne change pas ; personne n'est prévenu.
    pub fn ignorer(self) -> Issue<J> {
        Issue {
            decision: Decision::Ignoree,
        }
    }

    /// Un secret manque : sur un appareil qui prédit, ce geste attend la confirmation.
    /// Sur la partie qui fait foi, c'est une faute du jeu.
    pub fn attendre(self, _: Absent) -> Issue<J> {
        Issue {
            decision: Decision::Attente,
        }
    }
}

/// Après la décision : écriture.
pub struct Apres<'e, J: Jeu> {
    jeu: &'e mut J,
    sorties: &'e mut Sorties,
    autorite: &'e Autorite,
    maintenant: Maintenant<'e>,
}

/// Tout ce que l'écriture demande, emprunté ensemble.
pub struct Parts<'p, J: Jeu> {
    /// L'état.
    pub jeu: &'p mut J,
    /// Le jeton qui ouvre les secrets.
    pub autorite: &'p Autorite,
    /// L'instant de l'entrée.
    pub maintenant: Maintenant<'p>,
    /// Le hasard, les identifiants et les sorties.
    pub sorties: &'p mut Sorties,
}

impl<J: Jeu> Apres<'_, J> {
    /// Tout ce que l'écriture demande, emprunté ensemble.
    pub fn parts(&mut self) -> Parts<'_, J> {
        Parts {
            jeu: self.jeu,
            autorite: self.autorite,
            maintenant: self.maintenant.preter(),
            sorties: self.sorties,
        }
    }

    /// L'état, en écriture.
    pub fn jeu(&mut self) -> &mut J {
        self.jeu
    }

    /// L'entrée est acceptée.
    pub fn fin(self) -> Issue<J> {
        Issue {
            decision: Decision::Acceptee,
        }
    }

    /// Un secret manque après avoir accepté : sur la partie qui fait foi, une faute ;
    /// l'état écrit jusque-là sera restauré par le commun.
    pub fn attendre(self, _: Absent) -> Issue<J> {
        Issue {
            decision: Decision::Attente,
        }
    }
}
