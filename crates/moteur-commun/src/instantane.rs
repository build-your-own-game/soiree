//! L'instantané d'une instance, hors des quatre zones (§ 4.5 ; PHASE-3, § 2.4, P3), et
//! sa forme dans la garde.

use alloc::format;
use alloc::sync::Arc;
use alloc::vec::Vec;

use partie::persistance::{InstantaneGarde, RefusGarde};

use crate::machine::Bits;
use crate::protocole::OCTETS_PAR_PAGE;

/// L'instantané d'une instance : la mémoire **hors des quatre zones** que le moteur
/// remet à zéro avant chaque appel (la pile, la boîte, le brouillon, la zone de
/// panique), sa taille, et les globales mutables d'origine. Les globales du moteur n'y
/// sont pas : il les pose avant chaque appel. La restauration remet les zones à zéro :
/// elles ne portaient rien d'un appel au suivant (P3).
///
/// Se clone sans copier la mémoire : un point de reprise, un bot et la tête le
/// partagent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instantane {
    module: [u8; 32],
    pages: u64,
    plages: Arc<[(u64, u64)]>,
    octets: Arc<[u8]>,
    globales: Arc<[Bits]>,
}

impl Instantane {
    pub(crate) fn new(
        module: [u8; 32],
        pages: u64,
        gardees: Vec<(u64, u64)>,
        octets: Vec<u8>,
        globales: Vec<Bits>,
    ) -> Instantane {
        Instantane {
            module,
            pages,
            plages: Arc::from(gardees),
            octets: Arc::from(octets),
            globales: Arc::from(globales),
        }
    }

    /// L'empreinte du module réécrit dont c'est l'instantané.
    pub fn module(&self) -> [u8; 32] {
        self.module
    }

    /// La taille de la mémoire de l'instantané, en pages.
    pub fn pages(&self) -> u64 {
        self.pages
    }

    /// Les plages de la mémoire gardées, `(adresse, longueur)`, dans l'ordre : la
    /// mémoire hors des quatre zones.
    pub fn plages(&self) -> &[(u64, u64)] {
        &self.plages
    }

    /// Les octets des plages, mis bout à bout.
    pub fn octets(&self) -> &[u8] {
        &self.octets
    }

    /// Les globales mutables d'origine, dans l'ordre de leurs indices.
    pub fn globales(&self) -> &[Bits] {
        &self.globales
    }
}

/// L'instantané d'un module se garde (PHASE-3, § 2.4). Sa forme, depuis
/// `partie::persistance::VERSION_GARDE` 2 : les 32 octets de l'empreinte du module
/// réécrit ; le nombre de pages, en `u64` petit-boutiste ; le nombre de plages, en
/// `u32` ; puis chaque plage, son adresse et sa longueur en `u64`, suivies de ses
/// octets ; le nombre de globales mutables d'origine, en `u32`, puis chacune : une
/// étiquette (0 `i32`, 1 `i64`, 2 `f32`, 3 `f64`) et ses bits en `u64`, comme
/// l'empreinte les lit.
///
/// La relecture est stricte : un octet de trop ou de moins, une plage vide, des plages
/// dans le désordre ou qui se chevauchent, une plage au-delà de la mémoire, une
/// étiquette inconnue, des bits hauts dans une globale de 32 bits sont refusés. Que les
/// plages soient exactement le reste des zones du module, la restauration le vérifie :
/// elle seule connaît les zones.
impl InstantaneGarde for Instantane {
    fn ecrire(&self, dans: &mut Vec<u8>) {
        dans.extend_from_slice(&self.module);
        dans.extend_from_slice(&self.pages.to_le_bytes());
        let n = u32::try_from(self.plages.len()).unwrap_or(u32::MAX);
        dans.extend_from_slice(&n.to_le_bytes());
        let mut reste: &[u8] = &self.octets;
        for (adresse, longueur) in self.plages.iter() {
            dans.extend_from_slice(&adresse.to_le_bytes());
            dans.extend_from_slice(&longueur.to_le_bytes());
            let k = usize::try_from(*longueur).unwrap_or(usize::MAX);
            let (plage, apres) = reste.split_at_checked(k).unwrap_or((reste, &[]));
            dans.extend_from_slice(plage);
            reste = apres;
        }
        let n = u32::try_from(self.globales.len()).unwrap_or(u32::MAX);
        dans.extend_from_slice(&n.to_le_bytes());
        for g in self.globales.iter() {
            let (t, b) = g.octets();
            dans.extend_from_slice(&t);
            dans.extend_from_slice(&b);
        }
    }

    fn lire(octets: &[u8]) -> Result<Self, RefusGarde> {
        let refus = |quoi: &str| RefusGarde::Instantane(format!("instantané de module : {quoi}"));
        let mut reste = octets;
        let mut prendre = |n: usize| -> Result<&[u8], RefusGarde> {
            let (t, r) = reste.split_at_checked(n).ok_or_else(|| refus("coupé"))?;
            reste = r;
            Ok(t)
        };
        let u64_de = |t: &[u8]| {
            t.try_into()
                .map(u64::from_le_bytes)
                .map_err(|_| refus("entier"))
        };
        let u32_de = |t: &[u8]| {
            t.try_into()
                .map(u32::from_le_bytes)
                .map_err(|_| refus("entier"))
        };
        let module: [u8; 32] = prendre(32)?.try_into().map_err(|_| refus("empreinte"))?;
        let pages = u64_de(prendre(8)?)?;
        let memoire = pages
            .checked_mul(OCTETS_PAR_PAGE)
            .ok_or_else(|| refus("trop de pages"))?;
        let n = u32_de(prendre(4)?)?;
        let mut lues = Vec::new();
        let mut gardes = Vec::new();
        let mut fin_precedente = 0u64;
        for _ in 0..n {
            let adresse = u64_de(prendre(8)?)?;
            let longueur = u64_de(prendre(8)?)?;
            let fin = adresse
                .checked_add(longueur)
                .ok_or_else(|| refus("plage hors de la mémoire"))?;
            if longueur == 0 {
                return Err(refus("plage vide"));
            }
            if adresse < fin_precedente {
                return Err(refus("plages dans le désordre ou qui se chevauchent"));
            }
            if fin > memoire {
                return Err(refus("plage hors de la mémoire"));
            }
            let k = usize::try_from(longueur).map_err(|_| refus("plage trop longue"))?;
            gardes.extend_from_slice(prendre(k)?);
            lues.push((adresse, longueur));
            fin_precedente = fin;
        }
        let n = u32_de(prendre(4)?)?;
        let mut globales = Vec::new();
        for _ in 0..n {
            let t = prendre(1)?.first().copied().ok_or_else(|| refus("coupé"))?;
            let b = u64_de(prendre(8)?)?;
            // Les bits hauts d'un `i32` ou d'un `f32` sont nuls à l'écriture : un octet
            // non nul vient d'ailleurs.
            let etroit =
                |b: u64| u32::try_from(b).map_err(|_| refus("bits d'une globale de 32 bits"));
            globales.push(match t {
                0 => Bits::I32(etroit(b)?.cast_signed()),
                1 => Bits::I64(b.cast_signed()),
                2 => Bits::F32(etroit(b)?),
                3 => Bits::F64(b),
                _ => return Err(refus("étiquette de globale inconnue")),
            });
        }
        if !reste.is_empty() {
            return Err(refus("octets en trop"));
        }
        Ok(Instantane::new(module, pages, lues, gardes, globales))
    }
}

#[cfg(test)]
mod tests {
    //! La forme de l'instantané gardé, figée (vérification du lot 1, constat 3 ; reprise
    //! au lot 2 pour l'instantané hors des zones) : ses octets écrits à la main, et une
    //! seconde lecture qui ne passe pas par [`InstantaneGarde::lire`] (SAL-90). Changer
    //! la forme fait rougir l'un et l'autre, et demande de monter
    //! `partie::persistance::VERSION_GARDE`.

    use alloc::vec;
    use alloc::vec::Vec;

    use partie::persistance::{InstantaneGarde, RefusGarde, VERSION_GARDE};
    use proptest::prelude::*;

    use super::{Bits, Instantane};

    /// Une page de deux plages, de trois et deux octets, et une globale de chaque sorte.
    fn instantane_fige() -> Instantane {
        Instantane::new(
            [3; 32],
            1,
            vec![(16, 3), (65_534, 2)],
            vec![0xaa, 0xbb, 0xcc, 0x55, 0x66],
            vec![
                Bits::I32(-2),
                Bits::I64(-3),
                Bits::F32(0x3f80_0000),
                Bits::F64(0x4000_0000_0000_0000),
            ],
        )
    }

    /// Les octets de [`instantane_fige`], d'après la forme : l'empreinte, les pages en
    /// `u64` petit-boutiste, le nombre de plages en `u32`, chaque plage (adresse et
    /// longueur en `u64`, puis ses octets), le nombre de globales en `u32`, puis
    /// chacune : son étiquette, ses bits en `u64` petit-boutiste (les bits hauts d'un
    /// `i32` ou d'un `f32` à zéro).
    fn octets_figes() -> Vec<u8> {
        [
            &[3u8; 32][..],
            &[1, 0, 0, 0, 0, 0, 0, 0],
            &[2, 0, 0, 0],
            &[16, 0, 0, 0, 0, 0, 0, 0],
            &[3, 0, 0, 0, 0, 0, 0, 0],
            &[0xaa, 0xbb, 0xcc],
            &[0xfe, 0xff, 0, 0, 0, 0, 0, 0],
            &[2, 0, 0, 0, 0, 0, 0, 0],
            &[0x55, 0x66],
            &[4, 0, 0, 0],
            &[0, 0xfe, 0xff, 0xff, 0xff, 0, 0, 0, 0],
            &[1, 0xfd, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
            &[2, 0, 0, 0x80, 0x3f, 0, 0, 0, 0],
            &[3, 0, 0, 0, 0, 0, 0, 0, 0x40],
        ]
        .concat()
    }

    #[test]
    fn les_octets_de_l_instantane_sont_figes() {
        assert_eq!(
            VERSION_GARDE, 2,
            "les octets de ce test sont ceux de la version 2"
        );
        let mut o = Vec::new();
        instantane_fige().ecrire(&mut o);
        assert!(
            o == octets_figes(),
            "l'instantané s'écrit autrement : {o:?}"
        );
        assert!(
            Instantane::lire(&octets_figes()) == Ok(instantane_fige()),
            "les octets figés se relisent autrement"
        );
    }

    fn u64_de(t: &[u8]) -> u64 {
        u64::from_le_bytes(t.try_into().unwrap())
    }

    /// SAL-90 : une lecture d'après la forme seule, sans `lire`.
    #[test]
    fn une_seconde_lecture_relit_l_instantane() {
        let attendu = instantane_fige();
        let mut o = Vec::new();
        attendu.ecrire(&mut o);
        let (empreinte, suite) = o.split_at(32);
        assert_eq!(empreinte, attendu.module());
        let (pages, suite) = suite.split_at(8);
        assert_eq!(u64_de(pages), 1);
        let (nombre, mut suite) = suite.split_at(4);
        assert_eq!(u32::from_le_bytes(nombre.try_into().unwrap()), 2);
        let mut lues = Vec::new();
        let mut gardes = Vec::new();
        for _ in 0..2 {
            let (adresse, apres) = suite.split_at(8);
            let (longueur, apres) = apres.split_at(8);
            let n = usize::try_from(u64_de(longueur)).unwrap();
            let (plage, apres) = apres.split_at(n);
            lues.push((u64_de(adresse), u64_de(longueur)));
            gardes.extend_from_slice(plage);
            suite = apres;
        }
        assert_eq!(lues, attendu.plages());
        assert_eq!(gardes, attendu.octets());
        let (nombre, mut suite) = suite.split_at(4);
        assert_eq!(u32::from_le_bytes(nombre.try_into().unwrap()), 4);
        let mut globales = Vec::new();
        for _ in 0..4 {
            let (globale, apres) = suite.split_at(9);
            suite = apres;
            let bits = u64_de(&globale[1..]);
            globales.push(match globale[0] {
                0 => Bits::I32(i32::from_le_bytes(
                    bits.to_le_bytes()[..4].try_into().unwrap(),
                )),
                1 => Bits::I64(i64::from_le_bytes(bits.to_le_bytes())),
                2 => Bits::F32(u32::try_from(bits).unwrap()),
                3 => Bits::F64(bits),
                etiquette => panic!("étiquette {etiquette}"),
            });
        }
        assert!(suite.is_empty(), "des octets en trop");
        assert_eq!(globales, attendu.globales());
    }

    /// Une relecture stricte : un octet de trop ou de moins, une plage vide, des plages
    /// dans le désordre, une plage au-delà de la mémoire, une étiquette inconnue, des
    /// bits hauts dans un `i32`.
    #[test]
    fn un_instantane_abime_est_refuse() {
        // La longueur de la première plage, après l'empreinte, les pages, le nombre de
        // plages et son adresse ; l'adresse de la seconde ; l'étiquette de la première
        // globale ; le cinquième octet de ses bits.
        const LONGUEUR_1: usize = 52;
        const ADRESSE_2: usize = 63;
        const ETIQUETTE: usize = 85;
        const BIT_HAUT: usize = 90;
        let bon = octets_figes();
        let refuse = |o: &[u8]| matches!(Instantane::lire(o), Err(RefusGarde::Instantane(_)));
        assert!(!refuse(&bon));
        let mut trop = bon.clone();
        trop.push(0);
        assert!(refuse(&trop));
        assert!(refuse(&bon[..bon.len() - 1]));
        let mut vide = bon.clone();
        vide[LONGUEUR_1] = 0;
        assert!(refuse(&vide));
        let mut desordre = bon.clone();
        desordre[ADRESSE_2..ADRESSE_2 + 8].copy_from_slice(&8u64.to_le_bytes());
        assert!(refuse(&desordre));
        let mut dehors = bon.clone();
        dehors[ADRESSE_2..ADRESSE_2 + 8].copy_from_slice(&65_535u64.to_le_bytes());
        assert!(refuse(&dehors));
        let mut pages = bon.clone();
        pages[32] = 0;
        assert!(refuse(&pages));
        let mut etiquette = bon.clone();
        etiquette[ETIQUETTE] = 4;
        assert!(refuse(&etiquette));
        let mut haut = bon.clone();
        haut[BIT_HAUT] = 1;
        assert!(refuse(&haut));
    }

    proptest! {
        /// REM-64 : des octets au hasard, ou les octets figés abîmés, ne font jamais
        /// paniquer la relecture ; ce qu'elle accepte se réécrit à l'identique.
        #[test]
        fn une_relecture_ne_panique_jamais(
            hasard in proptest::collection::vec(any::<u8>(), 0..200),
            position in 0usize..200,
            octet in any::<u8>(),
        ) {
            for o in [hasard, {
                let mut o = octets_figes();
                let k = position.checked_rem(o.len()).unwrap();
                o[k] = octet;
                o
            }] {
                if let Ok(i) = Instantane::lire(&o) {
                    let mut reecrit = Vec::new();
                    i.ecrire(&mut reecrit);
                    prop_assert_eq!(reecrit, o);
                }
            }
        }
    }
}
