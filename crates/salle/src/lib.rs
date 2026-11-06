//! La salle, sans entrées-sorties (PHASE-3, § 2.2) : ce que l'autorité et les appareils
//! se disent, et comment ils le chiffrent.
//!
//! - [`chiffre`] : la poignée de main Noise `NKpsk0` entre un appareil et l'autorité, les
//!   trames de transport et leur fenêtre de rejeu, les cases chiffrées de la garde, les
//!   clés et le fragment du QR (§ 2.4, § 2.6) ;
//! - [`fil`] : les messages chiffrés entre l'autorité et un appareil (§ 2.7).
//!
//! **Le hasard y entre en paramètre**, comme le temps ([`chiffre::Hasard`]) : un
//! générateur donné par la coque. `getrandom` ne sert qu'aux coques de `web/`.
//!
//! Viennent au lot 4b : `Autorite`, `Appareil`, la garde, l'horloge et les messages entre
//! la page et le Worker (`ecran`).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod chiffre;
pub mod fil;
