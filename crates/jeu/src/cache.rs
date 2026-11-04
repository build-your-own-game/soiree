//! La forme canonique, scellée (§ 2.2, D24) : ce qu'écrit et relit `Etat::ecrire` et
//! `Etat::lire`. Cachée de la documentation : un jeu n'a rien à y faire.
//!
//! [`Ecrivain`] et [`Liseur`] ne se construisent que dans `jeu` : le code d'un jeu
//! peut appeler `Etat::ecrire`, mais n'a jamais d'écrivain à lui passer. Il ne peut
//! donc pas sérialiser un secret pour en lire les octets.
//!
//! # Le format
//!
//! - entiers non signés en varint LEB128, signés en zigzag puis varint, `u8` et `i8`
//!   sur un octet brut ; `bool` sur un octet 0 ou 1 ; `char` comme son point de code ;
//! - flottants en petit-boutiste, comptés quand ils ne sont pas finis (CAT-05, SQ-68) ;
//! - textes et suites préfixés de leur longueur ; variants numérotés en varint ;
//! - un `Message` : son encodage postcard, préfixé de sa longueur, relu strictement
//!   par [`jeu_abi::decoder`], donc validé comme au décodage d'une entrée ;
//! - un `Partage` : une fois par allocation, puis un renvoi à son rang (TL-20).
//!
//! L'écrivain ne garde que les octets d'une fenêtre : la forme canonique d'un gros état
//! se lit par morceaux (`Appel::Canonique`) sans tenir en entier dans le brouillon.

use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::any::Any;

pub use jeu_abi::ErreurFil as Illisible;
use jeu_abi::Illisible as Quoi;

/// La dépendance `serde` des types que dérive `#[jeu::message]`, pour que le jeu n'ait
/// pas à la déclarer lui-même.
#[doc(hidden)]
pub use serde;

/// L'empreinte de forme, calculée à la compilation (§ 2.2). Utilisé par les macros.
pub mod forme {
    use crate::Contient;
    use jeu_abi::{fnv1a64, fnv1a64_suite};

    /// Le départ d'une empreinte : la version de l'algorithme, puis la sorte de type.
    pub const fn depart(sorte: &str) -> u64 {
        mot(fnv1a64(b"jeu::Forme, version 1"), sorte)
    }

    /// Ajoute un nom, préfixé de sa longueur : `ab` puis `c` ne vaut pas `a` puis `bc`.
    pub const fn mot(h: u64, m: &str) -> u64 {
        let h = fnv1a64_suite(h, &(m.len() as u64).to_le_bytes());
        fnv1a64_suite(h, m.as_bytes())
    }

    /// Ajoute un nombre : un compte, ou l'empreinte d'un champ.
    pub const fn entier(h: u64, n: u64) -> u64 {
        fnv1a64_suite(h, &n.to_le_bytes())
    }

    /// L'empreinte d'un type du commun ou d'une primitive, par son nom seul : sa forme
    /// est figée par ailleurs (`forme_figee` de `jeu-abi`, ou le langage).
    pub const fn nommee(nom: &str) -> u64 {
        mot(depart("commun"), nom)
    }

    /// L'empreinte d'un type à un paramètre.
    pub const fn de(nom: &str, parametre: u64) -> u64 {
        entier(nommee(nom), parametre)
    }

    /// `Oui` si l'un des deux l'est.
    pub const fn ou(a: Contient, b: Contient) -> Contient {
        match (a, b) {
            (Contient::Non, Contient::Non) => Contient::Non,
            (Contient::Oui, _) | (_, Contient::Oui) => Contient::Oui,
        }
    }
}

/// Où va ce qu'écrit l'écrivain.
enum Cible<'v> {
    /// Les octets d'une fenêtre `[debut, fin)` du flux, ajoutés à ce tampon.
    Fenetre {
        debut: usize,
        fin: usize,
        dans: &'v mut Vec<u8>,
    },
}

/// L'écrivain de la forme canonique. Ne se construit que dans `jeu`.
pub struct Ecrivain<'v> {
    cible: Cible<'v>,
    /// Le nombre d'octets du flux écrits jusqu'ici, y compris hors de la fenêtre.
    position: usize,
    /// Les flottants non finis écrits (CAT-05, SQ-68).
    non_finis: u64,
    /// Les `Partage` déjà écrits, par adresse d'allocation, avec leur rang.
    partages: BTreeMap<usize, u64>,
    /// Une erreur d'encodage d'un message : la forme ne vaut rien.
    echec: Option<jeu_abi::ErreurEncodage>,
}

impl<'v> Ecrivain<'v> {
    /// Un écrivain qui ajoute tout le flux à `dans`.
    pub(crate) fn tout(dans: &'v mut Vec<u8>) -> Ecrivain<'v> {
        Ecrivain::fenetre(dans, 0, usize::MAX)
    }

    /// Un écrivain qui n'ajoute à `dans` que les octets `[debut, fin)` du flux.
    pub(crate) fn fenetre(dans: &'v mut Vec<u8>, debut: usize, fin: usize) -> Ecrivain<'v> {
        Ecrivain {
            cible: Cible::Fenetre { debut, fin, dans },
            position: 0,
            non_finis: 0,
            partages: BTreeMap::new(),
            echec: None,
        }
    }

    /// La longueur du flux écrit jusqu'ici.
    pub(crate) fn position(&self) -> usize {
        self.position
    }

    /// Les flottants non finis écrits.
    pub(crate) fn non_finis(&self) -> u64 {
        self.non_finis
    }

    /// L'erreur d'encodage, s'il y en a eu une.
    pub(crate) fn echec(&self) -> Option<jeu_abi::ErreurEncodage> {
        self.echec
    }

    /// Des octets bruts.
    pub fn octets(&mut self, b: &[u8]) {
        let Cible::Fenetre { debut, fin, dans } = &mut self.cible;
        let depart = self.position;
        let arrivee = depart.saturating_add(b.len());
        let de = (*debut).max(depart);
        let a = (*fin).min(arrivee);
        if de < a
            && let Some(morceau) = b.get(de.saturating_sub(depart)..a.saturating_sub(depart))
        {
            dans.extend_from_slice(morceau);
        }
        self.position = arrivee;
    }

    /// Un varint LEB128.
    pub fn varint(&mut self, mut n: u128) {
        let mut tampon = [0u8; 19];
        let mut i = 0usize;
        for case in &mut tampon {
            let septet = u8::try_from(n & 0x7f).unwrap_or(0);
            n = n.wrapping_shr(7);
            *case = if n == 0 { septet } else { septet | 0x80 };
            i = i.saturating_add(1);
            if n == 0 {
                break;
            }
        }
        self.octets(tampon.get(..i).unwrap_or(&[]));
    }

    /// Un entier signé, en zigzag.
    pub fn signe(&mut self, n: i128) {
        let z = n.wrapping_shl(1) ^ n.wrapping_shr(127);
        self.varint(z.cast_unsigned());
    }

    /// Le rang d'un variant.
    pub fn variant(&mut self, rang: u32) {
        self.varint(u128::from(rang));
    }

    /// Une longueur.
    pub fn longueur(&mut self, n: usize) {
        self.varint(n as u128);
    }

    /// Un `f32`, en petit-boutiste ; compté s'il n'est pas fini.
    pub fn f32(&mut self, x: f32) {
        if !x.is_finite() {
            self.non_finis = self.non_finis.saturating_add(1);
        }
        self.octets(&x.to_bits().to_le_bytes());
    }

    /// Un `f64`, en petit-boutiste ; compté s'il n'est pas fini.
    pub fn f64(&mut self, x: f64) {
        if !x.is_finite() {
            self.non_finis = self.non_finis.saturating_add(1);
        }
        self.octets(&x.to_bits().to_le_bytes());
    }

    /// Un texte, préfixé de sa longueur.
    pub fn texte(&mut self, s: &str) {
        self.longueur(s.len());
        self.octets(s.as_bytes());
    }

    /// Un message : son encodage postcard, préfixé de sa longueur.
    pub fn message<T: serde::Serialize + ?Sized>(&mut self, v: &T) {
        let mut tampon = Vec::new();
        if let Err(e) = jeu_abi::encoder(v, &mut tampon) {
            self.echec = Some(e);
        }
        self.longueur(tampon.len());
        self.octets(&tampon);
    }

    /// Un contenu partagé : écrit une fois par allocation, puis par son rang (TL-20).
    /// Rend `true` si le contenu reste à écrire.
    pub(crate) fn partage<T: 'static>(&mut self, rc: &Rc<T>) -> bool {
        let adresse = Rc::as_ptr(rc).addr();
        if let Some(rang) = self.partages.get(&adresse).copied() {
            self.variant(1);
            self.varint(u128::from(rang));
            false
        } else {
            let rang = self.partages.len() as u64;
            let _: Option<u64> = self.partages.insert(adresse, rang);
            self.variant(0);
            true
        }
    }
}

/// Le liseur de la forme canonique. Ne se construit que dans `jeu`.
pub struct Liseur<'o> {
    octets: &'o [u8],
    position: usize,
    /// Les `Partage` déjà lus, dans l'ordre de leur rang.
    partages: Vec<Rc<dyn Any>>,
}

impl<'o> Liseur<'o> {
    pub(crate) fn new(octets: &'o [u8]) -> Liseur<'o> {
        Liseur {
            octets,
            position: 0,
            partages: Vec::new(),
        }
    }

    /// Ce qui reste à lire.
    pub(crate) fn reste(&self) -> &'o [u8] {
        self.octets.get(self.position..).unwrap_or(&[])
    }

    /// L'erreur `quoi` à la position courante.
    pub fn erreur(&self, quoi: Quoi) -> Illisible {
        Illisible {
            ou: u32::try_from(self.position).unwrap_or(u32::MAX),
            quoi,
        }
    }

    /// L'erreur d'un variant inconnu.
    pub fn variant_inconnu(&self) -> Illisible {
        self.erreur(Quoi::Valeur)
    }

    /// `n` octets bruts.
    pub fn octets(&mut self, n: usize) -> Result<&'o [u8], Illisible> {
        let fin = self.position.checked_add(n).ok_or(self.erreur(Quoi::Fin))?;
        let b = self
            .octets
            .get(self.position..fin)
            .ok_or(self.erreur(Quoi::Fin))?;
        self.position = fin;
        Ok(b)
    }

    /// Un octet.
    pub fn octet(&mut self) -> Result<u8, Illisible> {
        let b = self
            .octets
            .get(self.position)
            .copied()
            .ok_or(self.erreur(Quoi::Fin))?;
        self.position = self.position.saturating_add(1);
        Ok(b)
    }

    /// Un varint d'au plus `bits` bits.
    pub fn varint(&mut self, bits: u32) -> Result<u128, Illisible> {
        let mut n: u128 = 0;
        let mut decalage: u32 = 0;
        loop {
            let b = self.octet()?;
            let septet = u128::from(b & 0x7f);
            if decalage >= bits
                || (decalage > 0 && septet.wrapping_shr(bits.saturating_sub(decalage)) != 0)
            {
                return Err(self.erreur(Quoi::Varint));
            }
            n |= septet.wrapping_shl(decalage);
            if b & 0x80 == 0 {
                return Ok(n);
            }
            decalage = decalage.saturating_add(7);
        }
    }

    /// Un entier signé en zigzag, d'au plus `bits` bits.
    pub fn signe(&mut self, bits: u32) -> Result<i128, Illisible> {
        let z = self.varint(bits)?;
        let n = z.wrapping_shr(1).cast_signed() ^ (z & 1).cast_signed().wrapping_neg();
        Ok(n)
    }

    /// Le rang d'un variant.
    pub fn variant(&mut self) -> Result<u32, Illisible> {
        let v = self.varint(32)?;
        u32::try_from(v).map_err(|_| self.erreur(Quoi::Variant))
    }

    /// Une longueur de suite dont chaque élément prend au moins un octet : jamais plus
    /// grande que ce qui reste (REM-58).
    pub fn longueur(&mut self) -> Result<usize, Illisible> {
        let n = self.longueur_sans_borne()?;
        if n > self.reste().len() {
            return Err(self.erreur(Quoi::Fin));
        }
        Ok(n)
    }

    /// Une longueur de suite d'éléments qui peuvent ne prendre aucun octet.
    pub fn longueur_sans_borne(&mut self) -> Result<usize, Illisible> {
        let v = self.varint(64)?;
        usize::try_from(v).map_err(|_| self.erreur(Quoi::Limite))
    }

    /// Un booléen.
    pub fn booleen(&mut self) -> Result<bool, Illisible> {
        match self.octet()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(self.erreur(Quoi::Booleen)),
        }
    }

    /// Un texte, en UTF-8 valide.
    pub fn texte(&mut self) -> Result<&'o str, Illisible> {
        let n = self.longueur()?;
        let b = self.octets(n)?;
        core::str::from_utf8(b).map_err(|_| self.erreur(Quoi::Texte))
    }

    /// Un message : relu strictement, donc validé (SAL-117).
    pub fn message<T: serde::de::DeserializeOwned>(&mut self) -> Result<T, Illisible> {
        let n = self.longueur()?;
        let debut = self.position;
        let b = self.octets(n)?;
        jeu_abi::decoder::<T>(b).map_err(|e| Illisible {
            ou: u32::try_from(debut)
                .unwrap_or(u32::MAX)
                .saturating_add(e.ou),
            quoi: e.quoi,
        })
    }

    /// Un contenu partagé : neuf, ou un renvoi à un contenu déjà lu de même type.
    pub(crate) fn partage<T: 'static>(
        &mut self,
        lire: impl FnOnce(&mut Liseur<'o>) -> Result<T, Illisible>,
    ) -> Result<Rc<T>, Illisible> {
        match self.variant()? {
            0 => {
                let v = Rc::new(lire(self)?);
                let tout: Rc<dyn Any> = v.clone();
                self.partages.push(tout);
                Ok(v)
            }
            1 => {
                let rang =
                    usize::try_from(self.varint(64)?).map_err(|_| self.erreur(Quoi::Valeur))?;
                let deja = self
                    .partages
                    .get(rang)
                    .cloned()
                    .ok_or(self.erreur(Quoi::Valeur))?;
                deja.downcast::<T>()
                    .map_err(|_: Rc<dyn Any>| self.erreur(Quoi::Valeur))
            }
            _ => Err(self.variant_inconnu()),
        }
    }
}
