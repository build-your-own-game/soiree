//! Ce que les pages envoient au jeu (`Up`) et ce que le jeu leur répond (`Down`).
//!
//! La machine emprunte ces types : comme elle, ce crate se passe du système.

#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

/// D'une télécommande vers le jeu.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Up {
    /// Le joueur a choisi son prénom.
    Name(String),
    /// Le joueur a tapé sur le bouton.
    Tap,
}

/// Du jeu vers une page : ce qu'elle doit afficher maintenant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Down {
    Screen(Screen),
    Remote(Remote),
}

/// Ce que montre l'écran.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screen {
    /// L'adresse que les téléphones ouvrent.
    pub join_url: String,
    pub players: Vec<Player>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    pub taps: u32,
    /// Faux quand son téléphone est en veille ou déconnecté.
    pub here: bool,
}

/// Ce que montre la télécommande d'un joueur.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remote {
    pub name: String,
    pub taps: u32,
}
