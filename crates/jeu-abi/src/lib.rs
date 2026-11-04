//! Ce que le commun et le module d'un jeu échangent (INTERFACE.md, § 3 et § 8.1).
//!
//! Cette crate est la définition unique de l'enveloppe ([`Appel`], [`Retour`],
//! [`EntreeFil`]) et du vocabulaire du commun : le temps, les appareils et leurs
//! capacités, les entrées de l'écran, la manette, les effets, la scène, les
//! demandes et leurs réponses, la déclaration en forme possédée. Le moteur, le
//! cœur de la partie et la colle du module en dépendent ; elle ne connaît aucun
//! type de jeu (D6). Les types d'un jeu y voyagent en octets.
//!
//! # Ce qu'elle garantit
//!
//! - **Une seule définition.** Le moteur encode un [`Appel`], le module le décode
//!   avec le même type ; le module encode un [`Retour`], le moteur le décode.
//! - **Une valeur venue de l'extérieur est validée à son décodage** (SAL-117) :
//!   [`Fini`], [`Axe`], [`Axe2`], [`Borne`], [`TexteCourt`], [`CodeTouche`],
//!   [`Places`] et la structure d'une [`SceneFil`] ne se décodent pas invalides.
//! - **Le décodage est strict** ([`decoder`]) : un octet restant est un refus
//!   (REM-59, OUT-146, SAL-118), et une longueur plus grande que ce qui reste
//!   échoue sans réserver de mémoire pour elle (REM-58).
//! - **Aucun type de l'enveloppe n'est récursif.** La scène est une liste à plat
//!   dont chaque nœud nomme son parent : décoder ce qu'un module rend ne peut pas
//!   faire déborder la pile du moteur, quelle que soit la profondeur de la scène.
//!
//! # Le format
//!
//! postcard 1.x (§ 3.7) : entiers non signés en varint LEB128, entiers signés en
//! zigzag puis varint, `u8` et `i8` sur un octet brut, flottants en petit-boutiste,
//! longueurs préfixées en varint, variants numérotés dans l'ordre de déclaration
//! (en varint), structures et tuples à la suite sans étiquette, `Option` par un
//! octet 0 ou 1. postcard lit aussi un varint non minimal (`80 00` pour 0) :
//! deux suites d'octets peuvent donner la même valeur. Le journal garde les octets
//! reçus, et le rejeu ne dépend pas de cette liberté. [`encoder`] écrit toujours
//! la forme minimale.
//!
//! # Les constructeurs du commun
//!
//! Quelques identités ne se créent depuis un nombre que par le commun : [`Appareil`],
//! [`Pas`], [`IdGeste`], [`IdFichier`], [`IdDemande`], [`HeureMurale`] et [`Cle`]. Leurs constructeurs `du_fil` sont publics, parce que le moteur et le
//! cœur en ont besoin, et cachés de la documentation, parce qu'un jeu n'a pas à
//! s'en servir. C'est le même niveau de fermeture que `jeu::colle` (§ 8.3).
//!
//! # `no_std`
//!
//! La crate se compile sans `std`, avec `alloc`, pour `wasm32-unknown-unknown`.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

extern crate alloc;

mod appareils;
mod declaration;
mod demandes;
mod entrees;
mod enveloppe;
mod format;
mod scene;
mod temps;
mod valeurs;
mod zones;

pub use appareils::{
    Absence, Appareil, Capacite, Capacites, CauseDepart, Orientation, Presence, RoleAppareil,
};
pub use declaration::{
    Accueil, ApplicableFil, AssetFil, DeclarationFil, FormeEcran, Limites, Mecanique, PauseFil,
    Places, Prevision, ReglesTenuFil, Repartition, SorteAsset, TelephonesFil, Voulues,
    raison_suffisante,
};
pub use demandes::{DemandeFil, EchecDemande, Flux, FormatImage, IdDemande, Reponse, ReponseFil};
pub use entrees::{
    CauseTenu, Composant, ComposantFil, Deplace, FichierRecu, FinSaisie, GenrePointeur, IdFichier,
    IdGeste, Mods, PhasePointeur, PhaseTouche, Pointeur, Touche,
};
pub use enveloppe::{
    ABI, Appel, AttenteFil, Bonjour, ComposantBotFil, ConstatFil, CoupFil, EcranFil, EntreeFil,
    EtatFil, FichierBotFil, IssueFil, MontreFil, OuiFil, ProprietesFil, QuandFil, RegardFil,
    Retour, ReveilFil, SorteAppel, SorteOctets, SortieFil, StyleFil, TenuFil, Validation, VueFil,
};
pub use format::{
    ErreurDecodage, ErreurEncodage, ErreurFil, Fnv1a, Illisible, decoder, encoder, fnv1a64,
    fnv1a64_suite,
};
pub use scene::{
    Alignement, Animation, ClavierBrut, Cle, ComposantVuFil, CompteAReboursFil, Coupe, Courbe,
    Echantillonnage, Effet, IdEffet, Lignes, NoeudFil, RaccourciFil, Reglages, SceneFil, Sensation,
    Signal, Son, SorteNoeud, StyleTexte, Taille, Tenue, Unite, Vibration, Visuel, ZoneVu,
};
pub use temps::{Duree, HeureMurale, Instant, PAS_PAR_SECONDE, Pas};
pub use valeurs::{
    Appui, Axe, Axe2, Borne, Bouton, CodeTouche, Cumul, Fini, Fixe, HorsBornes, Manette, Oui, Rect,
    Rgba, TexteCourt,
};
pub use zones::{
    Canonique, PANIQUE_CODE, PANIQUE_ENTETE, entete_de_panique, lire_canonique, lire_panique,
};
