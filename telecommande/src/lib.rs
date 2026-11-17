//! # La brique « télécommande »
//!
//! Le programme de tous les jeux. Elle relie trois choses : la machine du jeu
//! (voir la brique `cartouche`), l'écran (une page ouverte sur l'ordinateur de
//! l'hôte) et les téléphones (une page que chaque joueur ouvre en scannant un
//! QR code).
//!
//! - Sur l'ordinateur : [`console::run`] est tout le programme d'un jeu. Il
//!   prépare les pages, les sert, donne le temps à la machine, lui passe ce
//!   que les pages envoient et leur renvoie ce qu'elles doivent montrer.
//! - Dans le navigateur : [`web::connect`] ouvre le lien vers la console, et
//!   [`web::upload`] lui confie ce qui est lourd (une image, un son).
//!
//! Sur le fil, la console envoie un [`Wire`] ; la page envoie sa commande
//! telle quelle, en JSON. Une page qui envoie une commande que la machine ne
//! connaît pas ne compile pas, si elle emprunte le type de la machine.

use serde::{Deserialize, Serialize};

pub use cartouche::{Content, Kind, Page, Role};

#[cfg(not(target_arch = "wasm32"))]
pub mod console;
#[cfg(not(target_arch = "wasm32"))]
mod doors;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod host;
#[cfg(not(target_arch = "wasm32"))]
pub mod pages;
#[cfg(not(target_arch = "wasm32"))]
mod store;
#[cfg(not(target_arch = "wasm32"))]
mod tls;
#[cfg(target_arch = "wasm32")]
pub mod web;

/// Le code de fermeture d'un lien remplacé : la même page s'est connectée
/// ailleurs (un deuxième onglet). L'ancien lien ne doit pas se rouvrir, sinon
/// les deux se reprendraient la place sans fin.
pub(crate) const REPLACED: u16 = 4000;

/// Ce que la console envoie à une page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Wire<View, Effect> {
    /// À la connexion : ce que la console sait et que la page ne peut pas deviner.
    Welcome(Welcome),
    /// Ce que la page doit montrer maintenant, et le numéro du pas.
    View { tick: u64, view: View },
    /// Ce que la page joue une fois.
    Effect(Effect),
}

/// Ce que la console dit à une page qui arrive.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Welcome {
    /// L'adresse que les téléphones ouvrent (son QR code est servi à `/qr.svg`).
    /// Vide quand le jeu se joue sans télécommande.
    pub join_url: String,
    /// Le rythme du jeu : pour dessiner un compte à rebours à partir du numéro du pas.
    pub ticks_per_second: u32,
}
