//! Le format de l'enveloppe : encoder, décoder strictement, empreinter (§ 3.7).

use alloc::vec::Vec;
use core::fmt;

use serde::{Deserialize, Serialize};

/// Ce qui rend des octets illisibles. Une seule liste, pour le décodage strict et
/// pour le refus d'un appel par la frontière ([`crate::Retour::Illisible`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Illisible {
    /// Les octets s'arrêtent avant la fin de la valeur.
    Fin,
    /// La valeur est complète, et des octets restent après elle (REM-59, SAL-118).
    OctetsRestants,
    /// Un varint plus long que son type ne le permet.
    Varint,
    /// Un booléen qui ne vaut ni 0 ni 1.
    Booleen,
    /// Une option dont l'octet de tête ne vaut ni 0 ni 1.
    Option,
    /// Un texte qui n'est pas de l'UTF-8, ou un caractère invalide.
    Texte,
    /// Un numéro de variant au-delà de `u32::MAX`.
    Variant,
    /// Une valeur hors de son domaine : variant inconnu, NaN, nombre hors bornes,
    /// texte trop long ou sale, scène mal formée.
    Valeur,
    /// Une forme que le format ne sait pas lire, comme un type auto-décrit.
    Forme,
    /// Un appel hors de l'ordre attendu, comme une entrée avant la création.
    Ordre,
    /// Une limite dépassée : taille, nombre, profondeur.
    Limite,
    /// Une version ou une forme qui ne correspond pas : ABI, `formes`, empreinte.
    Version,
}

/// Où et pourquoi des octets sont illisibles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErreurFil {
    /// La position, en octets depuis le début, où la lecture s'est arrêtée.
    pub ou: u32,
    /// Ce qui ne va pas.
    pub quoi: Illisible,
}

impl fmt::Display for ErreurFil {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "octets illisibles à la position {} : {:?}",
            self.ou, self.quoi
        )
    }
}

/// L'erreur de [`decoder`] : la même que celle qui voyage dans l'enveloppe.
pub type ErreurDecodage = ErreurFil;

/// Ce qui empêche d'encoder une valeur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErreurEncodage {
    /// Une suite dont la longueur n'est pas connue d'avance : le format la préfixe.
    LongueurInconnue,
    /// Une valeur que son type refuse d'écrire.
    Valeur,
    /// Une forme que le format ne sait pas écrire.
    Forme,
}

impl fmt::Display for ErreurEncodage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let quoi = match self {
            ErreurEncodage::LongueurInconnue => "une suite de longueur inconnue",
            ErreurEncodage::Valeur => "une valeur refusée par son type",
            ErreurEncodage::Forme => "une forme que le format ne sait pas écrire",
        };
        write!(f, "encodage impossible : {quoi}")
    }
}

/// Ajoute l'encodage de `v` à la fin de `dans`.
///
/// En cas d'erreur, `dans` retrouve sa longueur d'avant l'appel.
pub fn encoder<T: Serialize + ?Sized>(v: &T, dans: &mut Vec<u8>) -> Result<(), ErreurEncodage> {
    let debut = dans.len();
    match postcard::serialize_with_flavor(v, VersVec(dans)) {
        Ok(()) => Ok(()),
        Err(e) => {
            dans.truncate(debut);
            Err(erreur_d_encodage(&e))
        }
    }
}

/// Décode une valeur qui occupe exactement `octets`.
///
/// Un octet restant est un refus (REM-59, OUT-146, SAL-118). Une suite qui annonce
/// plus d'éléments qu'il ne reste d'octets ne réserve rien pour eux : la mémoire
/// réservée suit les octets réellement présents (REM-58).
pub fn decoder<'a, T: Deserialize<'a>>(octets: &'a [u8]) -> Result<T, ErreurDecodage> {
    let mut lecteur = postcard::Deserializer::from_bytes(octets);
    let resultat = T::deserialize(&mut lecteur);
    let reste = lecteur.finalize().map_or(0, <[u8]>::len);
    let ou = u32::try_from(octets.len().saturating_sub(reste)).unwrap_or(u32::MAX);
    match resultat {
        Ok(v) if reste == 0 => Ok(v),
        Ok(_) => Err(ErreurFil {
            ou,
            quoi: Illisible::OctetsRestants,
        }),
        Err(e) => Err(ErreurFil {
            ou,
            quoi: illisibilite(&e),
        }),
    }
}

/// Ce que dit une erreur de postcard, dans le vocabulaire du commun.
#[expect(
    clippy::match_same_arms,
    reason = "les variants connus sont nommés un à un ; le dernier bras couvre ceux qu'ajoutera postcard"
)]
fn illisibilite(e: &postcard::Error) -> Illisible {
    match e {
        postcard::Error::DeserializeUnexpectedEnd => Illisible::Fin,
        postcard::Error::DeserializeBadVarint => Illisible::Varint,
        postcard::Error::DeserializeBadBool => Illisible::Booleen,
        postcard::Error::DeserializeBadOption => Illisible::Option,
        postcard::Error::DeserializeBadChar | postcard::Error::DeserializeBadUtf8 => {
            Illisible::Texte
        }
        postcard::Error::DeserializeBadEnum => Illisible::Variant,
        postcard::Error::SerdeDeCustom => Illisible::Valeur,
        postcard::Error::WontImplement
        | postcard::Error::NotYetImplemented
        | postcard::Error::SerializeBufferFull
        | postcard::Error::SerializeSeqLengthUnknown
        | postcard::Error::DeserializeBadEncoding
        | postcard::Error::DeserializeBadCrc
        | postcard::Error::SerdeSerCustom
        | postcard::Error::CollectStrError => Illisible::Forme,
        // `postcard::Error` est `non_exhaustive` : une erreur ajoutée par une version
        // de postcard reste une forme illisible, jamais une réussite.
        _ => Illisible::Forme,
    }
}

#[expect(
    clippy::match_same_arms,
    reason = "les variants connus sont nommés un à un ; le dernier bras couvre ceux qu'ajoutera postcard"
)]
fn erreur_d_encodage(e: &postcard::Error) -> ErreurEncodage {
    match e {
        postcard::Error::SerializeSeqLengthUnknown => ErreurEncodage::LongueurInconnue,
        postcard::Error::SerdeSerCustom | postcard::Error::CollectStrError => {
            ErreurEncodage::Valeur
        }
        postcard::Error::WontImplement
        | postcard::Error::NotYetImplemented
        | postcard::Error::SerializeBufferFull
        | postcard::Error::DeserializeUnexpectedEnd
        | postcard::Error::DeserializeBadVarint
        | postcard::Error::DeserializeBadBool
        | postcard::Error::DeserializeBadChar
        | postcard::Error::DeserializeBadUtf8
        | postcard::Error::DeserializeBadOption
        | postcard::Error::DeserializeBadEnum
        | postcard::Error::DeserializeBadEncoding
        | postcard::Error::DeserializeBadCrc
        | postcard::Error::SerdeDeCustom => ErreurEncodage::Forme,
        // Voir `illisibilite` : `non_exhaustive`.
        _ => ErreurEncodage::Forme,
    }
}

/// Écrit à la fin d'un `Vec` emprunté.
struct VersVec<'v>(&'v mut Vec<u8>);

impl postcard::ser_flavors::Flavor for VersVec<'_> {
    type Output = ();

    fn try_extend(&mut self, data: &[u8]) -> postcard::Result<()> {
        self.0.extend_from_slice(data);
        Ok(())
    }

    fn try_push(&mut self, data: u8) -> postcard::Result<()> {
        self.0.push(data);
        Ok(())
    }

    fn finalize(self) -> postcard::Result<Self::Output> {
        Ok(())
    }
}

const FNV_DEPART: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PREMIER: u64 = 0x0100_0000_01b3;

/// FNV-1a 64 bits de `octets`. Évaluable à la compilation.
///
/// Ce n'est pas une empreinte cryptographique : elle nomme des clés, des effets et
/// des formes, jamais une version qu'un adversaire voudrait imiter (BLAKE3 s'en charge,
/// côté moteur).
pub const fn fnv1a64(octets: &[u8]) -> u64 {
    fnv1a64_suite(FNV_DEPART, octets)
}

/// Continue un FNV-1a 64 bits depuis un état déjà calculé : `fnv1a64_suite(fnv1a64(a), b)`
/// vaut `fnv1a64` de `a` suivi de `b`. Évaluable à la compilation.
pub const fn fnv1a64_suite(etat: u64, octets: &[u8]) -> u64 {
    let mut h = etat;
    let mut reste = octets;
    while let [premier, suite @ ..] = reste {
        h ^= *premier as u64;
        h = h.wrapping_mul(FNV_PREMIER);
        reste = suite;
    }
    h
}

/// Continue un FNV-1a 64 bits avec le varint de postcard de `n` : sept bits par
/// octet, les moins forts d'abord, le bit fort posé tant qu'il en reste. Évaluable à
/// la compilation.
pub(crate) const fn fnv1a64_varint(etat: u64, n: u64) -> u64 {
    let mut h = etat;
    let mut reste = n;
    loop {
        let bas = (reste & 0x7f) as u8;
        reste >>= 7;
        let octet = if reste == 0 { bas } else { bas | 0x80 };
        h ^= octet as u64;
        h = h.wrapping_mul(FNV_PREMIER);
        if reste == 0 {
            return h;
        }
    }
}

/// Un FNV-1a 64 bits qu'on nourrit morceau par morceau.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fnv1a(u64);

impl Fnv1a {
    /// L'état de départ, celui d'une suite vide.
    pub const fn new() -> Fnv1a {
        Fnv1a(FNV_DEPART)
    }

    /// Ajoute des octets.
    pub const fn ecrire(&mut self, octets: &[u8]) {
        self.0 = fnv1a64_suite(self.0, octets);
    }

    /// L'empreinte des octets écrits jusqu'ici.
    pub const fn valeur(&self) -> u64 {
        self.0
    }
}

impl Default for Fnv1a {
    fn default() -> Self {
        Fnv1a::new()
    }
}

/// Nourrit un FNV-1a avec les octets de l'encodage, sans les garder.
struct VersFnv<'h>(&'h mut Fnv1a);

impl postcard::ser_flavors::Flavor for VersFnv<'_> {
    type Output = ();

    fn try_extend(&mut self, data: &[u8]) -> postcard::Result<()> {
        self.0.ecrire(data);
        Ok(())
    }

    fn try_push(&mut self, data: u8) -> postcard::Result<()> {
        self.0.ecrire(&[data]);
        Ok(())
    }

    fn finalize(self) -> postcard::Result<Self::Output> {
        Ok(())
    }
}

/// L'octet ajouté à l'empreinte d'une valeur que le format n'a pas pu écrire en
/// entier : l'empreinte reste une fonction de la valeur, et se distingue de celle
/// des octets écrits seuls.
const MARQUE_ENCODAGE_IMPOSSIBLE: u8 = 0xff;

/// FNV-1a 64 de l'encodage de `v`, calculé sans allouer.
///
/// Une valeur que le format ne sait pas écrire (une suite de longueur inconnue)
/// donne l'empreinte des octets écrits jusque-là, suivis d'un octet `0xff`.
pub(crate) fn empreinte_de<T: Serialize + ?Sized>(v: &T) -> u64 {
    let mut h = Fnv1a::new();
    if postcard::serialize_with_flavor(v, VersFnv(&mut h)).is_err() {
        h.ecrire(&[MARQUE_ENCODAGE_IMPOSSIBLE]);
    }
    h.valeur()
}
