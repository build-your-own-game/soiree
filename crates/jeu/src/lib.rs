//! Ce qu'écrit l'auteur d'un jeu (INTERFACE.md, § 2 et § 8.3) : les traits [`Jeu`] et
//! [`Epreuve`], l'étape en deux temps ([`Avant`], [`Apres`], [`Issue`]), les secrets,
//! la scène, les textes, le hasard, la déclaration, et la colle du module
//! (`jeu::colle`, [`module!`]).
//!
//! L'auteur d'un jeu n'utilise que `jeu` : tout s'importe par `use jeu::prelude::*;`.
//! Le vocabulaire du commun sans paramètre de jeu vient de `jeu-abi` (D6) ; ce qui porte
//! un type du jeu, ou un jeton que seul le commun crée ([`Maintenant`], [`Echeance`],
//! [`Autorite`], [`Regard`], [`Secret`], [`Hasard`]), vit ici.
//!
//! # La forme d'un jeu (§ 2.1)
//!
//! - La crate de règles est une bibliothèque `#![no_std]`, `#![forbid(unsafe_code)]`,
//!   avec `extern crate alloc`. `no_std` fait de quatre règles d'écriture des formes :
//!   ni `HashMap`, ni `Instant::now`, ni `SystemTime::now`, ni les fonctions flottantes
//!   de la plateforme ; les mathématiques passent par [`math`] (HIS-50, OUT-144, OUT-145).
//! - Le module est une seconde crate, `<jeu>-module`, de deux lignes :
//!
//! ```text
//! #![cfg_attr(target_arch = "wasm32", no_std)]
//! jeu::module!(devine::Devine);
//! ```
//!
//! # Le code `unsafe`
//!
//! Il n'est pas interdit dans cette crate : l'allocateur à deux modes et l'état du module
//! (`colle::wasm`, compilé pour `wasm32` seulement) en demandent, chaque fois sous
//! `#[expect(unsafe_code, reason = "…")]` (§ 2.14, D34). Le reste est du code sûr.
//!
//! Le constructeur du jeton de l'hôte, `colle::Hote::new`, est `unsafe` sans toucher à
//! la mémoire : c'est un sceau. Les portes de la colle qui exécutent le jeu ou relisent
//! son enveloppe le demandent, et la crate de règles, sans `unsafe`, ne peut pas le
//! construire : une vue ne peut pas se rappeler sous un autre regard, ni relire ses
//! propres secrets par la forme canonique.

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

extern crate alloc;
// Les macros écrivent `::jeu::…` : `jeu` se nomme lui-même pour s'en servir.
extern crate self as jeu;

#[doc(hidden)]
pub mod cache;
#[doc(hidden)]
pub mod colle;
mod collections;
mod declaration;
mod etape;
pub mod fichiers;
mod forme;
mod hasard;
mod page;
mod scene;
mod secret;
mod temps;
mod texte;
mod traits;

#[cfg(test)]
mod tests;

pub use collections::{Appareils, Partage};
pub use declaration::{Applicable, Asset, Declaration, Pause, Raison, ReglesTenu, Telephones};
pub use etape::{
    Apres, Avant, Changement, DeEcran, Entree, Geste, Issue, Parts, Quand, Sorties, Temps, Tenu,
};
pub use forme::{Aucun, Contient, Etat, Forme, Message, octets};
pub use hasard::Hasard;
/// Tous les combien l'épreuve lit l'hypothèse d'une [`Coupure`] (E11) : un jeu s'y
/// accorde à la compilation (`const _: () = assert!(jeu::PERIODE_HYPOTHESE.en_pas() <=
/// 30);`) au lieu de deviner la période de l'épreuve.
pub use jeu_abi::PERIODE_HYPOTHESE;
pub use jeu_derive::{Variantes, etat, message};
pub use page::{ChampPage, Locaux, Page, Recue};
pub use scene::{ComposantVu, CompteARebours, Definitif, Montre, Noeud, Scene};
pub use secret::{Absent, Autorite, Lecteurs, Qui, Regard, Secret};
pub use temps::{Echeance, Maintenant, Reveil};
pub use texte::{CleTexte, Encrable, Encre, Limite, Texte};
pub use traits::{
    Attente, Bot, ComposantBot, Constat, Construction, Coupure, Creation, Demande, Demandes,
    Epreuve, FichierBot, Jeu, Phase, Proprietes, Style, Variantes, Verif,
};

/// Les mathématiques des flottants, identiques en natif et dans le module : `libm`
/// (HIS-50, OUT-144).
pub mod math {
    pub use libm::*;
}

/// Pour les tests natifs du commun seulement : un regard choisi, pour éprouver une
/// projection. Absent du module (`wasm32`) : le jeu qui tourne à la soirée ne peut
/// pas forger un regard. Une copie native qui s'en servirait montrerait autre chose
/// que le module, ce qu'E9 compare.
#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub mod essai {
    /// Le regard de ce fil.
    pub const fn regard(qui: jeu_abi::RegardFil) -> crate::Regard {
        crate::Regard::du_fil(qui)
    }
}

/// Tout ce qu'écrit l'auteur d'un jeu.
pub mod prelude {
    pub use crate::{
        Absent, Appareils, Applicable, Apres, Asset, Attente, Aucun, Autorite, Avant, Bot,
        ChampPage, Changement, CleTexte, ComposantBot, ComposantVu, CompteARebours, Constat,
        Construction, Coupure, Creation, DeEcran, Declaration, Definitif, Demande, Demandes,
        Echeance, Encrable, Encre, Entree, Epreuve, Etat, FichierBot, Geste, Hasard, Issue, Jeu,
        Lecteurs, Limite, Locaux, Maintenant, Message, Montre, Noeud, Page, Partage, Parts, Pause,
        Phase, Proprietes, Quand, Qui, Raison, Recue, Regard, ReglesTenu, Reveil, Scene, Secret,
        Sorties, Style, Telephones, Temps, Tenu, Texte, Variantes, Verif,
    };
    pub use jeu_abi::{
        Absence, Accueil, Alignement, Animation, Appareil, Appui, Axe, Axe2, Borne, Bouton,
        Capacite, Capacites, CauseDepart, CauseTenu, Clavier, ClavierBrut, Cle, CodeTouche,
        Composant, Coupe, Courbe, Cumul, Declencheur, Deplace, Direction8, Directions, Duree,
        Echantillonnage, EchecDemande, EcranTelephone, Effet, Envoi, FichierRecu, FinSaisie, Fini,
        Fixe, Flux, FormatImage, FormeEcran, GenrePointeur, HeureMurale, IdDemande, IdEffet,
        IdFichier, IdGeste, ImageRendue, Instant, Lignes, Limites, Manette, Marges, Mecanique,
        Mods, Orientation, Oui, PAS_PAR_SECONDE, PERIODE_HYPOTHESE, Pas, PhasePointeur,
        PhaseTouche, Places, Pointeur, Presence, Prevision, Rect, Reglages, Repartition, Reponse,
        Rgba, RoleAppareil, Sensation, Signal, Son, SorteAsset, StylePage, StyleTexte, Taille,
        TaillePage, Tenue, TexteCourt, Touche, Unite, Vibration, Visuel, Voulues, ZoneVu,
    };
}

/// Le module WebAssembly d'un jeu : `jeu::module!(devine::Devine);` (§ 2.14).
///
/// Sous `cfg(target_arch = "wasm32")`, il écrit l'allocateur global à deux modes, le
/// gestionnaire de panique (qui écrit le message dans la zone de panique puis exécute
/// `unreachable`), la boîte d'entrée, la zone de panique, l'état du module, les deux
/// exports `soiree_boite` et `soiree_appel`, et la vérification `const` de l'accord de
/// la déclaration et des types (D28). Ailleurs, il n'écrit que cette vérification.
///
/// Le type se nomme par un chemin qui se lit depuis un sous-module : `devine::Devine`,
/// ou `crate::MonJeu`.
#[macro_export]
macro_rules! module {
    ($jeu:ty) => {
        const _: () = $crate::colle::verifier_accord::<$jeu>();
        const _: () = $crate::colle::verifier_invariants::<$jeu>();

        #[cfg(target_arch = "wasm32")]
        #[doc(hidden)]
        mod __module_du_jeu {
            type LeJeu = $jeu;
            const BROUILLON: usize = $crate::colle::taille_brouillon::<LeJeu>();

            #[global_allocator]
            static ALLOCATEUR: $crate::colle::wasm::Allocateur<BROUILLON> =
                $crate::colle::wasm::Allocateur::new();
            static BOITE: $crate::colle::Zone<{ $crate::colle::CAPACITE_BOITE }> =
                $crate::colle::Zone::new();
            static PANIQUE: $crate::colle::Zone<{ $crate::colle::CAPACITE_PANIQUE }> =
                $crate::colle::Zone::new();
            static ETAT: $crate::colle::wasm::Cellule<$crate::colle::wasm::EtatModule<LeJeu>> =
                $crate::colle::wasm::Cellule::new($crate::colle::wasm::EtatModule::new());

            #[panic_handler]
            fn paniquer(info: &::core::panic::PanicInfo<'_>) -> ! {
                $crate::colle::ecrire_panique(&PANIQUE, info);
                ::core::arch::wasm32::unreachable()
            }

            #[expect(unsafe_code, reason = "l'export que le moteur appelle (§ 3.3)")]
            #[unsafe(no_mangle)]
            pub extern "C" fn soiree_boite(taille: i32, sorte: i32) -> i32 {
                // SAFETY : l'export est l'hôte de la colle ; le jeton ne quitte pas cet appel.
                let hote = unsafe { $crate::colle::Hote::new() };
                $crate::colle::wasm::boite(&hote, &ETAT, &ALLOCATEUR, &BOITE, taille, sorte)
            }

            #[expect(unsafe_code, reason = "l'export que le moteur appelle (§ 3.3)")]
            #[unsafe(no_mangle)]
            pub extern "C" fn soiree_appel(taille: i32) -> i64 {
                // SAFETY : l'export est l'hôte de la colle ; le jeton ne quitte pas cet appel.
                let hote = unsafe { $crate::colle::Hote::new() };
                $crate::colle::wasm::appel(&hote, &ETAT, &ALLOCATEUR, &BOITE, &PANIQUE, taille)
            }
        }
    };
}
