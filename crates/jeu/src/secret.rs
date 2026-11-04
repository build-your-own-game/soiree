//! Les secrets (§ 2.8) : [`Secret`], [`Qui`], et les deux jetons qui les ouvrent,
//! [`Autorite`] et [`Regard`].
//!
//! | Porte | Qui l'a | Ce qu'elle ouvre |
//! |---|---|---|
//! | `lire`, `lire_mut` avec `Autorite` | l'étape, la création, `resume`, `invariants` | tout, sur la partie qui fait foi ; ce que la projection contient, sur un appareil |
//! | `voir` avec `Regard` | les vues, les bots | ce que ce regard voit |
//! | `montrer` | l'étape, après `accepter` | change qui voit |

use core::fmt;

use jeu_abi::{Appareil, Oui, RegardFil};

use crate::cache::{Ecrivain, Illisible, Liseur, forme};
use crate::forme::{Contient, Etat, Forme, Message};

/// Le jeton qui ouvre un secret. Seuls l'étape, la création et les invariants le
/// reçoivent. Ni `Clone`, ni `Copy`, ni constructeur public.
#[derive(Debug)]
pub struct Autorite {
    _prive: (),
}

impl Autorite {
    /// Réservé à la colle : le seul jeton, prêté à l'étape.
    pub(crate) const fn new() -> Autorite {
        Autorite { _prive: () }
    }
}

/// Qui regarde : l'écran, ou un téléphone. Construit par le commun seul.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Regard {
    qui: RegardFil,
}

impl Regard {
    /// Réservé à la colle : le regard de ce qui regarde, relu sur le fil. Privé à
    /// `jeu` : une vue ne peut pas forger le regard d'un joueur pour ouvrir ses
    /// secrets (§ 2.8, CONTRAT « Un secret ne quitte pas l'ordinateur de la soirée »).
    pub(crate) const fn du_fil(qui: RegardFil) -> Regard {
        Regard { qui }
    }

    /// Est-ce l'écran ?
    pub const fn ecran(&self) -> Oui {
        match self.qui {
            RegardFil::Ecran => Oui::Oui,
            RegardFil::Appareil(_) => Oui::Non,
        }
    }

    /// L'appareil, si c'est un téléphone.
    pub const fn appareil(&self) -> Option<Appareil> {
        match self.qui {
            RegardFil::Ecran => None,
            RegardFil::Appareil(n) => Some(Appareil::du_fil(n)),
        }
    }

    /// Ce regard voit-il ce que `qui` peut voir ?
    pub fn voit(&self, qui: &Qui) -> Oui {
        let voit = match (qui, self.qui) {
            (Qui::Tous, _) | (Qui::Ecran, RegardFil::Ecran) => true,
            (Qui::Appareil(a), RegardFil::Appareil(n)) => a.fil() == n,
            (Qui::Appareils(l), RegardFil::Appareil(n)) => l.iter().any(|a| a.fil() == n),
            (Qui::Personne, _)
            | (Qui::Ecran, RegardFil::Appareil(_))
            | (Qui::Appareil(_) | Qui::Appareils(_), RegardFil::Ecran) => false,
        };
        if voit { Oui::Oui } else { Oui::Non }
    }
}

/// Le secret n'est pas lisible ici. Dans l'étape, `?` le convertit en `attendre`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Absent;

/// Au plus huit appareils qui voient un secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(
    try_from = "alloc::vec::Vec<Appareil>",
    into = "alloc::vec::Vec<Appareil>"
)]
pub struct Lecteurs {
    appareils: [Option<Appareil>; Lecteurs::MAX],
}

impl Lecteurs {
    /// Le plus d'appareils.
    pub const MAX: usize = 8;

    /// Personne.
    pub const fn aucun() -> Lecteurs {
        Lecteurs {
            appareils: [None; Lecteurs::MAX],
        }
    }

    /// Ces appareils, s'il y en a au plus huit, chacun une fois.
    pub fn de(appareils: &[Appareil]) -> Option<Lecteurs> {
        let mut l = Lecteurs::aucun();
        for a in appareils {
            if !l.ajouter(*a) {
                return None;
            }
        }
        Some(l)
    }

    /// Ajoute un appareil. Rend `false` s'il n'y a plus de place ; un appareil déjà
    /// là n'est pas compté deux fois.
    pub fn ajouter(&mut self, a: Appareil) -> bool {
        if self.contient(a) {
            return true;
        }
        match self.appareils.iter_mut().find(|c| c.is_none()) {
            Some(case) => {
                *case = Some(a);
                true
            }
            None => false,
        }
    }

    /// Cet appareil en est-il ?
    pub fn contient(&self, a: Appareil) -> bool {
        self.iter().any(|b| b == a)
    }

    /// Les appareils, dans l'ordre où ils ont été ajoutés.
    pub fn iter(&self) -> impl Iterator<Item = Appareil> + '_ {
        self.appareils.iter().filter_map(|a| *a)
    }
}

impl TryFrom<alloc::vec::Vec<Appareil>> for Lecteurs {
    type Error = jeu_abi::HorsBornes;

    fn try_from(v: alloc::vec::Vec<Appareil>) -> Result<Self, Self::Error> {
        let l = Lecteurs::de(&v).ok_or(jeu_abi::HorsBornes {
            attendu: "au plus huit appareils",
        })?;
        if l.iter().count() == v.len() {
            Ok(l)
        } else {
            Err(jeu_abi::HorsBornes {
                attendu: "chaque appareil une fois",
            })
        }
    }
}

impl From<Lecteurs> for alloc::vec::Vec<Appareil> {
    fn from(l: Lecteurs) -> Self {
        l.iter().collect()
    }
}

/// Qui voit un secret. Cette information est publique : on sait qu'un secret existe,
/// et qui le voit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Qui {
    /// Personne : seule l'autorité le lit.
    Personne,
    /// Un téléphone.
    Appareil(Appareil),
    /// Quelques téléphones.
    Appareils(Lecteurs),
    /// L'écran de l'ordinateur de la soirée.
    Ecran,
    /// Tout le monde.
    Tous,
}

impl Forme for Lecteurs {
    const EMPREINTE: u64 = forme::nommee("Lecteurs");
    const SECRET: Contient = Contient::Non;
    const VIDE: Contient = Contient::Non;
}
impl Etat for Lecteurs {
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
impl Message for Lecteurs {}

impl Forme for Qui {
    const EMPREINTE: u64 = forme::nommee("Qui");
    const SECRET: Contient = Contient::Non;
    const VIDE: Contient = Contient::Non;
}
impl Etat for Qui {
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
impl Message for Qui {}

/// Une valeur que tout le monde ne voit pas.
///
/// Sur la partie qui fait foi, la valeur est toujours là. Sur un appareil, la
/// projection l'a vidée si son regard ne la voit pas : [`Secret::lire`] rend alors
/// [`Absent`]. `Secret` n'est pas un `Message` : une vue ne peut pas en contenir.
#[derive(Clone)]
pub struct Secret<T: Etat> {
    qui: Qui,
    valeur: Option<T>,
}

impl<T: Etat> Secret<T> {
    /// Cache `valeur` : seuls la voient l'autorité et `qui`.
    pub fn cacher(qui: Qui, valeur: T) -> Secret<T> {
        Secret {
            qui,
            valeur: Some(valeur),
        }
    }

    /// La valeur, pour l'étape, la création, `resume` et `invariants`.
    pub fn lire(&self, _: &Autorite) -> Result<&T, Absent> {
        self.valeur.as_ref().ok_or(Absent)
    }

    /// La valeur, à modifier, pour l'étape après `accepter`.
    pub fn lire_mut(&mut self, _: &Autorite) -> Result<&mut T, Absent> {
        self.valeur.as_mut().ok_or(Absent)
    }

    /// Pour une vue ou un bot : la valeur, seulement si ce regard a le droit de la voir.
    pub fn voir(&self, r: &Regard) -> Option<&T> {
        match r.voit(&self.qui) {
            Oui::Oui => self.valeur.as_ref(),
            Oui::Non => None,
        }
    }

    /// Change qui voit. Une révélation est un changement d'état, fait par l'étape.
    pub fn montrer(&mut self, qui: Qui) {
        self.qui = qui;
    }

    /// Qui voit.
    pub fn qui(&self) -> &Qui {
        &self.qui
    }
}

/// `<secret>` : le `Debug` d'un état n'écrit jamais la valeur d'un secret.
impl<T: Etat> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<secret>")
    }
}

impl<T: Etat> Forme for Secret<T> {
    const EMPREINTE: u64 = forme::de("Secret", T::EMPREINTE);
    const SECRET: Contient = Contient::Oui;
    const VIDE: Contient = Contient::Non;
}

impl<T: Etat> Etat for Secret<T> {
    fn projeter(&self, r: &Regard) -> Self {
        Secret {
            qui: self.qui,
            valeur: match r.voit(&self.qui) {
                Oui::Oui => self.valeur.as_ref().map(|v| v.projeter(r)),
                Oui::Non => None,
            },
        }
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        self.qui.ecrire(e);
        self.valeur.ecrire(e);
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        Ok(Secret {
            qui: Qui::lire(l)?,
            valeur: Option::<T>::lire(l)?,
        })
    }
}
