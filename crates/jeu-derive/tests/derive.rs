//! Les tests des macros (INTERFACE.md, § 8.2) : ce qui compile, ce qui ne compile pas,
//! l'empreinte de forme et la projection.

use jeu::prelude::*;
use jeu::{Contient, Etat, Forme};
use jeu_abi::RegardFil;

/// Ce qui doit compiler, et ce qui ne doit pas : un secret dans un message, une cellule
/// dans un état, chaque attribut `serde` refusé (REM-60, PUN-91, SAL-119), `Variantes`
/// sur un variant à champs.
#[test]
fn ce_qui_compile_et_ce_qui_ne_compile_pas() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/passe/*.rs");
    t.compile_fail("tests/ui/echoue/*.rs");
}

/// FNV-1a 64 écrit à nouveau, sans `jeu` : la seconde définition de l'algorithme
/// d'empreinte, telle que la documentation de `jeu-derive` la décrit.
fn fnv(h: u64, octets: &[u8]) -> u64 {
    let mut h = h;
    for b in octets {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn mot(h: u64, m: &str) -> u64 {
    fnv(fnv(h, &(m.len() as u64).to_le_bytes()), m.as_bytes())
}

fn entier(h: u64, n: u64) -> u64 {
    fnv(h, &n.to_le_bytes())
}

fn depart(sorte: &str) -> u64 {
    mot(fnv(0xcbf2_9ce4_8422_2325, b"jeu::Forme, version 1"), sorte)
}

fn primitive(nom: &str) -> u64 {
    mot(depart("commun"), nom)
}

mod un {
    #[jeu::message]
    pub struct Point {
        pub x: i32,
        pub y: i32,
    }

    #[jeu::message]
    pub enum Sens {
        Haut,
        Bas,
    }
}

mod deux {
    #[jeu::message]
    pub struct Point {
        pub x: i32,
        pub y: i32,
        pub z: i32,
    }

    #[jeu::message]
    pub enum Sens {
        Bas,
        Haut,
    }
}

mod trois {
    #[jeu::message]
    pub struct Point {
        pub y: i32,
        pub x: i32,
    }
}

#[test]
fn empreinte_change_avec_un_champ() {
    assert_ne!(
        <un::Point as Forme>::EMPREINTE,
        <deux::Point as Forme>::EMPREINTE
    );
    assert_ne!(
        <un::Point as Forme>::EMPREINTE,
        <trois::Point as Forme>::EMPREINTE
    );
}

#[test]
fn empreinte_change_avec_l_ordre_des_variants() {
    assert_ne!(
        <un::Sens as Forme>::EMPREINTE,
        <deux::Sens as Forme>::EMPREINTE
    );
}

/// L'empreinte est une constante de la compilation, sans chemin de module ni machine :
/// recalculée ici à la main selon l'algorithme documenté, et écrite en dur. Un
/// changement de l'algorithme rougit ce test (PUN-90, pour les formes).
#[test]
fn empreinte_stable_entre_deux_compilations() {
    let i32_ = primitive("i32");
    let h = mot(depart("message struct"), "Point");
    let h = entier(h, 2);
    let h = entier(mot(h, "x"), i32_);
    let attendue = entier(mot(h, "y"), i32_);
    assert_eq!(<un::Point as Forme>::EMPREINTE, attendue);
    assert_eq!(<un::Point as Forme>::EMPREINTE, 0x6c8f_1279_d567_26d9);
}

#[jeu::etat]
pub struct Table {
    pub public: u32,
    pub cache: Secret<u32>,
}

#[test]
fn un_etat_dit_s_il_contient_un_secret() {
    assert_eq!(<Table as Forme>::SECRET, Contient::Oui);
    assert_eq!(<un::Point as Forme>::SECRET, Contient::Non);
    assert_eq!(<Aucun as Forme>::VIDE, Contient::Oui);
    assert_eq!(<Table as Forme>::VIDE, Contient::Non);
}

/// Chaque `Qui`, devant chaque regard : la projection vide ce que le regard ne voit pas,
/// et `voir` s'accorde avec elle (§ 2.8, CAT-46).
#[test]
fn projection_vide_ce_que_le_regard_ne_voit_pas() {
    let onze = Appareil::du_fil(11);
    let vingt_deux = Appareil::du_fil(22);
    let lecteurs = Lecteurs::de(&[onze, vingt_deux]).unwrap();
    let ecran = jeu::essai::regard(RegardFil::Ecran);
    let ra = jeu::essai::regard(RegardFil::Appareil(11));
    let rc = jeu::essai::regard(RegardFil::Appareil(33));
    // (qui, voit l'écran, voit a, voit c)
    let table = [
        (Qui::Personne, false, false, false),
        (Qui::Appareil(onze), false, true, false),
        (Qui::Appareils(lecteurs), false, true, false),
        (Qui::Ecran, true, false, false),
        (Qui::Tous, true, true, true),
    ];
    for (qui, voit_ecran, voit_a, voit_c) in table {
        let t = Table {
            public: 7,
            cache: Secret::cacher(qui, 42),
        };
        for (regard, voit) in [(&ecran, voit_ecran), (&ra, voit_a), (&rc, voit_c)] {
            let p = t.projeter(regard);
            assert_eq!(p.public, 7, "{qui:?}");
            assert_eq!(p.cache.qui(), &qui, "qui est public");
            assert_eq!(
                p.cache.voir(regard).copied(),
                voit.then_some(42),
                "{qui:?} {regard:?}"
            );
            assert_eq!(
                t.cache.voir(regard).copied(),
                voit.then_some(42),
                "{qui:?} {regard:?}"
            );
            // Ce que la projection n'a pas reçu, aucune porte ne l'ouvre.
            let autres = [&ecran, &ra, &rc];
            for autre in autres {
                if !voit {
                    assert_eq!(p.cache.voir(autre), None);
                }
            }
        }
    }
}

/// Le `Debug` d'un état n'écrit jamais la valeur d'un secret.
#[test]
fn debug_d_un_secret_le_cache() {
    let t = Table {
        public: 7,
        cache: Secret::cacher(Qui::Personne, 424_242),
    };
    let texte = format!("{t:?}");
    assert!(texte.contains("<secret>"), "{texte}");
    assert!(!texte.contains("424242"), "{texte}");
}
