//! # La brique « télécommande »
//!
//! Elle relie trois choses : le jeu, qui tourne sur l'ordinateur de l'hôte,
//! l'écran (une page ouverte sur cet ordinateur) et les téléphones (une page
//! que chaque joueur ouvre en scannant un QR code).
//!
//! - Sur l'ordinateur : [`pages::build`] prépare les pages, puis
//!   [`host::Room`] les sert et échange des messages avec elles.
//! - Dans le navigateur : [`web::connect`] ouvre le lien vers le jeu.
//!
//! Les messages sont deux types à toi, partagés par le jeu et les pages :
//! ce qu'une page envoie (`Up`) et ce que le jeu répond (`Down`). Une page qui
//! envoie un message que le jeu ne connaît pas ne compile pas.
//!
//! La brique ne sait rien des joueurs, des scores ni des manches. Elle dit
//! qu'une page est arrivée, ce qu'elle a envoyé, et qu'elle est partie.

use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
pub mod host;
#[cfg(not(target_arch = "wasm32"))]
pub mod pages;
#[cfg(target_arch = "wasm32")]
pub mod web;

/// Une page, reconnue d'une connexion à l'autre : un téléphone qui se met en
/// veille puis revient garde le même `Id`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Id(pub u64);

/// Ce qu'est une page pour le jeu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
