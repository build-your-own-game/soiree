//! # La brique « cartouche »
//!
//! Le contrat entre un jeu et la console (voir `CONTRAT.md`). La console est
//! le programme ; le jeu est une cartouche : une machine, qui remplit
//! [`Machine`], et des pages.
//!
//! La machine ne voit rien du système. Tout ce qui lui arrive passe par une
//! seule porte, [`Machine::apply`], et tout son état s'écrit en octets
//! ([`State`]). C'est ce qui permet à la console de jouer des parties toute
//! seule, de les photographier, de les reprendre et de les rejouer.
//!
//! Cette brique se passe du système, comme la machine qui l'emprunte. Le
//! banc ([`bench`]) et le journal ([`journal`]) le demandent : ils ne sont là
//! que derrière leur option.

#![no_std]

extern crate alloc;
#[cfg(feature = "journal")]
extern crate std;

#[cfg(feature = "bench")]
pub mod bench;
pub mod draw;
#[cfg(feature = "journal")]
pub mod journal;
mod notes;
pub mod rng;
mod state;

use alloc::vec::Vec;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub use notes::{Note, note, take_notes};
pub use state::{State, fingerprint};

/// Ce qu'est une page pour le jeu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// L'écran, sur l'ordinateur de l'hôte.
    Screen,
    /// La télécommande d'un joueur, sur son téléphone.
    Remote,
}

impl Role {
    /// Le nom du rôle dans l'adresse du lien.
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Screen => "screen",
            Role::Remote => "remote",
        }
    }
}

/// Une page : qui elle est, et ce qu'elle est. La même page garde le même
/// numéro quand elle se recharge ou revient de veille.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Page {
    pub id: u64,
    pub role: Role,
}

impl Page {
    /// L'écran qui porte ce numéro.
    pub fn screen(id: u64) -> Self {
        Self {
            id,
            role: Role::Screen,
        }
    }

    /// La télécommande qui porte ce numéro.
    pub fn remote(id: u64) -> Self {
        Self {
            id,
            role: Role::Remote,
        }
    }
}

/// Tout ce qui peut arriver à la partie. Il n'y a pas d'autre porte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input<Command> {
    /// Une page arrive ou revient.
    Joined(Page),
    /// Une page n'est plus là.
    Left(Page),
    /// Une page a envoyé une commande.
    Message(Page, Command),
    /// Un pas de temps s'est écoulé.
    Tick,
}

/// À qui va un effet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum To {
    /// Cette page seulement.
    Page(Page),
    /// Tous les écrans.
    Screens,
    /// Toutes les télécommandes.
    Remotes,
    /// Toutes les pages.
    Everyone,
}

impl To {
    /// Cette page fait-elle partie des destinataires ?
    pub fn reaches(self, page: Page) -> bool {
        match self {
            To::Page(one) => one == page,
            To::Screens => page.role == Role::Screen,
            To::Remotes => page.role == Role::Remote,
            To::Everyone => true,
        }
    }
}

/// Le type des effets d'un jeu qui n'en a pas : aucune valeur n'existe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoEffect {}

/// La machine d'un jeu : ses règles, et rien d'autre.
///
/// Mêmes entrées, dans le même ordre, à partir de la même graine : même
/// partie, sur l'ordinateur comme dans un navigateur.
pub trait Machine: State {
    /// Ce qu'une page peut envoyer.
    type Command: DeserializeOwned;
    /// Ce qu'une page doit montrer maintenant.
    type View: Serialize;
    /// Ce qu'une page joue une fois, et qu'on peut rater sans rien perdre :
    /// un son, une vibration, un « trop tard ». Sans effets : [`NoEffect`].
    type Effect: Serialize;

    /// Le rythme du jeu : le nombre de [`Input::Tick`] par seconde. 0 : le
    /// temps ne compte pas.
    const TICKS_PER_SECOND: u32;
    /// Vrai : la console garde l'état d'un lancement à l'autre.
    const REMEMBERS: bool = false;

    /// Une partie neuve. La graine est tout le hasard du jeu.
    fn new(seed: u64) -> Self;

    /// La seule façon de changer la partie. Rend ce que les pages jouent une
    /// fois ; tout ce qui dure est dans l'état, et se lit par [`Machine::view`].
    fn apply(&mut self, input: Input<Self::Command>) -> Vec<(To, Self::Effect)>;

    /// Ce que cette page doit montrer. Une page inconnue ne montre rien.
    fn view(&self, page: Page) -> Option<Self::View>;

    /// Ce que cette page pourrait raisonnablement envoyer maintenant : ce que
    /// ses boutons enverraient. La console s'en sert pour jouer des parties
    /// qui avancent.
    fn suggest(&self, page: Page) -> Vec<Self::Command>;
}

/// Note un cas qui n'aurait pas dû arriver, et continue.
///
/// Dans les tests et sur le banc, la note arrête tout : on la voit. À la
/// soirée, la console l'écrit dans le journal et la partie continue.
///
/// ```ignore
/// let Some(seat) = self.seats.get_mut(&page.id) else {
///     cartouche::oups!("un joueur assis sans place");
///     return Vec::new();
/// };
/// ```
#[macro_export]
macro_rules! oups {
    ($what:expr) => {
        $crate::note($what)
    };
}
