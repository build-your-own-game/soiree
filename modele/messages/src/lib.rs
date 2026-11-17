//! Ce qui passe entre les pages et la machine : ce qu'une page envoie (`Up`),
//! ce qu'elle doit montrer (`View`), ce qu'elle joue une fois (`Effect`).
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

/// Ce qu'une page doit montrer maintenant. Une page qui se recharge retrouve
/// tout ici : elle ne garde rien elle-même.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum View {
    Screen(Screen),
    Remote(Remote),
}

/// Ce que montre l'écran.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screen {
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

/// Ce qu'une page joue une fois, et qu'on peut rater sans rien perdre.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect {
    /// Un joueur vient de passer une dizaine de taps : l'écran le fête.
    Cheer { name: String, taps: u32 },
}
