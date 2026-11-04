//! Deux formes que le module écrit et que le commun lit sans connaître le jeu : la
//! zone de panique (§ 3.2) et les bords de la forme canonique (§ 3.5).
//!
//! Elles ne passent pas par postcard : la zone de panique s'écrit pendant une panique,
//! sans allouer, et les bords de la forme canonique se lisent sans décoder le corps.
//! Leur définition unique est ici, pour que la colle de `jeu` qui les écrit et le
//! moteur ou l'épreuve qui les lisent ne puissent pas diverger.
//!
//! # La zone de panique
//!
//! À l'adresse que `Bonjour` annonce : un code (`u32` petit-boutiste, 0 sans
//! panique, [`PANIQUE_CODE`] après une panique), la longueur du message (`u32`
//! petit-boutiste), puis le message en UTF-8, coupé à la capacité de la zone entre
//! deux caractères. Le moteur remet les [`PANIQUE_ENTETE`] premiers octets à zéro
//! avant chaque appel ; la colle aussi.
//!
//! # Les bords de la forme canonique
//!
//! `forme` (`u64` petit-boutiste, l'empreinte de la forme du type de l'état) ‖ corps
//! ‖ `non_finis` (`u64` petit-boutiste, le nombre de flottants non finis du corps,
//! E17, CAT-05, SQ-68). `DepuisCanonique` reçoit `forme` en paramètre : ce sont les
//! huit premiers octets de ce que `Canonique` a rendu.

/// Le code d'une panique dans la zone de panique.
pub const PANIQUE_CODE: u32 = 1;

/// La taille de l'entête de la zone de panique : le code et la longueur.
pub const PANIQUE_ENTETE: u32 = 8;

/// L'entête d'une zone de panique qui porte un message de `longueur` octets.
pub const fn entete_de_panique(longueur: u32) -> [u8; 8] {
    let c = PANIQUE_CODE.to_le_bytes();
    let l = longueur.to_le_bytes();
    [c[0], c[1], c[2], c[3], l[0], l[1], l[2], l[3]]
}

/// Le message d'une zone de panique, s'il y a eu une panique : ses octets, coupés à
/// la fin de la zone si la longueur écrite la dépasse. Ils sont UTF-8 quand la colle
/// de `jeu` les a écrits ; un module écrit sans elle peut mentir.
pub fn lire_panique(zone: &[u8]) -> Option<&[u8]> {
    let (code, reste) = zone.split_first_chunk::<4>()?;
    if u32::from_le_bytes(*code) != PANIQUE_CODE {
        return None;
    }
    let (longueur, message) = reste.split_first_chunk::<4>()?;
    let n = usize::try_from(u32::from_le_bytes(*longueur)).unwrap_or(usize::MAX);
    message.get(..n.min(message.len()))
}

/// Une forme canonique, découpée en ses bords et son corps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Canonique<'a> {
    /// L'empreinte de la forme du type de l'état.
    pub forme: u64,
    /// Le corps, que seul le jeu sait lire.
    pub corps: &'a [u8],
    /// Le nombre de flottants non finis du corps.
    pub non_finis: u64,
}

/// Découpe une forme canonique entière ; `None` si elle est plus courte que ses deux
/// bords.
pub fn lire_canonique(octets: &[u8]) -> Option<Canonique<'_>> {
    let (forme, reste) = octets.split_first_chunk::<8>()?;
    let (corps, non_finis) = reste.split_last_chunk::<8>()?;
    Some(Canonique {
        forme: u64::from_le_bytes(*forme),
        corps,
        non_finis: u64::from_le_bytes(*non_finis),
    })
}
