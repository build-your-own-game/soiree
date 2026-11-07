//! La salle, sans entrées-sorties (PHASE-3, § 2.2) : ce que l'autorité et les appareils
//! se disent, et comment ils le chiffrent.
//!
//! - [`chiffre`] : la poignée de main Noise `NKpsk0` entre un appareil et l'autorité, les
//!   trames de transport et leur fenêtre de rejeu, les cases chiffrées de la garde, les
//!   clés et le fragment du QR (§ 2.4, § 2.6) ;
//! - [`fil`] : les messages chiffrés entre l'autorité et un appareil (§ 2.7) ;
//! - [`ecran`] : les messages entre la page de l'écran et le Worker (§ 2.7) ;
//! - [`horloge`] : le pas que l'autorité joue, et l'heure de l'autorité qu'un appareil
//!   estime par écho (SAL-127 à SAL-129) ;
//! - [`garde`] : la chaîne des cases que l'autorité écrit au relais, et sa relecture
//!   (§ 2.4) ;
//! - [`autorite`] : l'autorité, la partie qui fait foi et ses appareils (§ 2.3 à § 2.7) ;
//! - [`appareil`] : un appareil, le cœur du téléphone sans sa page (§ 2.10) ;
//! - [`en_cours`] : le geste en cours, que l'autorité dit à la page avant chaque `Entree`
//!   (P39, lot 10a) ;
//! - [`chien`] : le chien de garde de la page de l'écran, ses deux garde-fous muraux,
//!   l'entrée qui bloque deux fois, les relances, le retard des réveils (P5, P39, lot 10a) ;
//! - [`simulation`] : un relais en mémoire et un réseau simulé déterministe, à horloge
//!   qui saute d'échéance en échéance et graine fixe (SAL-09), qui font tourner la vraie
//!   autorité et de vrais appareils (SAL-10 ; PHASE-3, § 8.1, lot 4b).
//!
//! **Le hasard y entre en paramètre**, comme le temps ([`chiffre::Hasard`]) : un
//! générateur donné par la coque ; celle du Worker (`web/autorite`, lot 10a) le tire de
//! `crypto.getRandomValues`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod appareil;
pub mod autorite;
pub mod chien;
pub mod chiffre;
pub mod ecran;
pub mod en_cours;
pub mod fil;
pub mod garde;
pub mod horloge;
pub mod simulation;
