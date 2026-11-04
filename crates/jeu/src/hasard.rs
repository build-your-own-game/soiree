//! Le générateur du jeu (§ 2.7) : xoshiro256**, semé par une graine de 256 bits, neuve
//! à chaque soirée sauf rejeu voulu (PUN-89, CAT-92, CAT-93). Il vit dans l'enveloppe
//! du module, en secret, et ne s'atteint qu'après `accepter` (D21).

use core::num::{NonZeroU32, NonZeroU64};

use jeu_abi::{Borne, Oui};

use crate::cache::{Ecrivain, Illisible, Liseur, forme};
use crate::forme::{Contient, Etat, Forme};
use crate::secret::Regard;

/// Le générateur : xoshiro256** (Blackman et Vigna, 2018).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hasard {
    s: [u64; 4],
}

/// Un pas de splitmix64, pour semer un état qui ne serait que des zéros.
const fn splitmix64(x: u64) -> u64 {
    let z = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let z = (z ^ z.wrapping_shr(30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    let z = (z ^ z.wrapping_shr(27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ z.wrapping_shr(31)
}

impl Hasard {
    /// Le générateur de cette graine : ses 32 octets en quatre mots petit-boutistes. Une
    /// graine nulle, interdite à xoshiro, est remplacée par quatre pas de splitmix64.
    pub(crate) fn depuis_graine(graine: &[u8; 32]) -> Hasard {
        let (mots, _) = graine.as_chunks::<8>();
        let mut s = [0u64; 4];
        for (case, mot) in s.iter_mut().zip(mots) {
            *case = u64::from_le_bytes(*mot);
        }
        if s == [0; 4] {
            let mut x = 0u64;
            for case in &mut s {
                x = splitmix64(x);
                *case = x;
            }
        }
        Hasard { s }
    }

    /// Le générateur d'un état brut, pour les tests de référence.
    #[doc(hidden)]
    pub const fn depuis_etat(s: [u64; 4]) -> Hasard {
        Hasard { s }
    }

    /// Un entier de 64 bits.
    pub fn entier(&mut self) -> u64 {
        let [s0, s1, s2, s3] = self.s;
        let resultat = s1.wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s1.wrapping_shl(17);
        let s2 = s2 ^ s0;
        let s3 = s3 ^ s1;
        let s1 = s1 ^ s2;
        let s0 = s0 ^ s3;
        let s2 = s2 ^ t;
        let s3 = s3.rotate_left(45);
        self.s = [s0, s1, s2, s3];
        resultat
    }

    /// Un entier dans `[0, n)`, sans biais : méthode de Lemire (CAT-91).
    pub fn borne(&mut self, n: NonZeroU64) -> u64 {
        let n = n.get();
        let mut m = u128::from(self.entier()).wrapping_mul(u128::from(n));
        let mut bas = m.cast_unsigned_bas();
        if bas < n {
            let seuil = n.wrapping_neg().checked_rem(n).unwrap_or(0);
            while bas < seuil {
                m = u128::from(self.entier()).wrapping_mul(u128::from(n));
                bas = m.cast_unsigned_bas();
            }
        }
        u64::try_from(m.wrapping_shr(64)).unwrap_or(0)
    }

    /// Un entier dans `[MIN, MAX]`, sans biais.
    pub fn dans<const MIN: i64, const MAX: i64>(&mut self) -> Borne<MIN, MAX> {
        let etendue = MAX.abs_diff(MIN).checked_add(1).and_then(NonZeroU64::new);
        let decalage = match etendue {
            Some(n) => self.borne(n),
            // Toute l'étendue d'un i64 : un entier quelconque convient.
            None => self.entier(),
        };
        let v = MIN.wrapping_add(decalage.cast_signed());
        Borne::new(v).unwrap_or(Borne::PLUS_PETIT)
    }

    /// Mélange `v` : Fisher-Yates, chaque ordre avec la même probabilité (CAT-89, CAT-90).
    pub fn melanger<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let Some(n) = u64::try_from(i)
                .ok()
                .and_then(|i| i.checked_add(1))
                .and_then(NonZeroU64::new)
            else {
                continue;
            };
            let j = usize::try_from(self.borne(n)).unwrap_or(0);
            v.swap(i, j);
        }
    }

    /// Un élément de `v` au hasard ; `None` si `v` est vide.
    pub fn choisir<'v, T>(&mut self, v: &'v [T]) -> Option<&'v T> {
        let n = NonZeroU64::new(u64::try_from(v.len()).ok()?)?;
        let i = usize::try_from(self.borne(n)).ok()?;
        v.get(i)
    }

    /// `Oui` une fois sur `n`.
    pub fn une_sur(&mut self, n: NonZeroU32) -> Oui {
        if self.borne(NonZeroU64::from(n)) == 0 {
            Oui::Oui
        } else {
            Oui::Non
        }
    }

    /// Un générateur indépendant, semé par celui-ci.
    #[must_use]
    pub fn deriver(&mut self) -> Hasard {
        let mut graine = [0u8; 32];
        for mot in graine.as_chunks_mut::<8>().0 {
            *mot = self.entier().to_le_bytes();
        }
        Hasard::depuis_graine(&graine)
    }
}

/// Les 64 bits bas d'un `u128`, sans conversion qui tronque en silence.
trait Bas {
    fn cast_unsigned_bas(self) -> u64;
}

impl Bas for u128 {
    fn cast_unsigned_bas(self) -> u64 {
        u64::try_from(self & u128::from(u64::MAX)).unwrap_or(0)
    }
}

impl Forme for Hasard {
    const EMPREINTE: u64 = forme::nommee("Hasard");
    const SECRET: Contient = Contient::Non;
    const VIDE: Contient = Contient::Non;
}

impl Etat for Hasard {
    fn projeter(&self, _: &Regard) -> Self {
        self.clone()
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        for m in self.s {
            e.octets(&m.to_le_bytes());
        }
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        let mut s = [0u64; 4];
        for case in &mut s {
            let b = l.octets(8)?;
            let t: [u8; 8] = b
                .try_into()
                .map_err(|_| l.erreur(jeu_abi::Illisible::Fin))?;
            *case = u64::from_le_bytes(t);
        }
        Ok(Hasard { s })
    }
}
