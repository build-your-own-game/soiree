//! Les tests de ce qui est scellé dans `jeu` : la forme canonique, l'arène, la zone de
//! panique (INTERFACE.md, § 8.3).

#![expect(
    clippy::arithmetic_side_effects,
    reason = "des bornes de test, petites et choisies : aucune ne déborde"
)]

extern crate std;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use jeu_abi::{Appareil, Illisible as Quoi, Pas};
use proptest::prelude::*;

use crate::cache::{Ecrivain, Liseur};
use crate::colle::{CODE_PANIQUE, Zone, ecrire_panique, lire_panique, reserver};
use crate::prelude::*;

#[jeu::etat]
struct Tout {
    octet: u8,
    signe: i64,
    grand: u128,
    texte: String,
    suite: Vec<Option<u16>>,
    table: BTreeMap<u32, String>,
    flottant: f64,
    secret: Secret<u32>,
    image: Partage<Vec<u8>>,
    appareils: Appareils<u8>,
    paire: (u8, bool),
    tableau: [u16; 3],
    lettre: char,
    pas: Pas,
    rien: (),
    vides: Vec<()>,
    choix: Choix,
}

#[jeu::etat]
enum Choix {
    Rien,
    Un(i8),
    Deux { a: u32, b: Option<Box<Choix>> },
}

fn ecrire<S: Etat>(v: &S) -> (Vec<u8>, u64) {
    let mut o = Vec::new();
    let mut e = Ecrivain::tout(&mut o);
    v.ecrire(&mut e);
    let n = e.non_finis();
    (o, n)
}

fn lire<S: Etat>(o: &[u8]) -> Result<S, crate::cache::Illisible> {
    let mut l = Liseur::new(o);
    let v = S::lire(&mut l)?;
    if l.reste().is_empty() {
        Ok(v)
    } else {
        Err(l.erreur(Quoi::OctetsRestants))
    }
}

fn tout() -> impl Strategy<Value = Tout> {
    (
        (any::<u8>(), any::<i64>(), any::<u128>(), ".{0,12}"),
        prop::collection::vec(any::<Option<u16>>(), 0..6),
        prop::collection::btree_map(any::<u32>(), ".{0,4}", 0..4),
        (
            any::<f64>(),
            any::<u32>(),
            prop::collection::vec(any::<u8>(), 0..16),
        ),
        prop::collection::vec((any::<u64>(), any::<u8>()), 0..4),
        (
            any::<(u8, bool)>(),
            any::<[u16; 3]>(),
            any::<char>(),
            any::<u64>(),
            0usize..5,
        ),
        (any::<i8>(), any::<u32>()),
    )
        .prop_map(
            |(
                (octet, signe, grand, texte),
                suite,
                table,
                (flottant, s, image),
                app,
                divers,
                (i, a),
            )| {
                let mut appareils = Appareils::new();
                for (n, v) in app {
                    let _: Option<u8> = appareils.inserer(Appareil::du_fil(n), v);
                }
                let (paire, tableau, lettre, pas, vides) = divers;
                Tout {
                    octet,
                    signe,
                    grand,
                    texte,
                    suite,
                    table,
                    flottant,
                    secret: Secret::cacher(Qui::Personne, s),
                    image: Partage::new(image),
                    appareils,
                    paire,
                    tableau,
                    lettre,
                    pas: Pas::du_fil(pas),
                    rien: (),
                    vides: vec![(); vides],
                    choix: match a % 3 {
                        0 => Choix::Rien,
                        1 => Choix::Un(i),
                        _ => Choix::Deux {
                            a,
                            b: Some(Box::new(Choix::Un(i))),
                        },
                    },
                }
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    /// Écrire puis relire rend un état qui s'écrit aux mêmes octets (§ 8.3, D24).
    #[test]
    fn forme_canonique_aller_retour(t in tout()) {
        let (octets, _) = ecrire(&t);
        let relu: Tout = lire(&octets).unwrap();
        prop_assert_eq!(ecrire(&relu).0, octets);
    }

    /// Une fenêtre de la forme canonique est la tranche correspondante du flux entier :
    /// les morceaux de `Appel::Canonique` se recollent.
    #[test]
    fn fenetre_est_une_tranche(t in tout(), debut in 0usize..200, longueur in 0usize..200) {
        let (entier, _) = ecrire(&t);
        let mut o = Vec::new();
        let mut e = Ecrivain::fenetre(&mut o, debut, debut + longueur);
        t.ecrire(&mut e);
        let attendu = entier.get(debut.min(entier.len())..(debut + longueur).min(entier.len())).unwrap();
        prop_assert_eq!(e.position(), entier.len());
        prop_assert_eq!(o.as_slice(), attendu);
    }

    /// Des octets abîmés ne paniquent pas la lecture : elle rend une erreur, ou un état.
    #[test]
    fn octets_abimes_sans_panique(t in tout(), i in any::<prop::sample::Index>(), b in any::<u8>()) {
        let (mut octets, _) = ecrire(&t);
        if !octets.is_empty() {
            let i = i.index(octets.len());
            octets[i] = b;
        }
        let _: Result<Tout, _> = lire::<Tout>(&octets);
        octets.truncate(octets.len() / 2);
        prop_assert!(lire::<Tout>(&octets).is_err() || octets.is_empty());
    }
}

/// Une suite d'éléments de taille nulle qui annonce 2⁶³ éléments est refusée tout de
/// suite, au lieu de tourner sans fin (REM-58) : `octets_abimes_sans_panique` l'a trouvée
/// en abîmant la longueur de `Tout::vides`.
#[test]
fn suite_de_vides_absurde_refusee() {
    let mut octets = vec![0xff; 9];
    octets.push(0x7f);
    assert!(lire::<Vec<()>>(&octets).is_err());
    let (octets, _) = ecrire(&vec![(); crate::forme::SUITE_DE_VIDES_MAX]);
    assert_eq!(
        lire::<Vec<()>>(&octets).unwrap().len(),
        crate::forme::SUITE_DE_VIDES_MAX
    );
}

/// Un `Partage` s'écrit une fois par allocation, et se relit partagé (TL-20).
#[test]
fn partage_ecrit_une_fois() {
    let image = Partage::new(vec![7u8; 10_000]);
    let historique = vec![image.clone(), image.clone(), image];
    let (octets, _) = ecrire(&historique);
    assert!(octets.len() < 10_100, "{}", octets.len());
    let relu: Vec<Partage<Vec<u8>>> = lire(&octets).unwrap();
    assert!(core::ptr::eq(relu[0].get(), relu[2].get()));
    assert_eq!(relu[1].get().len(), 10_000);
}

/// Besoin 16 des jeux (TL-18) : `Partage::meme` dit qu'un contenu est partagé sans le
/// lire, et le partage survit à la forme canonique.
#[test]
fn partage_meme_survit_a_la_forme_canonique() {
    let image = Partage::new(vec![7u8; 100]);
    let autre = Partage::new(vec![7u8; 100]);
    assert_eq!(Partage::meme(&image, &image.clone()), Oui::Oui);
    assert_eq!(
        Partage::meme(&image, &autre),
        Oui::Non,
        "même contenu, autre allocation"
    );
    let (octets, _) = ecrire(&vec![image.clone(), autre, image]);
    let relu: Vec<Partage<Vec<u8>>> = lire(&octets).unwrap();
    assert_eq!(Partage::meme(&relu[0], &relu[2]), Oui::Oui);
    assert_eq!(Partage::meme(&relu[0], &relu[1]), Oui::Non);
}

/// Besoin 15 des jeux (TL-22) : des octets s'écrivent et se lisent d'un bloc, avec les
/// mêmes octets qu'un par un : la longueur en varint, puis les octets.
#[test]
fn octets_en_bloc_meme_forme() {
    let v: Vec<u8> = (0..=255u8).chain(0..44).collect();
    let (octets, _) = ecrire(&v);
    let mut attendu = vec![0xac, 0x02];
    attendu.extend_from_slice(&v);
    assert_eq!(octets, attendu);
    assert_eq!(lire::<Vec<u8>>(&octets).unwrap(), v);
    // Une longueur qui annonce plus qu'il ne reste échoue, sans réserver (REM-58).
    assert!(lire::<Vec<u8>>(&[0xff, 0xff, 0x03, 1, 2]).is_err());
    // Une suite de suites d'octets aussi.
    let vv = vec![vec![1u8, 2], Vec::new(), vec![3]];
    let (octets, _) = ecrire(&vv);
    assert_eq!(octets, [3, 2, 1, 2, 0, 1, 3]);
    assert_eq!(lire::<Vec<Vec<u8>>>(&octets).unwrap(), vv);
}

/// Un renvoi à un `Partage` d'un autre type est refusé, pas relu de travers.
#[test]
fn partage_d_un_autre_type_refuse() {
    let (mut octets, _) = ecrire(&(Partage::new(5u8), Partage::new(6u8)));
    // Le second devient un renvoi au premier, mais relu comme `Partage<u16>`.
    octets.truncate(2);
    octets.extend([1, 0]);
    assert!(lire::<(Partage<u8>, Partage<u16>)>(&octets).is_err());
}

/// La forme canonique compte les flottants non finis (CAT-05, SQ-68).
#[test]
fn non_finis_comptes() {
    let (_, n) = ecrire(&vec![1.0f64, f64::NAN, f64::INFINITY, 2.0]);
    assert_eq!(n, 2);
    let (_, n) = ecrire(&(f32::NEG_INFINITY, 0.5f32));
    assert_eq!(n, 1);
}

/// Une longueur absurde échoue sans réserver (REM-58) ; un octet restant est refusé
/// (REM-59).
#[test]
fn lecture_stricte() {
    let mut o = Vec::new();
    let mut e = Ecrivain::tout(&mut o);
    e.varint(u128::from(u64::MAX));
    assert!(lire::<Vec<u32>>(&o).is_err());
    let (mut octets, _) = ecrire(&3u32);
    octets.push(0);
    assert_eq!(lire::<u32>(&octets).unwrap_err().quoi, Quoi::OctetsRestants);
    assert_eq!(lire::<bool>(&[2]).unwrap_err().quoi, Quoi::Booleen);
    // Un varint trop long pour un u16.
    assert!(lire::<u16>(&[0xff, 0xff, 0x7f]).is_err());
    // Une table d'appareils en double est une forme qu'aucune table n'écrit.
    let (o, _) = ecrire(&vec![
        (Appareil::du_fil(1), 1u8),
        (Appareil::du_fil(1), 2u8),
    ]);
    assert!(lire::<Appareils<u8>>(&o).is_err());
}

/// La projection d'un `Partage` sans secret garde le partage.
#[test]
fn projection_garde_le_partage() {
    let p = Partage::new(vec![1u8, 2, 3]);
    let r = Regard::du_fil(jeu_abi::RegardFil::Ecran);
    assert!(core::ptr::eq(p.projeter(&r).get(), p.get()));
}

/// L'arène du brouillon : alignée, bornée, sans débordement.
#[test]
fn arene_reserve_aligne_et_borne() {
    assert_eq!(reserver(1000, 2000, 1000, 10, 8), Some((1000, 1010)));
    assert_eq!(reserver(1000, 2000, 1010, 4, 8), Some((1016, 1020)));
    assert_eq!(reserver(1000, 2000, 1990, 11, 1), None);
    assert_eq!(reserver(1000, 2000, 1990, 10, 1), Some((1990, 2000)));
    assert_eq!(reserver(1000, 2000, 1000, usize::MAX, 1), None);
    assert_eq!(reserver(1000, 2000, usize::MAX - 2, 1, 16), None);
    assert_eq!(reserver(1000, 2000, 1000, 1, 3), None);
}

proptest! {
    #[test]
    fn arene_jamais_hors_de_sa_zone(debut in 0usize..1 << 20, n in 0usize..1 << 16, haut in 0usize..1 << 21, taille in 0usize..1 << 17, puissance in 0u32..12) {
        let fin = debut + n;
        let al = 1usize << puissance;
        if let Some((a, b)) = reserver(debut, fin, haut.max(debut), taille, al) {
            prop_assert!(debut <= a && a <= b && b <= fin);
            prop_assert_eq!(a % al, 0);
            prop_assert_eq!(b - a, taille);
            prop_assert!(a >= haut.max(debut));
        }
    }
}

/// La zone de panique : code, longueur, message coupé entre deux caractères.
#[test]
fn zone_de_panique() {
    let z: Zone<16> = Zone::new();
    assert_eq!(lire_panique(&z.lire(0, 16)), None);
    ecrire_panique(&z, &"éàé");
    let octets = z.lire(0, 16);
    assert_eq!(
        u32::from_le_bytes(octets[..4].try_into().unwrap()),
        CODE_PANIQUE
    );
    assert_eq!(lire_panique(&octets), Some("éàé"));
    ecrire_panique(&z, &"ééééé");
    // 8 octets de place : quatre « é », jamais un demi.
    assert_eq!(lire_panique(&z.lire(0, 16)), Some("éééé"));
    crate::colle::effacer_panique(&z);
    assert_eq!(lire_panique(&z.lire(0, 16)), None);
}
