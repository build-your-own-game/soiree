//! Le protocole d'un appel, une seule fois pour tous les moteurs (INTERFACE.md, § 3.4 et
//! § 4.6 ; PHASE-3, § 2.3, P2 et P3).
//!
//! Un moteur WebAssembly ne fournit que des primitives sur une instance du module
//! réécrit ([`Machine`]) : lire et écrire la mémoire, la remettre à zéro, la faire
//! grandir, lire et poser une globale, appeler `soiree_boite` et `soiree_appel`, et
//! rendre un piège brut ([`Piege`]). Tout le reste est ici, défini une fois, pour
//! wasmtime (`moteur-wasmtime`) et pour le navigateur (`moteur-navigateur`, lot 3a) :
//!
//! - le déroulé d'un appel (§ 3.4) et le classement d'un piège en faute ou en panne
//!   ([`Protocole`], qui est une [`partie::Instance`]) ;
//! - l'instantané **hors des quatre zones** ([`Instantane`], P3), sa restauration qui
//!   les remet à zéro, et sa forme dans la garde ([`partie::persistance::InstantaneGarde`]) ;
//! - l'empreinte de l'état (§ 3.2) ;
//! - ce que le chargement vérifie : `Bonjour`, les zones, `Decrire`, l'accord de la
//!   déclaration ([`verifier_chargement`]), sous [`BUDGET_DE_CHARGEMENT`] ;
//! - le relevé de chaque appel d'une instance, et la reprise des journaux de l'épreuve
//!   qui le produit, que E20 compare d'un moteur à l'autre ([`releve`], lot 3b) ;
//! - la trace d'étalonnage, que la capture du harnais écrit dans wasmtime et que
//!   `soiree-banc etalonner` rejoue dans les navigateurs ([`trace`], lot 3c) ; la
//!   profondeur atteinte par un module de la variante de mesure de la réécriture
//!   ([`profondeur_atteinte`], M9, lot 3c).
//!
//! Deux morceaux qui s'accordent n'ont qu'une définition : le harnais de mesure des
//! navigateurs avait recopié ce protocole en JavaScript (`mesures/navigateur/page/
//! banc-coeur.js`) ; le lot 3c l'a retiré : `soiree-banc etalonner` rejoue la trace du
//! harnais sur ce protocole-ci.
//!
//! # Une faute, une panne
//!
//! Après un piège, dans cet ordre (le premier constat qui tient décide ;
//! `classer_garde_son_ordre` fait rougir ce fichier si l'ordre change) :
//!
//! | Constat après un piège | Issue |
//! |---|---|
//! | le moteur a échoué hors du module ([`Piege::Moteur`]) | panne de l'outil (`Interne`) |
//! | le garde-fou mural a interrompu l'appel ([`Piege::Interrompu`]) | panne de l'outil (`GardeFouMural`) |
//! | `soiree_croissance_refusee` posé | panne de la machine (`CroissanceRefusee`) |
//! | `soiree_carburant` négatif | [`partie::Faute::Budget`] |
//! | `soiree_profondeur` au-delà du plafond | [`partie::Faute::Profondeur`] |
//! | `soiree_croissance_bornee` posé | [`partie::Faute::Memoire`] |
//! | un message dans la zone de panique | [`partie::Faute::Panique`] |
//! | accès hors de la mémoire ([`Piege::HorsMemoire`]) | [`partie::Faute::HorsMemoire`] |
//! | pile native épuisée ([`Piege::PileNative`]) | panne de l'outil : la profondeur devait piquer avant |
//! | un autre piège ([`Piege::Autre`]) | [`partie::Faute::Piege`], nommé par le moteur |
//!
//! Après un appel réussi : la pile non rendue est une panne de l'outil
//! (`PileNonRendue`), une sortie hors du brouillon une [`partie::Faute::SortieInvalide`].
//!
//! # Les quatre zones
//!
//! La pile, la boîte, le brouillon et la zone de panique sont remises à zéro en entier
//! avant chaque appel : elles ne portent rien d'un appel au suivant. L'empreinte les
//! exclut, l'instantané aussi ; la restauration les remet à zéro. La zone de panique a
//! la forme de `jeu-abi` ([`jeu_abi::lire_panique`]), que le gestionnaire de panique de
//! `jeu::module!` écrit.
//!
//! # `no_std`
//!
//! Comme `partie`, la crate ne lit ni l'heure, ni un fichier, ni un fil : le temps et le
//! garde-fou mural sont au moteur.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

extern crate alloc;

mod chargement;
pub mod essais;
mod instantane;
mod machine;
mod protocole;
pub mod releve;
pub mod trace;

pub use chargement::{
    BUDGET_DE_CHARGEMENT, Charge, RefusModule, globales_d_origine, verifier_chargement,
};
pub use instantane::Instantane;
pub use machine::{Bits, Machine, Piege};
pub use protocole::{
    Carte, Plan, Protocole, Zone, Zones, oublier_la_profondeur_atteinte, profondeur_atteinte,
};

use alloc::string::String;
use partie::{CausePanne, PanneOutil};

/// La taille la plus grande d'un module livré, avant réécriture : 4 Mio, **choisi**
/// (PHASE-3, § 2.8 ; les modules des jeux font de 227 à 780 ko, mesure 4 de la phase
/// 1). Sa seule définition : le Worker de l'écran refuse au chargement un module plus
/// gros (lot 10a), et l'épreuve le fait échouer à E1, pour qu'un module éprouvé ne soit
/// jamais refusé le soir même (lot 6). M4, au lot 12, mesure ce qu'un téléphone en
/// télécharge et en compile.
pub const TAILLE_MODULE_MAX: u64 = 4 * 1024 * 1024;

fn interne(quoi: impl Into<String>) -> PanneOutil {
    PanneOutil::new(CausePanne::Interne, quoi)
}
