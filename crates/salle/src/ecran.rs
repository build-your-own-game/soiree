//! Les messages entre la page de l'écran et le Worker de l'autorité (PHASE-3, § 2.7) :
//! une définition Rust unique pour les deux côtés (CONTRAT, « Les types de bout en
//! bout »).
//!
//! Ils passent par `postMessage`, encodés en postcard dans un `ArrayBuffer` transféré,
//! sans copie ; ils empruntent leurs octets au décodage, comme [`crate::fil`]. Décodés
//! strictement par [`jeu_abi::decoder`] ; chaque variant a ses octets figés et sa seconde
//! lecture dans `tests/ecran.rs` (SAL-90). Changer la forme sans monter [`PROTOCOLE`] les
//! fait rougir ; un variant s'ajoute à la fin de son enum.
//!
//! Les instants sont dans la base `performance.timeOrigin + performance.now()`, en
//! microsecondes, celle de chaque contexte : que la page et le Worker la partagent à la
//! granularité près est à constater (M14).

use jeu_abi::{
    Appareil, DemandeFil, EcranFil, ErreurDecodage, ErreurEncodage, FichierRecu, IdDemande,
    IdGeste, Oui, Pas, ReponseFil,
};
use liaison::{CodeSalle, Lien, RefusRelais};
use serde::{Deserialize, Serialize};

/// La version des messages entre la page et le Worker. Monte quand la forme d'un type
/// de ce module change ; `tests/ecran.rs` la fige.
pub const PROTOCOLE: u32 = 1;

/// Ce que le Worker envoie à la page de l'écran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersPage<'a> {
    /// La scène de l'écran, une `SceneFil` encodée, à dessiner.
    Scene {
        /// Le pas dont elle est la vue.
        pas: Pas,
        /// L'instant où le Worker l'a produite.
        t_us: u64,
        /// La scène encodée.
        #[serde(borrow)]
        octets: &'a [u8],
    },
    /// Les octets d'une police ou d'une image du jeu.
    Ressource {
        /// La clé de l'asset.
        cle: u64,
        /// Ses octets.
        #[serde(borrow)]
        octets: &'a [u8],
    },
    /// Une demande du jeu, que la page exécute (§ 2.9).
    Demande {
        /// Son identité.
        id: IdDemande,
        /// La demande, telle que le module l'a écrite.
        #[serde(borrow)]
        quoi: DemandeFil<'a>,
    },
    /// L'état de la salle, pour l'écran du commun et le diagnostic.
    EtatSalle(EtatSalle),
    /// Le Worker a tourné : le chien de garde de la page s'en sert (P5, REM-02).
    Battement {
        /// Le dernier pas définitif.
        pas: Pas,
        /// Le retard de ce réveil sur l'image qui l'a demandé, en millisecondes.
        retard_ms: u32,
    },
    /// Le commun est en panne, ou la salle s'arrête : l'écran le dit.
    Panne(#[serde(borrow)] RapportPanne<'a>),
    /// Les traces de l'entrée au pixel (§ 8.4).
    Traces(Vec<Trace>),
    /// Posté avant chaque appel `Entree` : le geste en cours, pour l'écarter s'il bloque
    /// deux fois (P39).
    EnCours(Option<IdGeste>),
}

/// Ce que la page de l'écran envoie au Worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeLaPage<'a> {
    /// Une entrée de l'écran : clavier, pointeur, action, composant.
    Ecran {
        /// L'entrée.
        #[serde(borrow)]
        quoi: EcranFil<'a>,
        /// Le pas de la scène que l'écran montrait.
        vu: Pas,
        /// L'instant de l'événement.
        t_us: u64,
    },
    /// Le signal d'une image (`requestAnimationFrame`) : le Worker joue le pas que
    /// l'horloge a atteint (P6).
    Image {
        /// L'instant de l'image.
        t_us: u64,
        /// Sa période, en microsecondes.
        periode_us: u32,
    },
    /// Des fichiers reçus par la page ; leurs octets y restent (TL-14).
    Fichiers(Vec<FichierRecu>),
    /// La réponse à une demande.
    Reponse {
        /// La demande.
        id: IdDemande,
        /// Sa réponse.
        #[serde(borrow)]
        quoi: ReponseFil<'a>,
    },
    /// Une commande du commun.
    Commande(CommandeEcran),
}

/// Une commande de l'écran du commun, jamais du jeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandeEcran {
    /// Jouer un autre module, par son empreinte livrée : une nouvelle partie, mêmes liens,
    /// même registre (§ 2.8).
    ChangerDeJeu {
        /// L'empreinte du module.
        module: [u8; 32],
    },
    /// Une nouvelle soirée : secret d'entrée et clés neufs (§ 2.6).
    NouvelleSoiree,
    /// Reprendre après une panne, en écartant au besoin l'entrée qui a bloqué (P39).
    Reprendre {
        /// L'entrée à écarter.
        ecarter: Option<IdGeste>,
    },
    /// F8 : une marque au journal (SAL-162).
    Marque,
}

/// L'état de la salle, tel que l'écran le montre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EtatSalle {
    /// Le code de salle.
    pub salle: CodeSalle,
    /// Le lien de l'autorité au relais, s'il est ouvert.
    pub lien: Option<Lien>,
    /// Les appareils de la salle.
    pub appareils: Vec<AppareilVu>,
    /// Le dernier numéro de case que le relais dit écrit.
    pub garde: Option<u64>,
}

/// Un appareil, tel que l'écran le montre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppareilVu {
    /// L'appareil.
    pub appareil: Appareil,
    /// Est-il présent ?
    pub present: Oui,
    /// Son lien, s'il en a un.
    pub lien: Option<Lien>,
}

/// Ce qui est en panne.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanneSalle {
    /// Le commun : la partie ne s'ouvre pas, ne reprend pas, ou l'outil a lâché.
    Outil,
    /// La garde ne se relit pas, ou ne s'écrit pas.
    Garde,
    /// Le relais a refusé l'autorité ou sa garde.
    Relais(RefusRelais),
}

/// Le rapport d'une panne, pour l'écran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RapportPanne<'a> {
    /// Ce qui est en panne.
    pub quoi: PanneSalle,
    /// Le détail, pour le diagnostic.
    pub detail: &'a str,
}

/// D'où vient une trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdTrace {
    /// Une entrée de l'écran, par son numéro.
    Ecran(u64),
    /// Un geste d'un appareil, par son numéro dans son onglet.
    Appareil {
        /// L'appareil.
        appareil: Appareil,
        /// Le numéro du geste.
        numero: u64,
    },
}

/// Une entrée suivie jusqu'au pixel (§ 8.4), en microsecondes de l'horloge de
/// l'autorité, l'émetteur ramené par l'écart estimé.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trace {
    /// L'entrée.
    pub id: IdTrace,
    /// L'événement.
    pub evenement_us: u64,
    /// Son envoi.
    pub envoi_us: u64,
    /// Sa réception et son renvoi par le relais, en clair, horloge du relais.
    pub relais: Option<(u64, u64)>,
    /// Sa réception par l'autorité.
    pub recu_us: u64,
    /// Le pas où elle s'applique.
    pub place: Pas,
    /// Le début du pas joué.
    pub pas_debut_us: u64,
    /// Sa fin.
    pub pas_fin_us: u64,
    /// La fin des vues.
    pub vue_fin_us: u64,
    /// La scène reçue par la page.
    pub page_recue_us: u64,
    /// Le dessin soumis au GPU.
    pub soumis_us: u64,
    /// Le travail du GPU fini.
    pub gpu_fini_us: u64,
    /// L'incertitude : un demi-aller-retour pour un téléphone, plus M14.
    pub incertitude_us: u32,
}

impl<'a> VersPage<'a> {
    /// Encode le message.
    pub fn encoder(&self) -> Result<Vec<u8>, ErreurEncodage> {
        let mut v = Vec::new();
        jeu_abi::encoder(self, &mut v)?;
        Ok(v)
    }

    /// Décode un message qui occupe exactement `octets` (SAL-118).
    pub fn decoder(octets: &'a [u8]) -> Result<VersPage<'a>, ErreurDecodage> {
        jeu_abi::decoder(octets)
    }
}

impl<'a> DeLaPage<'a> {
    /// Encode le message.
    pub fn encoder(&self) -> Result<Vec<u8>, ErreurEncodage> {
        let mut v = Vec::new();
        jeu_abi::encoder(self, &mut v)?;
        Ok(v)
    }

    /// Décode un message qui occupe exactement `octets` (SAL-118).
    pub fn decoder(octets: &'a [u8]) -> Result<DeLaPage<'a>, ErreurDecodage> {
        jeu_abi::decoder(octets)
    }
}
