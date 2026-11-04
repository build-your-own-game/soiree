//! Ce qui arrive au cœur depuis le transport, et ce qu'il en fait (§ 4.3, § 8.6).
//!
//! Les types de l'enveloppe empruntent leurs octets ; ceux-ci les possèdent,
//! parce qu'une arrivée attend son pas dans le cœur, puis reste au journal.

use alloc::string::String;
use alloc::vec::Vec;

use jeu_abi::{
    Appareil, ComposantFil, DemandeFil, Deplace, Duree, EchecDemande, EcranFil, ErreurFil,
    FichierRecu, FinSaisie, Flux, FormatImage, IdDemande, IdFichier, IdGeste, Instant, Manette,
    Pas, Pointeur, Presence, ReponseFil, SceneFil, Touche,
};
use serde::{Deserialize, Serialize};

/// Une arrivée du transport, déjà dédoublonnée par appareil (§ 8.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arrivee {
    /// Un événement d'un téléphone : compté exactement une fois.
    Geste {
        /// L'appareil.
        de: Appareil,
        /// Le geste, encodé par le jeu.
        octets: Vec<u8>,
        /// L'instant annoncé du geste, sur l'horloge de la partie.
        fait: Instant,
        /// Le pas de la dernière vue que l'appareil dit avoir reçue.
        vu: Pas,
        /// L'identité du geste, stable à travers renvois et rejeux (§ 5.4).
        id: IdGeste,
        /// La demi-latence mesurée par écho depuis l'ordinateur de la soirée.
        demi_aller_retour: Duree,
    },
    /// Un état tenu d'un téléphone : complet, le dernier gagne.
    Tenu {
        /// L'appareil.
        de: Appareil,
        /// La valeur, encodée par le jeu.
        octets: Vec<u8>,
        /// Son numéro, qui ne fait que monter (REM-14).
        numero: u64,
        /// L'instant annoncé.
        fait: Instant,
        /// La demi-latence mesurée.
        demi_aller_retour: Duree,
    },
    /// L'état d'une manette, branchée à l'ordinateur de la soirée.
    Manette {
        /// L'appareil.
        de: Appareil,
        /// Son état, validé au décodage.
        etat: Manette,
        /// Son numéro.
        numero: u64,
        /// L'instant où il a été lu.
        fait: Instant,
    },
    /// Une entrée de l'écran de l'ordinateur de la soirée.
    Ecran {
        /// Ce qui arrive.
        quoi: EcranArrivee,
        /// Le pas de la scène que l'écran montrait.
        vu: Pas,
    },
    /// Un appareil arrive, part, revient ou change.
    Presence(Presence),
    /// La réponse à une demande.
    Reponse {
        /// La demande.
        id: IdDemande,
        /// Sa réponse.
        reponse: ReponseArrivee,
    },
}

/// Une entrée de l'écran, possédée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EcranArrivee {
    /// Une action que la scène portait, encodée par le jeu.
    Action(Vec<u8>),
    /// Une touche brute.
    Touche(Touche),
    /// Un pointeur brut.
    Pointeur(Pointeur),
    /// Le résultat d'un composant.
    Composant(ComposantArrivee),
}

impl EcranArrivee {
    /// Sa forme sur le fil, qui emprunte ses octets.
    pub fn fil(&self) -> EcranFil<'_> {
        match self {
            EcranArrivee::Action(a) => EcranFil::Action(a),
            EcranArrivee::Touche(t) => EcranFil::Touche(*t),
            EcranArrivee::Pointeur(p) => EcranFil::Pointeur(*p),
            EcranArrivee::Composant(c) => EcranFil::Composant(c.fil()),
        }
    }
}

/// Le résultat d'un composant de l'écran, possédé.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComposantArrivee {
    /// Un texte saisi.
    Saisie {
        /// Le champ, par sa clé dans la scène.
        champ: jeu_abi::Cle,
        /// Le texte entier du champ.
        texte: String,
        /// Ce qui termine, ou non, la saisie.
        fin: FinSaisie,
    },
    /// Un objet déplacé.
    Deplace(Deplace),
    /// Des fichiers choisis, déposés ou collés.
    Fichiers {
        /// Le geste qui les a apportés.
        geste: IdGeste,
        /// Les fichiers.
        fichiers: Vec<FichierRecu>,
    },
}

impl ComposantArrivee {
    /// Sa forme sur le fil.
    pub fn fil(&self) -> ComposantFil<'_> {
        match self {
            ComposantArrivee::Saisie { champ, texte, fin } => ComposantFil::Saisie {
                champ: *champ,
                texte,
                fin: *fin,
            },
            ComposantArrivee::Deplace(d) => ComposantFil::Deplace(*d),
            ComposantArrivee::Fichiers { geste, fichiers } => ComposantFil::Fichiers {
                geste: *geste,
                fichiers: fichiers.clone(),
            },
        }
    }
}

/// La réponse à une demande, possédée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReponseArrivee {
    /// L'image produite.
    Image(Result<Vec<u8>, EchecDemande>),
    /// Le rendu produit.
    Rendu(Result<Vec<u8>, EchecDemande>),
    /// Les octets lus.
    Lu(Result<Vec<u8>, EchecDemande>),
    /// Le téléchargement est donné, ou la ligne écrite.
    Fait(Result<(), EchecDemande>),
}

impl ReponseArrivee {
    /// Sa forme sur le fil.
    pub fn fil(&self) -> ReponseFil<'_> {
        match self {
            ReponseArrivee::Image(r) => ReponseFil::Image(r.as_deref().map_err(|e| *e)),
            ReponseArrivee::Rendu(r) => ReponseFil::Rendu(r.as_deref().map_err(|e| *e)),
            ReponseArrivee::Lu(r) => ReponseFil::Lu(r.as_deref().map_err(|e| *e)),
            ReponseArrivee::Fait(r) => ReponseFil::Fait(*r),
        }
    }
}

/// Une demande à lancer, possédée : ce que le commun exécute (§ 2.10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DemandeArrivee {
    /// Décoder, recadrer au centre, réduire au carré, encoder (TL-60).
    Image {
        /// Le fichier reçu.
        fichier: IdFichier,
        /// Le côté du carré, en pixels.
        carre: u16,
        /// Le format voulu.
        format: FormatImage,
    },
    /// Dessiner une scène hors écran (TL-57).
    Rendu {
        /// La scène.
        scene: SceneFil,
        /// Le facteur d'agrandissement.
        echelle: u8,
        /// Le format voulu.
        format: FormatImage,
    },
    /// Lire un fichier reçu, taille vérifiée avant lecture (TL-14).
    Lire {
        /// Le fichier.
        fichier: IdFichier,
        /// La taille la plus grande acceptée.
        max: u64,
    },
    /// Donner un fichier à télécharger.
    Telecharger {
        /// Son nom.
        nom: String,
        /// Ses octets.
        octets: Vec<u8>,
    },
    /// Ajouter une ligne à un flux de résultats.
    Ligne {
        /// Le flux.
        flux: Flux,
        /// La ligne.
        octets: Vec<u8>,
    },
}

impl DemandeArrivee {
    /// La demande possédée de cette forme sur le fil.
    pub fn depuis_fil(d: &DemandeFil<'_>) -> DemandeArrivee {
        match d {
            DemandeFil::Image {
                fichier,
                carre,
                format,
            } => DemandeArrivee::Image {
                fichier: *fichier,
                carre: *carre,
                format: *format,
            },
            DemandeFil::Rendu {
                scene,
                echelle,
                format,
            } => DemandeArrivee::Rendu {
                scene: scene.clone(),
                echelle: *echelle,
                format: *format,
            },
            DemandeFil::Lire { fichier, max } => DemandeArrivee::Lire {
                fichier: *fichier,
                max: *max,
            },
            DemandeFil::Telecharger { nom, octets } => DemandeArrivee::Telecharger {
                nom: String::from(*nom),
                octets: octets.to_vec(),
            },
            DemandeFil::Ligne { flux, octets } => DemandeArrivee::Ligne {
                flux: *flux,
                octets: octets.to_vec(),
            },
        }
    }
}

/// Ce que le cœur a fait d'une arrivée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// Placée à ce pas : elle s'appliquera quand la définitive l'atteindra.
    Place {
        /// Le pas.
        pas: Pas,
    },
    /// Le jeu ne la lit pas, ou elle dépasse `limites.geste` : elle n'entre pas au
    /// journal.
    Malforme(ErreurFil),
    /// Une entrée de l'écran que la dernière scène ne portait pas (SAL-152).
    HorsScene,
    /// Elle ne compte plus : un tenu d'un numéro déjà dépassé (REM-14), ou la
    /// réponse à une demande qui n'est plus en cours (TL-45).
    Perimee,
    /// L'appareil a déjà `limites.file` événements en attente.
    FilePleine,
    /// Le jeu a fauté en la validant : elle est écartée, et la faute est rapportée
    /// dans la prochaine [`crate::Avancee`].
    Ecartee,
}
