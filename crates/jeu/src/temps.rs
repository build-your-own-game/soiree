//! Le temps du jeu (§ 2.4) : [`Maintenant`], qu'on ne garde pas, et [`Echeance`], qui
//! n'en naît que (D15, PUN-34).
//!
//! Il n'existe ni `Pas + Duree` ni `Echeance + Duree` : une phase qui suit une autre
//! part de `Maintenant`, jamais de l'échéance d'avant. Après un gel, une seule phase se
//! termine, et la suivante reçoit toute sa durée.

use core::marker::PhantomData;

use jeu_abi::{Duree, Oui, Pas, ReveilFil};

use crate::cache::{Ecrivain, Illisible, Liseur, forme};
use crate::forme::{Contient, Etat, Forme, Message};
use crate::secret::Regard;

/// L'instant de l'entrée en cours. Ni `Etat`, ni `Clone`, ni `Copy` : il ne se garde
/// pas, et sa durée de vie est celle de l'entrée.
#[derive(Debug)]
pub struct Maintenant<'e> {
    pas: Pas,
    _entree: PhantomData<&'e ()>,
}

impl Maintenant<'_> {
    /// Réservé à la colle : l'instant de l'entrée en cours.
    pub(crate) const fn new(pas: Pas) -> Maintenant<'static> {
        Maintenant {
            pas,
            _entree: PhantomData,
        }
    }

    /// Le pas de l'entrée en cours.
    pub const fn pas(&self) -> Pas {
        self.pas
    }

    /// La durée écoulée depuis `p` ; zéro si `p` est à venir.
    pub const fn depuis(&self, p: Pas) -> Duree {
        Duree::pas(self.pas.numero().saturating_sub(p.numero()))
    }

    /// Une copie de durée de vie plus courte, pour la prêter.
    pub(crate) const fn preter(&self) -> Maintenant<'_> {
        Maintenant {
            pas: self.pas,
            _entree: PhantomData,
        }
    }
}

/// Une échéance. Se garde dans l'état. Ne naît que depuis [`Maintenant`] (PUN-34).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct Echeance(Pas);

impl Echeance {
    /// L'échéance à `d` de maintenant.
    pub const fn dans(m: &Maintenant<'_>, d: Duree) -> Echeance {
        Echeance(Pas::du_fil(m.pas.numero().saturating_add(d.en_pas())))
    }

    /// Est-elle atteinte maintenant ?
    pub const fn atteinte(&self, m: &Maintenant<'_>) -> Oui {
        self.atteinte_a(m.pas)
    }

    /// Est-elle atteinte au pas `p` ? Pour `quand.recu` d'un brouillon.
    pub const fn atteinte_a(&self, p: Pas) -> Oui {
        if p.numero() >= self.0.numero() {
            Oui::Oui
        } else {
            Oui::Non
        }
    }

    /// Ce qui reste d'ici l'échéance ; zéro si elle est passée.
    pub const fn restant(&self, m: &Maintenant<'_>) -> Duree {
        Duree::pas(self.0.numero().saturating_sub(m.pas.numero()))
    }

    /// Repoussée d'une durée mesurée, comme une pause (Sidequest, cahier 4.8).
    #[must_use]
    pub const fn repoussee(self, d: Duree) -> Echeance {
        Echeance(Pas::du_fil(self.0.numero().saturating_add(d.en_pas())))
    }

    /// Son pas.
    pub const fn pas(&self) -> Pas {
        self.0
    }
}

impl Forme for Echeance {
    const EMPREINTE: u64 = forme::nommee("Echeance");
    const SECRET: Contient = Contient::Non;
    const VIDE: Contient = Contient::Non;
}

impl Etat for Echeance {
    fn projeter(&self, _: &Regard) -> Self {
        *self
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        e.message(self);
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        l.message()
    }
}

impl Message for Echeance {}

/// Quand le jeu veut recevoir le temps. Lu après chaque entrée acceptée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reveil {
    /// Un pas à chaque pas (temps réel, glisser de Tier list).
    ChaquePas,
    /// Un seul pas, à l'échéance, ou au réveil si elle est passée.
    A(Echeance),
    /// Aucun pas : le jeu n'avance que sur des entrées.
    Jamais,
}

impl Reveil {
    /// Sa forme sur le fil.
    pub(crate) const fn fil(self) -> ReveilFil {
        match self {
            Reveil::ChaquePas => ReveilFil::ChaquePas,
            Reveil::A(e) => ReveilFil::A(e.0.numero()),
            Reveil::Jamais => ReveilFil::Jamais,
        }
    }
}
