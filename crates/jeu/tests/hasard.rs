//! Le hasard (INTERFACE.md, § 2.7 et § 8.3 ; CAT-89 à CAT-91, CAT-118). Les tests
//! statistiques ont leur graine fixée : ils ne clignotent pas (CAT-118).

use core::num::{NonZeroU32, NonZeroU64};

use jeu::Hasard;
use proptest::prelude::*;

/// Les dix premiers tirages de xoshiro256** depuis l'état `[1, 2, 3, 4]`, tels que les
/// donne l'implémentation de référence de Blackman et Vigna (`xoshiro256starstar.c`),
/// recopiés des tests de `rand_xoshiro`. Les deux premiers se vérifient à la main :
/// `rotl(2 × 5, 7) × 9 = 11 520`, puis `s1` vaut 0.
#[test]
fn hasard_vecteurs_de_reference() {
    let mut h = Hasard::depuis_etat([1, 2, 3, 4]);
    let attendus: [u64; 10] = [
        11_520,
        0,
        1_509_978_240,
        1_215_971_899_390_074_240,
        1_216_172_134_540_287_360,
        607_988_272_756_665_600,
        16_172_922_978_634_559_625,
        8_476_171_486_693_032_832,
        10_595_114_339_597_558_777,
        2_904_607_092_377_533_576,
    ];
    for a in attendus {
        assert_eq!(h.entier(), a);
    }
}

/// Un tirage dans `[0, 3)` sur 60 000 essais : chaque valeur à moins de 3 % de 20 000.
/// Puis une borne de `3 × 2⁶²`, où un modulo naïf donnerait la moitié basse dans 62,5 %
/// des tirages : la méthode de Lemire, qui rejette, l'équilibre (CAT-91).
#[test]
fn borne_sans_biais() {
    let mut h = Hasard::depuis_etat([0x1234, 0x5678, 0x9abc, 0xdef0]);
    let n = NonZeroU64::new(3).unwrap();
    let mut comptes = [0u32; 3];
    for _ in 0..60_000 {
        comptes[usize::try_from(h.borne(n)).unwrap()] += 1;
    }
    for c in comptes {
        assert!((19_400..=20_600).contains(&c), "{comptes:?}");
    }
    let grand = NonZeroU64::new(3 << 62).unwrap();
    let moitie = (3u64 << 62) / 2;
    let bas = (0..60_000).filter(|_| h.borne(grand) < moitie).count();
    assert!((29_000..=31_000).contains(&bas), "{bas}");
}

/// Chaque ordre de trois éléments avec la même probabilité, chaque élément à chaque place
/// (CAT-89, CAT-90).
#[test]
fn melanger_uniforme() {
    let mut h = Hasard::depuis_etat([7, 8, 9, 10]);
    let mut comptes = std::collections::BTreeMap::new();
    for _ in 0..60_000 {
        let mut v = [0u8, 1, 2];
        h.melanger(&mut v);
        *comptes.entry(v).or_insert(0u32) += 1;
    }
    assert_eq!(comptes.len(), 6);
    for c in comptes.values() {
        assert!((9_600..=10_400).contains(c), "{comptes:?}");
    }
}

proptest! {
    #[test]
    fn dans_reste_dans_ses_bornes(etat in any::<[u64; 4]>()) {
        let mut h = Hasard::depuis_etat(etat);
        for _ in 0..32 {
            let v = h.dans::<-3, 9>().get();
            prop_assert!((-3..=9).contains(&v));
            let _: jeu_abi::Borne<{ i64::MIN }, { i64::MAX }> = h.dans();
            prop_assert!(h.borne(NonZeroU64::new(7).unwrap()) < 7);
        }
    }

    #[test]
    fn choisir_et_une_sur(etat in any::<[u64; 4]>()) {
        let mut h = Hasard::depuis_etat(etat);
        prop_assert!(h.choisir::<u8>(&[]).is_none());
        let v = [1, 2, 3];
        prop_assert!(v.contains(h.choisir(&v).unwrap()));
        prop_assert_eq!(h.une_sur(NonZeroU32::new(1).unwrap()), jeu::prelude::Oui::Oui);
    }

    /// Le même état donne la même suite ; un générateur dérivé ne répète pas son parent.
    #[test]
    fn meme_graine_meme_suite(etat in any::<[u64; 4]>()) {
        let mut a = Hasard::depuis_etat(etat);
        let mut b = Hasard::depuis_etat(etat);
        let sa: Vec<u64> = (0..16).map(|_| a.entier()).collect();
        let sb: Vec<u64> = (0..16).map(|_| b.entier()).collect();
        prop_assert_eq!(&sa, &sb);
        let mut d = a.deriver();
        let sd: Vec<u64> = (0..16).map(|_| d.entier()).collect();
        let sa2: Vec<u64> = (0..16).map(|_| a.entier()).collect();
        prop_assert_ne!(sd, sa2);
    }
}
