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

use std::collections::BTreeSet;

use jeu_abi::{
    Appareil, ComposantVuFil, DemandeFil, EcranFil, ErreurDecodage, ErreurEncodage, FichierRecu,
    FormeEcran, IdDemande, IdGeste, Oui, Pas, ReponseFil, SceneFil, SorteNoeud,
};
use liaison::{AdresseAnnoncee, CodeSalle, Lien, RefusRelais};
use serde::{Deserialize, Serialize};

/// La version des messages entre la page et le Worker. Monte quand la forme d'un type
/// de ce module change ; `tests/ecran.rs` la fige. *2 au lot 10a* : `VersPage::Chargement`,
/// `Jeu`, `Accueil`, `Cle` et `Banc`, `DeLaPage::Demarrer`, et les commandes
/// `ExporterLaCle`, `Figer` et `Retarder`. *3 au lot 11* : la commande `Bots`.
pub const PROTOCOLE: u32 = 3;

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
    /// Le Worker entre dans un appel du chargement (`Bonjour`, `Decrire`), ou en sort :
    /// le chien de garde de la page applique le garde-fou du chargement, puis celui de
    /// la partie (P5, lot 10a).
    Chargement(Oui),
    /// Le jeu que le Worker a chargé : son empreinte livrée, et la forme de son écran.
    Jeu {
        /// Le BLAKE3 du module livré.
        module: [u8; 32],
        /// La forme de l'écran que le jeu déclare.
        forme: FormeEcran,
    },
    /// Le relais a accueilli l'autorité : de quoi écrire le QR (§ 2.6, § 2.8). Le
    /// fragment ne quitte jamais l'ordinateur de la soirée que dans le QR.
    Accueil {
        /// Le code de salle.
        salle: CodeSalle,
        /// Le fragment du QR, sans le `#` (`chiffre::Fragment::ecrire`).
        #[serde(borrow)]
        fragment: &'a str,
        /// Les adresses que le relais annonce.
        adresses: Vec<AdresseAnnoncee>,
    },
    /// La clé de garde G, sur la commande `ExporterLaCle` du diagnostic, jamais d'elle-même
    /// (§ 2.4) : la page la donne à l'hôte dans un fichier.
    Cle([u8; 32]),
    /// Un rapport pour le banc, en JSON, que la page écrit sur sa console ; seulement en
    /// mode banc (§ 8.2).
    Banc(#[serde(borrow)] &'a str),
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
    /// Le premier message au Worker : le mode banc, et le jeu d'une salle neuve (une salle
    /// déjà ouverte reprend le sien, gardé avec G).
    Demarrer {
        /// Le mode banc (§ 8.1) : le relais lancé avec `--banc` le dit à la page.
        banc: Oui,
        /// L'empreinte livrée du module, pour une salle neuve.
        module: Option<[u8; 32]>,
    },
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
    /// Le Worker rend la clé de garde G (`VersPage::Cle`), pour `soiree-banc rejouer-garde`
    /// (§ 2.4).
    ExporterLaCle,
    /// Le banc seulement : la partie ne joue plus, le journal s'écrit en entier à la garde,
    /// puis le Worker dit sa forme canonique (`VersPage::Banc`). Ignorée hors du banc.
    Figer,
    /// Le banc seulement : `fois` réveils de suite, le Worker attend `ms` millisecondes
    /// sans rien faire, comme un fil gelé ; la page doit signaler le retard (§ 8.3,
    /// `arriere-plan`). Ignorée hors du banc.
    Retarder {
        /// L'attente, en millisecondes.
        ms: u32,
        /// Combien de réveils de suite.
        fois: u32,
    },
    /// Le banc seulement : les coups des bots (de l'écran et des téléphones) joués, ou
    /// non. Le scénario `ecran` du banc les arrête : il joue lui-même, par les entrées du
    /// navigateur, et un coup de bot défairait ce qu'il vérifie (lot 11). Ignorée hors du
    /// banc.
    Bots(Oui),
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

/// Les ressources qu'une scène cite, par leur clé sur le fil : l'image de chaque nœud
/// `Image`, et la ressource de chaque `Lien` (lot 11). L'autorité envoie à la page celles
/// qu'elle n'a pas encore : une image reçue en partie (TL-14), une image exportée, n'est
/// pas un asset déclaré.
pub fn images_citees(scene: &SceneFil) -> BTreeSet<u64> {
    scene
        .noeuds
        .iter()
        .filter_map(|n| match &n.sorte {
            SorteNoeud::Image { image, .. } => Some(image.fil()),
            SorteNoeud::Composant {
                composant: ComposantVuFil::Lien { ressource, .. },
                ..
            } => Some(ressource.fil()),
            SorteNoeud::Rect { .. }
            | SorteNoeud::Texte { .. }
            | SorteNoeud::Groupe { .. }
            | SorteNoeud::Bouton { .. }
            | SorteNoeud::CompteARebours { .. }
            | SorteNoeud::Qr { .. }
            | SorteNoeud::Composant { .. }
            | SorteNoeud::Avertissements { .. }
            | SorteNoeud::Surface { .. }
            | SorteNoeud::Definitif => None,
        })
        .collect()
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
