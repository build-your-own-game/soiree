//! Le cœur de la partie qui fait foi (INTERFACE.md, § 4 et § 8.6).
//!
//! Le cœur place et ordonne les entrées, tient les tenus, avance la définitive et
//! la tête, fusionne les vues, suit les demandes, isole une faute, tient le journal
//! et reprend. Il ne connaît ni horloge, ni réseau, ni fil, ni WebAssembly, ni aucun
//! type de jeu : il appelle une [`Instance`] abstraite, dont il existe deux sortes,
//! le module dans wasmtime (`moteur-wasmtime`) et la copie native (`jeu-natif`),
//! plus le modèle des tests ([`modele::InstanceModele`]).
//!
//! La crate est `no_std` : rien ici ne peut lire l'heure, ouvrir un fichier ou
//! lancer un fil. C'est la forme qui fait respecter « sans entrées-sorties ».
//!
//! # Ce qui est là
//!
//! - Le contrat entre le cœur et les instances ([`Instance`], [`Fabrique`],
//!   [`Echec`], [`Faute`], [`PanneOutil`]), figé.
//! - Les règles, en fonctions pures ([`regles`]) : placer, garder l'ordre d'un
//!   appareil, ordonner, tenir les tenus, le temps et le rattrapage, les demandes,
//!   la fusion des scènes, ce que l'écran peut envoyer.
//! - La partie qui fait foi ([`Partie`]) : arrivées placées, pas joués, tenus,
//!   sorties, demandes, journal ([`Journal`]), points de reprise
//!   ([`PointDeReprise`]), faute isolée, reprise.
//! - La fenêtre et la tête (§ 4.2, § 4.4, D12) : avec une fenêtre, la définitive
//!   va jusqu'à `cible − fenêtre`, et une seconde instance, la tête, rejouée depuis
//!   un instantané de la définitive, montre l'écran jusqu'à la cible.
//! - La validation de la scène de l'écran ([`regles::valider_scene`], § 6.1).
//! - Après trois `Temps` écartés, le cœur cesse de livrer le temps et le dit
//!   ([`Avancee::en_panne`]) ; [`Partie::repartir`] repart du dernier point sain,
//!   sans les entrées suivantes (§ 4.6).

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

extern crate alloc;

mod arrivee;
mod coeur;
mod instance;
mod journal;
mod lecture;
pub mod modele;
mod partie;
mod pas;
pub mod regles;
mod tete;

pub use arrivee::{
    Arrivee, ComposantArrivee, DemandeArrivee, EcranArrivee, Placement, ReponseArrivee,
};
pub use coeur::EtatCoeur;
pub use instance::{CausePanne, Echec, Fabrique, Faute, Instance, PanneOutil, Responsable};
pub use jeu_abi::SorteAppel;
pub use journal::{
    ArriveePlacee, IssueJournal, Journal, Ligne, LigneEntree, QuoiPlace, ValeurTelephone,
};
pub use partie::{
    AttenteVisible, Avancee, Budgets, Destinataire, EtatVisible, LieuFaute, Ouverture, Partie,
    PointDeReprise, RapportFaute, RefusOuverture, RefusReprise, Tete, Vue,
};
