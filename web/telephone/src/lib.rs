//! La page du téléphone (PHASE-3, § 2.10 ; lot 12).
//!
//! Une page en `http://192.168.x.y:7878/s/<SALLE>#<fragment>`, hors contexte sécurisé (ni
//! WebGPU, ni `crypto.subtle`, ni verrou d'écran : SAL-97, OUT-211). Elle tient :
//!
//! - **l'appareil** (`salle::appareil`) : le lien au relais, Noise `NKpsk0`, les gestes en
//!   file jusqu'à leur accusé durable, le tenu redit à sa période, le module du jeu et les
//!   ressources de la page reçus par la connexion chiffrée (P18) ;
//! - **la page** (`salle::page`) : les locaux du commun, `Page` appelée dans le module, les
//!   relâchements (REM-23, REM-11, P28) ; une seule définition avec les téléphones natifs
//!   du banc ;
//! - **le rendu DOM** (dans le navigateur, `page`) : réconciliation par clé (SAL-165),
//!   `textContent` (REM-56), des champs qui ne se réécrivent jamais sous le doigt, la
//!   croix et les maintenus à capture du pointeur, `visualViewport`, `FontFace` ;
//! - **les écrans du commun** ([`ecrans`]) : un seul message à la fois, qui nomme la vraie
//!   cause (REM-69) ;
//! - **l'adresse** ([`adresse`]) : la salle du chemin, le fragment du QR ;
//! - **le retenu** ([`retenu`]) : sous quelle clé de `localStorage` la page garde ce
//!   qu'elle a retenu, et quand l'écrire ;
//! - **le style** ([`style`]) : le CSS d'un nœud, sans navigateur.
//!
//! Ce qui ne touche pas au navigateur se teste en natif ; le reste, par `soiree-banc`.

#![forbid(unsafe_code)]

pub mod adresse;
pub mod ecrans;
pub mod retenu;
pub mod style;

#[cfg(target_arch = "wasm32")]
mod page;
