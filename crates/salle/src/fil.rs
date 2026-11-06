//! Les messages chiffrés entre l'autorité et un appareil (PHASE-3, § 2.7) : une
//! définition Rust unique pour les deux côtés (CONTRAT, « Les types de bout en bout »).
//!
//! Ils voyagent dans les trames d'une [`crate::chiffre::Session`], que le relais fait
//! passer sans les lire. postcard, décodé strictement par [`jeu_abi::decoder`] ; chaque
//! variant a ses octets figés et sa seconde lecture dans `tests/fil.rs` (SAL-90).
//! Changer la forme sans monter [`PROTOCOLE`] les fait rougir ; un variant s'ajoute à la
//! fin de son enum.
//!
//! **`VersAutorite::Bonjour` reste le premier variant, et `protocole` son premier
//! champ** : une page d'un autre protocole se reconnaît même quand son bonjour ne se lit
//! plus ([`protocole_d_un_bonjour`], SAL-40). **Il est le premier message de transport**
//! de l'appareil, jamais une charge de la poignée de main (PHASE-3, § 2.6, SAL-17).

use jeu_abi::{Capacites, ErreurDecodage, ErreurEncodage, Instant, Pas};
use serde::{Deserialize, Serialize};

/// La version des messages chiffrés. Monte quand la forme d'un type de ce module
/// change ; `tests/fil.rs` la fige.
pub const PROTOCOLE: u32 = 1;

/// Ce qu'un appareil envoie à l'autorité.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersAutorite<'a> {
    /// Le premier message de transport. **Toujours le premier variant, `protocole` son
    /// premier champ** (SAL-40).
    Bonjour {
        /// [`PROTOCOLE`] de l'appareil.
        protocole: u32,
        /// Le jeton de l'appareil, tiré par lui et gardé dans son `localStorage` : son
        /// identité (SAL-14). Il ne voyage que chiffré, après la poignée de main.
        jeton: [u8; 32],
        /// L'onglet : deux onglets d'un même appareil ne partagent pas leurs numéros
        /// (SAL-36).
        onglet: u64,
        /// Les capacités de l'appareil, dites honnêtement (SAL-97, OUT-211).
        capacites: Capacites,
        /// L'empreinte du module que l'appareil a déjà, s'il en a un.
        module_connu: Option<[u8; 32]>,
    },
    /// Un geste, en file jusqu'à son accusé durable (PHASE-3, § 2.4).
    Geste {
        /// L'onglet qui l'a fait.
        onglet: u64,
        /// Son numéro dans l'onglet, à partir de 1.
        numero: u64,
        /// Le geste, encodé par le module.
        #[serde(borrow)]
        octets: &'a [u8],
        /// Quand il a été fait, dans le temps de l'autorité tel que l'appareil l'estime.
        fait: Instant,
        /// Le pas de la vue sur laquelle il a été fait (SAL-115).
        vu: Pas,
        /// Ce que l'appareil a mesuré du geste, pour la latence (§ 8.4).
        trace: TraceTelephone,
    },
    /// Un état tenu : le dernier gagne (REM-14).
    Tenu {
        /// Son numéro, qui croît à chaque changement et à chaque redite.
        numero: u64,
        /// L'état, encodé par le module.
        #[serde(borrow)]
        octets: &'a [u8],
        /// Quand il a été fait, dans le temps de l'autorité tel que l'appareil l'estime.
        fait: Instant,
    },
    /// Un écho, pour l'horloge (SAL-127, SAL-128).
    Echo {
        /// L'instant de l'envoi, horloge de l'émetteur, en microsecondes.
        t_us: u64,
        /// L'instant que l'écho rend, s'il répond à un écho de l'autre côté ; 0 sinon.
        echo_us: u64,
    },
    /// Un morceau du module reçu : la fenêtre d'envoi du module (PHASE-3, § 2.5).
    ModuleRecu {
        /// L'empreinte du module.
        empreinte: [u8; 32],
        /// Le numéro du morceau.
        morceau: u32,
    },
}

/// Ce que l'autorité envoie à un appareil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersAppareil<'a> {
    /// L'accueil, après un [`VersAutorite::Bonjour`] accepté (SAL-36, SAL-38).
    Accueil {
        /// Le numéro de la session, neuf à chaque accueil.
        session: u32,
        /// Le dernier geste de cet onglet que l'autorité a placé : l'appareil renvoie
        /// ceux d'après (SAL-56, SAL-57).
        dernier_geste: u64,
        /// Le dernier numéro de tenu reçu de cet appareil, s'il y en a un : l'appareil
        /// numérote au-dessus (SAL-38).
        dernier_tenu: Option<u64>,
        /// Le pas définitif de la partie.
        pas: Pas,
        /// L'heure de l'autorité à l'envoi, en microsecondes : le temps de la partie
        /// depuis son pas 0 (`horloge::Horloge::partie_us`, lot 4b).
        heure: u64,
        /// L'empreinte du module qui se joue, s'il y en a un.
        module: Option<[u8; 32]>,
        /// Ce que l'appareil doit respecter.
        limites: LimitesSalle,
    },
    /// Un refus typé, lisible et définitif (SAL-42, REM-77).
    Refus(RefusSalle),
    /// Un morceau du module réécrit, compressé (P18).
    Module {
        /// L'empreinte du module.
        empreinte: [u8; 32],
        /// Le numéro du morceau, à partir de 0.
        morceau: u32,
        /// Le nombre de morceaux.
        total: u32,
        /// Ses octets.
        #[serde(borrow)]
        octets: &'a [u8],
    },
    /// Une police ou une image de la page, dans la même fenêtre que le module (§ 2.10).
    Ressource {
        /// La clé de l'asset.
        cle: u64,
        /// Ses octets.
        #[serde(borrow)]
        octets: &'a [u8],
    },
    /// La vue confirmée, entière (SAL-28).
    Vue {
        /// Le pas de la vue.
        pas: Pas,
        /// La vue, encodée par le module.
        #[serde(borrow)]
        montre: &'a [u8],
    },
    /// Un avis du jeu pour cet appareil.
    Avis {
        /// L'avis, encodé.
        #[serde(borrow)]
        octets: &'a [u8],
    },
    /// Un geste écarté : il a fait échouer le jeu (P39).
    Ecarte {
        /// Son numéro.
        numero: u64,
    },
    /// L'accusé durable : les gestes de cet onglet jusqu'à `jusqu_a` sont écrits dans
    /// la garde (PHASE-3, § 2.4, SAL-56).
    Accuse {
        /// Le dernier numéro accusé.
        jusqu_a: u64,
    },
    /// Le commun fait attendre l'appareil, et dit pourquoi (REM-69).
    Attente(AttenteCommun),
    /// Un écho, pour l'horloge.
    Echo {
        /// L'instant de l'envoi, horloge de l'émetteur, en microsecondes.
        t_us: u64,
        /// L'instant que l'écho rend, s'il répond à un écho de l'autre côté ; 0 sinon.
        echo_us: u64,
    },
    /// Les coups d'un bot, en mode banc seulement (PHASE-3, § 8.1).
    Banc(#[serde(borrow)] CoupBanc<'a>),
}

/// Ce que l'appareil a mesuré d'un geste (PHASE-3, § 8.4), dans le temps de l'autorité
/// tel qu'il l'estime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceTelephone {
    /// L'événement (`pointerdown`), en microsecondes.
    pub evenement_us: u64,
    /// L'envoi, en microsecondes.
    pub envoi_us: u64,
    /// L'incertitude de l'estimation : un demi-aller-retour.
    pub incertitude_us: u32,
}

/// Les limites que l'appareil fait respecter de son côté.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitesSalle {
    /// Les octets d'un geste ou d'un tenu.
    pub geste: u32,
    /// Les gestes en file au plus ; au-delà, `Envoi::FilePleine` (SAL-07).
    pub file: u16,
}

/// Un refus de la salle, typé (SAL-42).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefusSalle {
    /// L'appareil parle un autre protocole : « Version différente, recharge » (SAL-40).
    Protocole {
        /// [`PROTOCOLE`] de l'autorité.
        autorite: u32,
    },
    /// La salle a tous ses appareils.
    Pleine,
    /// La salle est fermée.
    Fermee,
    /// Un onglet plus récent de cet appareil l'a remplacé : « Remplacé — Reprendre ici »
    /// (SAL-18, SAL-21).
    Remplace,
}

/// Pourquoi le commun fait attendre un appareil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttenteCommun {
    /// Le jeu se charge.
    Chargement,
    /// Le jeu est en panne.
    Panne,
    /// L'autorité reprend (SAL-23).
    Reprise,
}

/// Un coup d'un bot, que l'appareil du banc envoie tel quel, par numérotation, file,
/// chiffrement et relais (PHASE-3, § 8.1, lots 4b et 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoupBanc<'a> {
    /// Un geste, encodé.
    Geste(&'a [u8]),
    /// Un état tenu, encodé.
    Tenu(&'a [u8]),
}

impl<'a> VersAutorite<'a> {
    /// Encode le message.
    pub fn encoder(&self) -> Result<Vec<u8>, ErreurEncodage> {
        let mut v = Vec::new();
        jeu_abi::encoder(self, &mut v)?;
        Ok(v)
    }

    /// Décode un message qui occupe exactement `octets` (SAL-118).
    pub fn decoder(octets: &'a [u8]) -> Result<VersAutorite<'a>, ErreurDecodage> {
        jeu_abi::decoder(octets)
    }
}

impl<'a> VersAppareil<'a> {
    /// Encode le message.
    pub fn encoder(&self) -> Result<Vec<u8>, ErreurEncodage> {
        let mut v = Vec::new();
        jeu_abi::encoder(self, &mut v)?;
        Ok(v)
    }

    /// Décode un message qui occupe exactement `octets` (SAL-118).
    pub fn decoder(octets: &'a [u8]) -> Result<VersAppareil<'a>, ErreurDecodage> {
        jeu_abi::decoder(octets)
    }
}

/// Le protocole qu'annonce un [`VersAutorite::Bonjour`], même quand le reste ne se lit
/// plus (SAL-40) : le numéro du variant (0), puis le varint de `protocole`. `None` si
/// les octets ne commencent pas ainsi.
pub fn protocole_d_un_bonjour(octets: &[u8]) -> Option<u32> {
    let (&variant, reste) = octets.split_first()?;
    if variant != 0 {
        return None;
    }
    let mut valeur: u64 = 0;
    for (rang, &b) in reste.iter().take(5).enumerate() {
        let decalage = u32::try_from(rang).ok()?.checked_mul(7)?;
        valeur |= u64::from(b & 0x7f).checked_shl(decalage)?;
        if b & 0x80 == 0 {
            return u32::try_from(valeur).ok();
        }
    }
    None
}
