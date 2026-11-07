//! Les types des entrées qui n'ont pas de paramètre de jeu (§ 2.6).
//!
//! `Entree`, `Geste`, `Changement` et `DeEcran`, qui portent les types du jeu,
//! vivent dans `jeu` ; leur forme sur le fil est [`crate::EntreeFil`].

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::scene::Cle;
use crate::valeurs::{CodeTouche, Fixe, Oui, TexteCourt};

/// L'identité d'un geste, stable à travers renvois, rejeux et prédiction (§ 5.4).
/// Ce qu'un geste crée se nomme depuis elle, jamais depuis un compteur de l'état.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct IdGeste(u64);

impl IdGeste {
    /// Réservé au commun : l'identité de ce numéro.
    #[doc(hidden)]
    pub const fn du_fil(n: u64) -> IdGeste {
        IdGeste(n)
    }

    /// Le numéro sur le fil.
    pub const fn fil(self) -> u64 {
        self.0
    }
}

/// Un fichier reçu par le commun, dont les octets restent chez lui jusqu'à une
/// demande (TL-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct IdFichier(u64);

impl IdFichier {
    /// Réservé au commun : le fichier de ce numéro.
    #[doc(hidden)]
    pub const fn du_fil(n: u64) -> IdFichier {
        IdFichier(n)
    }

    /// Le numéro sur le fil.
    pub const fn fil(self) -> u64 {
        self.0
    }
}

/// Pourquoi un état tenu change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CauseTenu {
    /// Le joueur l'a changé.
    Geste,
    /// Le commun le relâche : plus de nouvelles depuis `fraicheur` (SQ-01).
    Silence,
    /// Le commun le relâche : l'appareil est parti.
    Depart,
    /// Le commun le relâche : la page a quitté le premier plan (REM-10).
    ArrierePlan,
    /// Le commun le relâche : une pause du commun commence.
    Pause,
}

/// Une touche brute du clavier de l'ordinateur de la soirée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Touche {
    /// La touche physique.
    pub code: CodeTouche,
    /// Les modificateurs tenus.
    pub mods: Mods,
    /// Enfoncée ou relâchée.
    pub phase: PhaseTouche,
    /// Une répétition automatique du système.
    pub repetition: Oui,
}

/// Les modificateurs du clavier.
///
/// Dans un raccourci, [`Mods::CMD_OU_CTRL`] (les deux à `Oui`) veut dire « Cmd sur
/// Mac, Ctrl ailleurs » : le commun le traduit pour la machine de la soirée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mods {
    /// Maj.
    pub maj: Oui,
    /// Ctrl.
    pub ctrl: Oui,
    /// Alt, ou Option.
    pub alt: Oui,
    /// Cmd, ou la touche Windows.
    pub meta: Oui,
}

impl Mods {
    /// Aucun modificateur.
    pub const AUCUN: Mods = Mods {
        maj: Oui::Non,
        ctrl: Oui::Non,
        alt: Oui::Non,
        meta: Oui::Non,
    };
    /// Maj seule.
    pub const MAJ: Mods = Mods {
        maj: Oui::Oui,
        ctrl: Oui::Non,
        alt: Oui::Non,
        meta: Oui::Non,
    };
    /// Dans un raccourci : Cmd sur Mac, Ctrl ailleurs.
    pub const CMD_OU_CTRL: Mods = Mods {
        maj: Oui::Non,
        ctrl: Oui::Oui,
        alt: Oui::Non,
        meta: Oui::Oui,
    };
}

/// Enfoncée ou relâchée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhaseTouche {
    /// Enfoncée.
    Enfoncee,
    /// Relâchée.
    Relachee,
}

/// Un pointeur brut, sur une surface que la scène a déclarée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pointeur {
    /// L'identité du pointeur, pour suivre plusieurs doigts.
    pub id: u32,
    /// Souris, doigt ou stylet : un seul chemin pour les trois (TL-36).
    pub genre: GenrePointeur,
    /// Le bouton, pour une souris.
    pub bouton: u8,
    /// Ce qui arrive.
    pub phase: PhasePointeur,
    /// La position, dans le repère de la scène.
    pub pos: (Fixe, Fixe),
}

/// Ce qui pointe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GenrePointeur {
    /// Une souris.
    Souris,
    /// Un doigt.
    Doigt,
    /// Un stylet.
    Stylet,
}

/// Ce qui arrive à un pointeur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhasePointeur {
    /// Il appuie.
    Appui,
    /// Il bouge.
    Deplacement,
    /// Il lâche.
    Relache,
    /// Le système l'annule (pointercancel).
    Annule,
}

/// Le résultat d'un composant de la scène : texte saisi, objet déposé, fichiers
/// reçus. C'est aussi sa forme sur le fil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Composant<'a> {
    /// Un texte saisi. Jamais pendant une composition IME (TL-32, PUN-70).
    Saisie {
        /// Le champ, par sa clé dans la scène.
        champ: Cle,
        /// Le texte entier du champ.
        texte: &'a str,
        /// Ce qui termine, ou non, la saisie.
        fin: FinSaisie,
    },
    /// Un objet déplacé.
    Deplace(Deplace),
    /// Des fichiers choisis, déposés ou collés.
    Fichiers {
        /// Le geste qui les a apportés.
        geste: IdGeste,
        /// ABI 4 : le sélecteur d'où ils viennent, par sa clé dans la scène ; `None` :
        /// un dépôt ou un collage.
        selecteur: Option<Cle>,
        /// Les fichiers.
        fichiers: Vec<FichierRecu>,
    },
}

/// La forme sur le fil d'un [`Composant`] : la même.
pub type ComposantFil<'a> = Composant<'a>;

/// Ce qui termine, ou non, une saisie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinSaisie {
    /// La saisie continue.
    EnCours,
    /// Le joueur a validé.
    Entree,
    /// Le champ a perdu le focus.
    PerteDeFocus,
}

/// Un objet déplacé, en termes de l'état du jeu (TL-36 à TL-47).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Deplace {
    /// La place visée, comptée dans l'ordre de la zone, sans l'objet tenu (TL-39).
    Survol {
        /// L'objet tenu.
        objet: Cle,
        /// La zone survolée.
        zone: Cle,
        /// La place visée dans la zone.
        indice: u32,
    },
    /// L'objet est déposé.
    Depose {
        /// L'objet déposé.
        objet: Cle,
        /// La zone.
        zone: Cle,
        /// La place dans la zone.
        indice: u32,
    },
    /// Le déplacement est annulé : Échap, pointercancel.
    Annule {
        /// L'objet qui était tenu.
        objet: Cle,
    },
}

/// Un fichier choisi, déposé ou collé. Ses octets restent chez le commun jusqu'à
/// une demande (TL-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FichierRecu {
    /// Le fichier.
    pub id: IdFichier,
    /// Son nom.
    pub nom: TexteCourt,
    /// Son type, tel que le navigateur le donne.
    pub genre: TexteCourt,
    /// Sa taille, en octets.
    pub taille: u64,
}
