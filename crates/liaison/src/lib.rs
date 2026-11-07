//! Les trames **en clair** entre le relais et les bouts (PHASE-3, § 2.5 et § 2.7), et
//! les constantes du lien que le monde simulé de l'épreuve relit (P27).
//!
//! Le relais (`soiree-relais`) ne dépend que de cette crate (P15) : il fait passer des
//! trames qu'il ne sait pas lire. Ce qu'il voit est ici, et rien d'autre : le code de
//! salle, les liens, les rôles, la classe d'une trame, les cases de la garde par leur
//! numéro, les instants de ses horodatages. Ce qui est chiffré (les messages entre
//! l'autorité et un appareil, les cases de la garde) n'y passe qu'en octets opaques
//! (`trame`, `octets`), et se définit dans `salle`.
//!
//! - [`VersRelais`] : ce qu'un bout (l'autorité, sa garde, un appareil) envoie au relais ;
//! - [`DuRelais`] : ce que le relais envoie à un bout ;
//! - les constantes : [`PROTOCOLE_RELAIS`], [`TRAME_MAX`], [`BATTEMENT_MS`],
//!   [`SILENCE_MAX_MS`], [`LIENS_MAX_SANS_AUTORITE`], et la redite d'un état tenu,
//!   [`periode_de_redite`].
//!
//! # La forme
//!
//! postcard, décodée strictement par [`jeu_abi::decoder`] (un octet restant est un
//! refus, SAL-118 ; une longueur qui annonce plus que ce qui reste ne réserve rien,
//! REM-58). Chaque variant a ses octets figés et sa seconde lecture, sans serde ni
//! postcard, dans `tests/forme.rs` (SAL-90) : changer la forme sans monter
//! [`PROTOCOLE_RELAIS`] les fait rougir. Un variant s'ajoute à la fin de son enum.
//!
//! **`VersRelais::Bonjour` reste le premier variant, et `protocole` son premier
//! champ**, pour toujours : un bout d'un autre protocole se reconnaît même quand le
//! reste de son bonjour ne se lit plus ([`protocole_d_un_bonjour`], SAL-40).
//!
//! # `no_std`
//!
//! La crate se compile sans `std`, avec `alloc`, pour `wasm32-unknown-unknown`.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

extern crate alloc;

use alloc::vec::Vec;
use core::fmt;

use jeu_abi::{ApplicableFil, Duree, ErreurDecodage, ErreurEncodage, ReglesTenuFil};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Les constantes
// ---------------------------------------------------------------------------

/// La version des trames en clair. Monte quand la forme d'un type de cette crate
/// change ; `tests/forme.rs` la fige.
pub const PROTOCOLE_RELAIS: u16 = 1;

/// La plus grande trame chiffrée qu'un lien porte (`Vers`, `Trame`, `De`, `Pour`),
/// en octets : 256 Kio (choisi, PHASE-3, annexe des valeurs choisies). Un module
/// voyage en morceaux. Elle borne la mémoire du relais par trame ; M15 dira si elle
/// gêne.
pub const TRAME_MAX: u32 = 256 * 1024;

/// La plus longue ligne d'un flux de résultats (`VersRelais::AjouterLigne`, la demande
/// `Ligne` d'un jeu), en octets : 64 Kio (choisi, lot 11). Le relais refuse une ligne plus
/// longue, sans couper le lien.
pub const LIGNE_MAX: usize = 64 * 1024;

/// Une ligne qu'un flux accepte : au plus [`LIGNE_MAX`] octets, et aucun saut de ligne
/// (`\n`), qui la couperait en deux dans le fichier du flux, une ligne par ligne. L'autorité
/// refuse au jeu une ligne qui ne l'est pas ; le relais l'ignore, sans couper le lien.
pub fn ligne_valide(ligne: &[u8]) -> bool {
    ligne.len() <= LIGNE_MAX && !ligne.contains(&b'\n')
}

/// Un nom de flux que le relais accepte, comme nom de fichier sur son disque : de 1 à 64
/// caractères, lettres ASCII minuscules, chiffres, `-` et `_` (lot 11). Rien qui sorte du
/// dossier de la salle (`..`, `/`), rien qu'un système de fichiers lise autrement.
pub fn flux_valide(nom: &str) -> bool {
    (1..=64).contains(&nom.len())
        && nom
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_')
}

/// L'intervalle des battements, dans les deux sens, en millisecondes (SAL-43).
pub const BATTEMENT_MS: u32 = 250;

/// Le silence au-delà duquel un lien est absent, en millisecondes : 3 s (choisi ;
/// SAL-44 suggère 5 à 6 s, M5 le mesurera). C'est aussi le silence d'un téléphone
/// simulé avant que l'épreuve annonce son départ (`epreuve::telephone`, P27).
pub const SILENCE_MAX_MS: u32 = 3_000;

/// [`SILENCE_MAX_MS`] en pas, arrondi au pas supérieur (180 pas).
pub const SILENCE_MAX: Duree = Duree::ms(SILENCE_MAX_MS as u64);

/// Les liens qu'une salle admet tant que son autorité n'a pas dit
/// [`VersRelais::Limites`] (choisi, PHASE-3, § 2.5).
pub const LIENS_MAX_SANS_AUTORITE: u16 = 16;

/// La plus courte redite d'un état tenu : 100 ms (P27).
pub const REDITE_MIN: Duree = Duree::ms(100);

/// La plus longue redite d'un état tenu : 250 ms (P27, REM-13). C'est la redite du
/// téléphone simulé de l'épreuve, pour tous les jeux, jusqu'à ce que M7 vérifie la
/// période propre à chaque jeu par la vraie page (lot 12).
pub const REDITE_MAX: Duree = Duree::ms(250);

/// La période à laquelle une page redit l'état qu'elle tient, tant qu'il n'est pas
/// relâché (P27) : le tiers de la plus courte de la fraîcheur et de l'âge maximal,
/// arrondi au pas inférieur, entre [`REDITE_MIN`] et [`REDITE_MAX`]. Rien sans
/// fraîcheur ni âge : rien ne le relâche, rien n'a à le redire.
///
/// Sidequest (fraîcheur de 450 ms, âge d'une seconde) : 150 ms, 9 pas (SQ-01).
/// Punchline (ni l'une ni l'autre) : rien.
pub fn periode_de_redite(r: &ReglesTenuFil) -> Option<Duree> {
    let plus_court = match (applicable(&r.fraicheur), applicable(&r.age_max)) {
        (Some(f), Some(a)) => f.min(a),
        (Some(d), None) | (None, Some(d)) => d,
        (None, None) => return None,
    };
    let tiers = Duree::pas(plus_court.en_pas().div_euclid(3));
    Some(tiers.clamp(REDITE_MIN, REDITE_MAX))
}

fn applicable(r: &ApplicableFil<Duree>) -> Option<Duree> {
    match r {
        ApplicableFil::Oui(d) => Some(*d),
        ApplicableFil::SansObjet(_) => None,
    }
}

// ---------------------------------------------------------------------------
// Les noms
// ---------------------------------------------------------------------------

/// Le code d'une salle (P13) : quatre octets, affichés en 8 chiffres hexadécimaux
/// majuscules (`3FA9C2D1`). Il n'est pas secret : il nomme la salle, d'une soirée à
/// l'autre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CodeSalle(pub [u8; 4]);

impl CodeSalle {
    /// Lit un code de 8 chiffres hexadécimaux **majuscules**, la seule forme qu'on
    /// écrit : une seule écriture par code (SAL-119).
    pub fn lire(texte: &str) -> Option<CodeSalle> {
        let octets = texte.as_bytes();
        if octets.len() != 8 {
            return None;
        }
        let mut code = [0u8; 4];
        for (sortie, [fort, faible]) in code.iter_mut().zip(octets.as_chunks::<2>().0) {
            *sortie = chiffre_hexa(*fort)?
                .checked_mul(16)?
                .checked_add(chiffre_hexa(*faible)?)?;
        }
        Some(CodeSalle(code))
    }
}

fn chiffre_hexa(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => c.checked_sub(b'0'),
        b'A'..=b'F' => c.checked_sub(b'A')?.checked_add(10),
        _ => None,
    }
}

impl fmt::Display for CodeSalle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for o in self.0 {
            write!(f, "{o:02X}")?;
        }
        Ok(())
    }
}

/// Un lien, numéroté par le relais dans l'ordre où il les ouvre (SAL-19) : le plus
/// grand est le plus récent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Lien(pub u32);

/// Ce qu'un lien est dans sa salle, dit dans son [`VersRelais::Bonjour`] (P36).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// L'autorité : le Worker de l'écran. Boucle locale seulement (SAL-33).
    Autorite {
        /// `blake3::derive_key("soiree preuve d'autorite 1", G)` : la première autorité
        /// d'une salle l'enregistre, une autre preuve est refusée.
        preuve: [u8; 32],
    },
    /// La garde de l'autorité, une seconde WebSocket, pour qu'un gros point ne
    /// retienne pas le temps réel. Boucle locale seulement.
    Garde {
        /// La même preuve que l'autorité.
        preuve: [u8; 32],
        /// Le lien de l'autorité dont elle dépend : une garde d'une autorité remplacée
        /// n'écrit plus (SAL-20).
        autorite: Lien,
    },
    /// Un appareil : un téléphone.
    Appareil,
}

/// La classe d'une trame. Elle voyage en clair pour le relais, et dans les données
/// associées du chiffrement : le relais ne peut pas la changer (PHASE-3, § 2.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Classe {
    /// Dans la file fiable du lien, bornée en octets ; pleine, elle ferme le lien.
    Fiable,
    /// Une place par canal : la dernière trame d'un canal remplace celle qui attend
    /// (SAL-06, SAL-70).
    Remplacable(u8),
}

/// Une case de la garde, par son numéro (PHASE-3, § 2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Case {
    /// Un point : le point, l'ouverture et le registre de la salle.
    Point(u64),
    /// Un morceau du journal, et ce que la salle a appris pendant ce temps.
    Journal(u64),
}

/// Une adresse IP, en octets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AdresseIp {
    /// IPv4.
    V4([u8; 4]),
    /// IPv6.
    V6([u8; 16]),
}

/// Une adresse où le relais écoute et que les téléphones peuvent joindre : celle du
/// QR (PHASE-3, § 2.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdresseAnnoncee {
    /// L'adresse.
    pub ip: AdresseIp,
    /// Le port (7878 par défaut, P14).
    pub port: u16,
}

// ---------------------------------------------------------------------------
// Les trames
// ---------------------------------------------------------------------------

/// Ce qu'un bout envoie au relais.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersRelais<'a> {
    /// Le premier message d'un lien. **Toujours le premier variant, `protocole` son
    /// premier champ** (SAL-40). `Autorite` et `Garde` : boucle locale seulement
    /// (SAL-33).
    Bonjour {
        /// [`PROTOCOLE_RELAIS`] du bout.
        protocole: u16,
        /// La salle.
        salle: CodeSalle,
        /// Ce que le lien y est.
        role: Role,
    },
    /// L'autorité vers un appareil.
    Vers {
        /// Le lien de l'appareil.
        lien: Lien,
        /// La classe de la trame.
        classe: Classe,
        /// La trame chiffrée, opaque au relais.
        trame: &'a [u8],
    },
    /// Un appareil vers l'autorité.
    Trame {
        /// La classe de la trame.
        classe: Classe,
        /// La trame chiffrée, opaque au relais.
        trame: &'a [u8],
    },
    /// L'autorité coupe le lien d'un appareil.
    Couper {
        /// Le lien.
        lien: Lien,
        /// Pourquoi.
        raison: RaisonCoupure,
    },
    /// L'aller-retour jusqu'au relais seul.
    Echo {
        /// L'instant de l'envoi, horloge du bout, en microsecondes.
        t_envoi_us: u64,
    },
    /// Le bout s'en va : lu avant la fermeture (SAL-58).
    AuRevoir,
    /// La garde écrit une case, chiffrée (PHASE-3, § 2.4).
    Ecrire {
        /// La case.
        case: Case,
        /// Ses octets, chiffrés par `salle`.
        octets: &'a [u8],
    },
    /// La garde demande la liste des cases.
    Lister,
    /// La garde lit une case.
    Lire {
        /// La case.
        case: Case,
    },
    /// La garde efface les cases d'avant le point `avant`.
    Effacer {
        /// Le numéro du plus vieux point gardé.
        avant: u64,
    },
    /// La demande `Ligne` d'un jeu, écrite sur disque par le relais (P16).
    AjouterLigne {
        /// Le flux.
        flux: &'a str,
        /// La ligne.
        ligne: &'a [u8],
    },
    /// L'autorité borne les liens de sa salle : `places.appareils + 4`.
    Limites {
        /// Les liens au plus.
        liens_max: u16,
    },
}

/// Ce que le relais envoie à un bout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DuRelais<'a> {
    /// La réponse à un [`VersRelais::Bonjour`] accepté.
    Accueil {
        /// Tirée neuve à chaque lancement du relais (SAL-25).
        epoque: [u8; 16],
        /// Le lien du bout.
        lien: Lien,
        /// Les adresses que le relais annonce.
        adresses: Vec<AdresseAnnoncee>,
    },
    /// Un refus typé et définitif (SAL-42, SAL-148) ; le lien se ferme après.
    Refus(RefusRelais),
    /// À l'autorité : un appareil s'est relié.
    Ouvert {
        /// Son lien.
        lien: Lien,
        /// L'adresse locale qu'il a jointe (SAL-75).
        adresse_jointe: AdresseIp,
    },
    /// À l'autorité : une trame d'un appareil.
    De {
        /// Le lien de l'appareil.
        lien: Lien,
        /// La classe que l'appareil a dite.
        classe: Classe,
        /// La trame chiffrée.
        #[serde(borrow)]
        trame: &'a [u8],
        /// Sa réception par le relais, horloge du relais, en microsecondes.
        recu_us: u64,
        /// Son renvoi par le relais, même horloge.
        renvoye_us: u64,
    },
    /// À un appareil : une trame de l'autorité.
    Pour {
        /// La classe que l'autorité a dite.
        classe: Classe,
        /// La trame chiffrée.
        #[serde(borrow)]
        trame: &'a [u8],
    },
    /// À l'autorité : le lien d'un appareil s'est fermé.
    Ferme {
        /// Le lien.
        lien: Lien,
        /// Pourquoi.
        cause: CauseFermeture,
    },
    /// À l'autorité : la file fiable d'un appareil se remplit.
    Encombre {
        /// Le lien.
        lien: Lien,
        /// Les octets qui y attendent.
        octets_en_attente: u32,
    },
    /// À un appareil : la salle n'a plus d'autorité (SAL-23).
    AutoriteAbsente,
    /// À un appareil : l'autorité est revenue (SAL-23).
    AutoritePresente,
    /// La réponse à un [`VersRelais::Echo`].
    Echo {
        /// L'instant de l'envoi, rendu tel quel.
        t_envoi_us: u64,
        /// L'instant du relais à la réponse.
        t_relais_us: u64,
    },
    /// À la garde : la case est écrite et synchronisée sur disque (SAL-26).
    Ecrit {
        /// La case.
        case: Case,
    },
    /// À la garde : les cases gardées.
    Cases(Vec<Case>),
    /// À la garde : une case lue.
    Lu {
        /// La case.
        case: Case,
        /// Ses octets, chiffrés.
        #[serde(borrow)]
        octets: &'a [u8],
    },
}

/// Un refus du relais, typé et définitif (SAL-42, SAL-148).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefusRelais {
    /// Le bout parle un autre protocole que le relais (SAL-40).
    Protocole {
        /// [`PROTOCOLE_RELAIS`] du relais.
        relais: u16,
    },
    /// Aucune autorité n'a ouvert cette salle.
    SalleInconnue,
    /// La preuve d'autorité n'est pas celle que la salle a enregistrée (P41).
    PreuveFausse,
    /// Un rôle ou une route de la boucle locale demandé d'ailleurs (SAL-33, P40).
    HorsBoucleLocale,
    /// La salle a tous ses liens.
    Pleine,
    /// Une trame au-delà de [`TRAME_MAX`].
    TropGros,
    /// La garde de la salle est pleine.
    GardePleine,
    /// Une autorité plus récente tient la salle (P36, SAL-18).
    Remplacee,
}

/// Pourquoi l'autorité coupe le lien d'un appareil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaisonCoupure {
    /// L'autorité a refusé l'appareil, et le lui a dit.
    Refuse,
    /// Un lien plus récent du même appareil l'a remplacé (SAL-18).
    Remplace,
    /// Une trame qui ne se déchiffre pas : elle ne coûte que son lien (SAL-117).
    Illisible,
    /// Une poignée de main inachevée, ou aucun premier message de transport, en 10 s
    /// (PHASE-3, § 2.6).
    PoigneeInachevee,
    /// La soirée s'arrête.
    Fin,
}

/// Pourquoi le relais a fermé le lien d'un appareil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CauseFermeture {
    /// L'appareil a dit [`VersRelais::AuRevoir`] (SAL-58).
    AuRevoir,
    /// La connexion s'est fermée sans au revoir.
    Fermee,
    /// Aucun battement depuis [`SILENCE_MAX_MS`].
    Muet,
    /// Sa file fiable est pleine : il reviendra, et recevra l'état entier.
    FilePleine,
    /// Une trame au-delà de [`TRAME_MAX`].
    TropGros,
    /// Un message que le relais ne lit pas.
    Illisible,
    /// L'autorité l'a coupé ([`VersRelais::Couper`]).
    Coupe,
}

// ---------------------------------------------------------------------------
// Encoder, décoder
// ---------------------------------------------------------------------------

impl<'a> VersRelais<'a> {
    /// Encode le message.
    pub fn encoder(&self) -> Result<Vec<u8>, ErreurEncodage> {
        let mut v = Vec::new();
        jeu_abi::encoder(self, &mut v)?;
        Ok(v)
    }

    /// Décode un message qui occupe exactement `octets` (SAL-118).
    pub fn decoder(octets: &'a [u8]) -> Result<VersRelais<'a>, ErreurDecodage> {
        jeu_abi::decoder(octets)
    }
}

impl<'a> DuRelais<'a> {
    /// Encode le message.
    pub fn encoder(&self) -> Result<Vec<u8>, ErreurEncodage> {
        let mut v = Vec::new();
        jeu_abi::encoder(self, &mut v)?;
        Ok(v)
    }

    /// Décode un message qui occupe exactement `octets` (SAL-118).
    pub fn decoder(octets: &'a [u8]) -> Result<DuRelais<'a>, ErreurDecodage> {
        jeu_abi::decoder(octets)
    }
}

/// Le protocole qu'annonce un [`VersRelais::Bonjour`], même quand le reste ne se lit
/// plus (SAL-40) : le numéro du variant (0), puis le varint de `protocole`. `None` si
/// les octets ne commencent pas ainsi.
///
/// Un bonjour cassé d'un **autre** protocole reçoit `Refus(Protocole)` ; un bonjour
/// cassé du protocole courant est du bruit.
pub fn protocole_d_un_bonjour(octets: &[u8]) -> Option<u16> {
    let (&variant, reste) = octets.split_first()?;
    if variant != 0 {
        return None;
    }
    lire_varint_u16(reste)
}

/// Un varint LEB128 de postcard pour un `u16` : trois octets au plus, sans
/// dépassement.
fn lire_varint_u16(octets: &[u8]) -> Option<u16> {
    let mut valeur: u32 = 0;
    for (rang, &b) in octets.iter().take(3).enumerate() {
        let decalage = u32::try_from(rang).ok()?.checked_mul(7)?;
        valeur |= u32::from(b & 0x7f).checked_shl(decalage)?;
        if b & 0x80 == 0 {
            return u16::try_from(valeur).ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_code_de_salle_s_ecrit_et_se_relit() {
        let c = CodeSalle([0x3f, 0xa9, 0xc2, 0xd1]);
        assert_eq!(c.to_string(), "3FA9C2D1");
        assert_eq!(CodeSalle::lire("3FA9C2D1"), Some(c));
        assert_eq!(CodeSalle::lire("00000000"), Some(CodeSalle([0; 4])));
        assert_eq!(CodeSalle::lire("FFFFFFFF"), Some(CodeSalle([0xff; 4])));
    }

    /// SAL-119 : une seule écriture par code ; ni minuscules, ni longueur fausse, ni
    /// signe.
    #[test]
    fn un_code_de_salle_d_une_autre_forme_est_refuse_sal119() {
        for t in [
            "3fa9c2d1",
            "3FA9C2D",
            "3FA9C2D10",
            "",
            "3FA9C2G1",
            "+FA9C2D1",
            "3FA9 2D1",
        ] {
            assert_eq!(CodeSalle::lire(t), None, "{t:?}");
        }
    }

    #[test]
    fn le_varint_d_un_u16_se_lit() {
        assert_eq!(lire_varint_u16(&[1]), Some(1));
        assert_eq!(lire_varint_u16(&[0x80, 0x01]), Some(128));
        assert_eq!(lire_varint_u16(&[0xff, 0xff, 0x03]), Some(u16::MAX));
        assert_eq!(lire_varint_u16(&[0xff, 0xff, 0x04]), None);
        assert_eq!(lire_varint_u16(&[0xff, 0xff, 0xff]), None);
        assert_eq!(lire_varint_u16(&[0x80]), None);
        assert_eq!(lire_varint_u16(&[]), None);
    }
}
