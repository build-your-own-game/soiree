//! Demandes et réponses (§ 2.10).
//!
//! `Demandes<'d>` et `Demande<'d>`, que le jeu remplit, vivent dans `jeu` ; leur
//! forme sur le fil est [`DemandeFil`].

use serde::{Deserialize, Serialize};

use crate::entrees::IdFichier;
use crate::scene::SceneFil;
use crate::valeurs::TexteCourt;

/// L'identité d'une demande : un identifiant neuf, jamais redonné dans la soirée,
/// même par une annulation du jeu (TL-23). Le jeu l'obtient de `Sorties::id()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct IdDemande(u64);

impl IdDemande {
    /// Réservé au commun : l'identité de ce numéro.
    #[doc(hidden)]
    pub const fn du_fil(n: u64) -> IdDemande {
        IdDemande(n)
    }

    /// Le numéro sur le fil.
    pub const fn fil(self) -> u64 {
        self.0
    }
}

/// Une demande, sur le fil. Elle existe tant que le jeu la liste ; une demande
/// retirée est annulée, et sa réponse jetée avant d'atteindre le jeu (TL-44 à TL-46).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DemandeFil<'a> {
    /// Décoder, recadrer au centre, réduire au carré, encoder (TL-60).
    Image {
        /// Le fichier reçu.
        fichier: IdFichier,
        /// Le côté du carré, en pixels.
        carre: u16,
        /// Le format voulu.
        format: FormatImage,
    },
    /// Dessiner une scène hors écran, sans faire sauter d'image à l'écran (TL-57).
    Rendu {
        /// La scène, sans action.
        scene: SceneFil,
        /// Le facteur d'agrandissement.
        echelle: u8,
        /// Le format voulu.
        format: FormatImage,
    },
    /// Lire un fichier reçu ; sa taille est vérifiée avant la lecture (TL-14).
    Lire {
        /// Le fichier.
        fichier: IdFichier,
        /// La taille la plus grande acceptée, en octets.
        max: u64,
    },
    /// Donner un fichier à télécharger.
    Telecharger {
        /// Son nom.
        nom: &'a str,
        /// Ses octets.
        octets: &'a [u8],
    },
    /// Ajouter une ligne à un flux de résultats de l'ordinateur de la soirée.
    Ligne {
        /// Le flux.
        flux: Flux,
        /// La ligne.
        octets: &'a [u8],
    },
}

/// Le format d'une image produite par le commun.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormatImage {
    /// PNG.
    Png,
    /// WebP si le navigateur sait l'écrire, PNG sinon.
    WebpSinonPng {
        /// La qualité du WebP, de 0 à 100.
        qualite: u8,
    },
}

/// Un flux de résultats de l'ordinateur de la soirée, par son nom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Flux(TexteCourt);

impl Flux {
    /// Le flux de ce nom, à écrire dans une constante.
    pub const fn new(nom: &'static str) -> Flux {
        Flux(TexteCourt::new(nom))
    }

    /// Son nom.
    pub fn nom(&self) -> &str {
        self.0.as_str()
    }
}

/// La réponse à une demande encore listée par le jeu. C'est aussi sa forme sur le fil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reponse<'a> {
    /// L'image produite.
    Image(#[serde(borrow)] Result<&'a [u8], EchecDemande>),
    /// Le rendu produit (ABI 4 : avec les images qu'il n'a pas dessinées).
    Rendu(#[serde(borrow)] Result<ImageRendue<'a>, EchecDemande>),
    /// Les octets lus.
    Lu(#[serde(borrow)] Result<&'a [u8], EchecDemande>),
    /// Le téléchargement est donné, ou la ligne écrite.
    Fait(Result<(), EchecDemande>),
}

/// Ce que rend un `Rendu` réussi (ABI 4) : l'image, et le nombre d'images de la
/// scène que le rendu n'a pas pu dessiner (TL-56, besoin 20 de Tier list). Le jeu
/// l'annonce ; il ne devine pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRendue<'a> {
    /// L'image produite, encodée dans le format demandé.
    pub octets: &'a [u8],
    /// Les images de la scène que le rendu n'a pas dessinées.
    pub non_dessinees: u32,
}

/// La forme sur le fil d'une [`Reponse`] : la même.
pub type ReponseFil<'a> = Reponse<'a>;

/// Pourquoi une demande a échoué. Un échec revient comme une réponse : le jeu
/// l'annonce, et la partie continue (SQ-78).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EchecDemande {
    /// Le fichier dépasse la taille permise.
    TropGros,
    /// Le fichier ne se lit pas comme demandé.
    Illisible,
    /// Le navigateur ou le joueur a refusé.
    Refuse,
    /// La demande a été annulée par le commun.
    Annule,
    /// Le commun a échoué.
    Interne,
}
