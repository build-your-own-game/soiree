//! La garde (PHASE-3, § 2.4, lot 4b) : la chaîne des cases, ce qu'elle refuse, et la
//! forme figée de ce que portent les cases, avec sa seconde lecture (SAL-90).
//!
//! `cfg(test)` marque tout ce binaire comme du code de test : les fonctions d'aide y
//! ont droit à `unwrap`, `expect`, `panic!` et aux index (CONTRAT, « Les règles
//! d'écriture »).

#![cfg(test)]
#![expect(
    clippy::arithmetic_side_effects,
    reason = "code de test : des instants et des numéros petits ; un débordement y serait l'échec du test"
)]

use jeu_abi::{Appareil, Oui, Pas};
use liaison::{Case, CodeSalle};
use partie::modele::{FabriqueModele, InstantaneModele};
use proptest::prelude::*;
use salle::chiffre::CleDeGarde;
use salle::garde::{
    ContenuJournal, ContenuPoint, Inscrit, Marque, OngletInscrit, RefusContenu, RefusDeGarde,
    Registre, VERSION_CASES, decoder_contenu_journal, decoder_contenu_point,
    encoder_contenu_journal, encoder_contenu_point, numero, relire,
};
use salle::simulation::{Pannes, Soiree};

type SoireeModele = Soiree<FabriqueModele, fn() -> FabriqueModele>;

/// Une soirée de 25 s sur le modèle, deux téléphones : trois points écrits, la garde
/// tient les deux derniers.
fn une_garde() -> (SoireeModele, Vec<(Case, Vec<u8>)>) {
    let mut s: SoireeModele = Soiree::new(
        FabriqueModele::default as fn() -> FabriqueModele,
        3,
        Oui::Oui,
        Pannes::AUCUNE,
    );
    for _ in 0..2 {
        let _: usize = s.ajouter_telephone();
    }
    let fin = s.t_us + 25_000_000;
    s.avancer_jusqua(fin);
    let cases = s.relais.cases();
    (s, cases)
}

fn relue(
    s: &SoireeModele,
    cases: &[(Case, Vec<u8>)],
    dernier: Option<u64>,
) -> Result<salle::garde::GardeLue<InstantaneModele>, RefusDeGarde> {
    relire::<InstantaneModele>(&s.cle_de_garde(), s.salle(), cases, dernier)
}

fn dernier(cases: &[(Case, Vec<u8>)]) -> Option<u64> {
    cases.iter().map(|(c, _)| numero(*c)).max()
}

#[test]
fn la_garde_tient_au_plus_deux_points_et_se_relit() {
    let (s, cases) = une_garde();
    let points = cases
        .iter()
        .filter(|(c, _)| matches!(c, Case::Point(_)))
        .count();
    assert!(
        s.relais
            .historique()
            .iter()
            .filter(|(c, _)| matches!(c, Case::Point(_)))
            .count()
            >= 3
    );
    assert_eq!(
        points, 2,
        "les points d'avant le point d'avant sont effacés"
    );
    assert!(
        matches!(cases[0].0, Case::Point(_)),
        "la garde commence à un point"
    );
    let l = relue(&s, &cases, dernier(&cases)).expect("la garde se relit");
    assert_eq!(l.suite.prochain, dernier(&cases).unwrap() + 1);
    assert_eq!(l.journal.base(), l.point.indice());
    assert_eq!(l.registre.appareils.len(), 2);
    let r = partie::Partie::reprendre(FabriqueModele::default(), l.ouverture, &l.point, &l.journal);
    assert!(r.is_ok(), "{:?}", r.err());
}

/// Le relais peut retenir les dernières cases : l'autorité, qui sait le dernier numéro
/// écrit, refuse une garde qui s'arrête avant (PHASE-3, § 2.4).
#[test]
fn une_garde_plus_vieille_que_le_dernier_n_est_refusee() {
    let (s, mut cases) = une_garde();
    let n = dernier(&cases);
    let _: Option<(Case, Vec<u8>)> = cases.pop();
    assert_eq!(
        relue(&s, &cases, n).unwrap_err(),
        RefusDeGarde::PlusVieille {
            lue: n.map(|n| n - 1),
            connue: n.unwrap()
        }
    );
    assert!(matches!(
        relue(&s, &[], n).unwrap_err(),
        RefusDeGarde::PlusVieille { lue: None, .. }
    ));
    assert_eq!(relue(&s, &[], None).unwrap_err(), RefusDeGarde::Vide);
}

/// Le relais ne peut pas intervertir deux cases, ni en changer la sorte, ni en retirer
/// une du milieu, ni en abîmer une, ni servir celles d'une autre salle (§ 2.4).
#[test]
fn une_case_de_garde_intervertie_dans_la_chaine_est_refusee() {
    let (soiree, cases) = une_garde();
    let plus_haut = dernier(&cases);
    let longueur = cases.len();
    // Deux cases de journal interverties, chacune sous le numéro de l'autre.
    let mut v = cases.clone();
    let (avant, apres) = (longueur - 2, longueur - 1);
    assert!(matches!(
        (v[avant].0, v[apres].0),
        (Case::Journal(_), Case::Journal(_))
    ));
    let tmp = v[avant].1.clone();
    v[avant].1 = v[apres].1.clone();
    v[apres].1 = tmp;
    assert!(matches!(
        relue(&soiree, &v, plus_haut),
        Err(RefusDeGarde::Scellee(_))
    ));
    // Une case de journal nommée point.
    let mut v = cases.clone();
    v[longueur - 1].0 = Case::Point(numero(v[longueur - 1].0));
    assert!(matches!(
        relue(&soiree, &v, plus_haut),
        Err(RefusDeGarde::Scellee(_))
    ));
    // Une case du milieu retirée.
    let mut v = cases.clone();
    let _: (Case, Vec<u8>) = v.remove(longueur - 2);
    assert!(matches!(
        relue(&soiree, &v, plus_haut),
        Err(RefusDeGarde::Trouee { .. })
    ));
    // Une case abîmée.
    let mut v = cases.clone();
    let abimee = &mut v[longueur - 1].1;
    let milieu = abimee.len() / 2;
    abimee[milieu] ^= 1;
    assert_eq!(
        relue(&soiree, &v, plus_haut).unwrap_err(),
        RefusDeGarde::Scellee(cases[longueur - 1].0)
    );
    // La case qui précède le dernier point, changée : le point ne s'ouvre plus.
    let point = cases
        .iter()
        .rposition(|(c, _)| matches!(c, Case::Point(_)))
        .unwrap();
    let mut v = cases.clone();
    v[point - 1].1[0] ^= 1;
    assert_eq!(
        relue(&soiree, &v, plus_haut).unwrap_err(),
        RefusDeGarde::Scellee(cases[point].0)
    );
    // Une autre salle, une autre clé.
    assert!(
        relire::<InstantaneModele>(&soiree.cle_de_garde(), CodeSalle([0; 4]), &cases, plus_haut)
            .is_err()
    );
    assert!(
        relire::<InstantaneModele>(
            &CleDeGarde::depuis_octets([9; 32]),
            soiree.salle(),
            &cases,
            plus_haut
        )
        .is_err()
    );
    // Deux fois le même numéro.
    let mut v = cases.clone();
    v.push(cases[longueur - 1].clone());
    assert!(matches!(
        relue(&soiree, &v, plus_haut),
        Err(RefusDeGarde::Doublon(_))
    ));
}

// ---------------------------------------------------------------------------
// La forme figée de ce que portent les cases
// ---------------------------------------------------------------------------

fn registre() -> Registre {
    Registre {
        appareils: vec![
            Inscrit {
                jeton: [0x11; 32],
                appareil: Appareil::du_fil(300),
                onglets: vec![
                    OngletInscrit {
                        onglet: 2,
                        dernier_geste: 5,
                    },
                    OngletInscrit {
                        onglet: 1 << 40,
                        dernier_geste: 0,
                    },
                ],
            },
            Inscrit {
                jeton: [0x22; 32],
                appareil: Appareil::du_fil(1),
                onglets: Vec::new(),
            },
        ],
        marques: vec![Marque {
            indice: 600,
            pas: Pas::du_fil(601),
        }],
        ecartes: vec![0xabc],
    }
}

const POINT_FIGE: &str = "020000000003010203020708021111111111111111111111111111111111111111111111111111111111111111ac02020205808080808020002222222222222222222222222222222222222222222222222222222222222222010001d804d90401bc15";
const JOURNAL_FIGE: &str = "020000000103040506000000";

fn hexa(o: &[u8]) -> String {
    use std::fmt::Write;
    o.iter().fold(String::new(), |mut s, b| {
        let _: std::fmt::Result = write!(s, "{b:02x}");
        s
    })
}

fn depuis_hexa(t: &str) -> Vec<u8> {
    let t: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    t.as_bytes()
        .chunks(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}

fn point() -> ContenuPoint {
    ContenuPoint {
        ouverture: vec![1, 2, 3],
        point: vec![7, 8],
        registre: registre(),
    }
}

fn journal() -> ContenuJournal {
    ContenuJournal {
        lignes: vec![4, 5, 6],
        registre: Registre::default(),
    }
}

#[test]
fn la_version_des_cases_est_figee() {
    assert_eq!(VERSION_CASES, 2);
}

#[test]
fn les_octets_des_contenus_de_case_sont_figes() {
    let p = encoder_contenu_point(&point());
    assert_eq!(hexa(&p), POINT_FIGE);
    assert_eq!(decoder_contenu_point(&p).unwrap(), point());
    let j = encoder_contenu_journal(&journal());
    assert_eq!(hexa(&j), JOURNAL_FIGE);
    assert_eq!(decoder_contenu_journal(&j).unwrap(), journal());
    // Chaque contenu sous l'autre sorte, ou d'une autre version, est refusé.
    assert_eq!(
        decoder_contenu_journal(&p).unwrap_err(),
        RefusContenu::Sorte { lue: 0 }
    );
    let mut v = j.clone();
    v[0] = 3;
    assert_eq!(
        decoder_contenu_journal(&v).unwrap_err(),
        RefusContenu::Version { lue: 3 }
    );
    assert_eq!(
        decoder_contenu_point(&[1, 0]).unwrap_err(),
        RefusContenu::Court
    );
}

/// SAL-118 : un octet de trop, ou un de moins, est un refus.
#[test]
fn un_contenu_fige_allonge_ou_raccourci_est_refuse_sal118() {
    for h in [POINT_FIGE, JOURNAL_FIGE] {
        let mut o = depuis_hexa(h);
        o.push(0);
        assert!(decoder_contenu_point(&o).is_err() && decoder_contenu_journal(&o).is_err());
        o.truncate(o.len() - 2);
        assert!(decoder_contenu_point(&o).is_err() && decoder_contenu_journal(&o).is_err());
    }
}

// La seconde lecture, sans serde ni postcard (SAL-90).

struct Lecteur<'a>(&'a [u8]);

impl Lecteur<'_> {
    fn octet(&mut self) -> u8 {
        let (b, reste) = self.0.split_first().expect("fin des octets");
        self.0 = reste;
        *b
    }

    fn varint(&mut self) -> u64 {
        let mut v = 0u64;
        for decalage in (0..70).step_by(7) {
            let b = self.octet();
            v |= u64::from(b & 0x7f) << decalage;
            if b & 0x80 == 0 {
                return v;
            }
        }
        panic!("varint trop long");
    }

    fn brut<const N: usize>(&mut self) -> [u8; N] {
        let (t, reste) = self.0.split_at(N);
        self.0 = reste;
        t.try_into().unwrap()
    }

    fn octets(&mut self) -> Vec<u8> {
        let n = usize::try_from(self.varint()).unwrap();
        let (t, reste) = self.0.split_at(n);
        self.0 = reste;
        t.to_vec()
    }

    fn registre(&mut self) -> Registre {
        let n = self.varint();
        let appareils = (0..n)
            .map(|_| {
                let jeton = self.brut::<32>();
                let appareil = Appareil::du_fil(self.varint());
                let m = self.varint();
                let onglets = (0..m)
                    .map(|_| OngletInscrit {
                        onglet: self.varint(),
                        dernier_geste: self.varint(),
                    })
                    .collect();
                Inscrit {
                    jeton,
                    appareil,
                    onglets,
                }
            })
            .collect();
        let k = self.varint();
        let marques = (0..k)
            .map(|_| Marque {
                indice: self.varint(),
                pas: Pas::du_fil(self.varint()),
            })
            .collect();
        let e = self.varint();
        let ecartes = (0..e).map(|_| self.varint()).collect();
        Registre {
            appareils,
            marques,
            ecartes,
        }
    }

    fn entete(&mut self, sorte: u8) {
        assert_eq!(u32::from_le_bytes(self.brut::<4>()), VERSION_CASES);
        assert_eq!(self.octet(), sorte);
    }
}

#[test]
fn une_seconde_lecture_relit_chaque_contenu_sal90() {
    let o = depuis_hexa(POINT_FIGE);
    let mut l = Lecteur(&o);
    l.entete(0);
    let p = ContenuPoint {
        ouverture: l.octets(),
        point: l.octets(),
        registre: l.registre(),
    };
    assert_eq!(l.0, &[] as &[u8]);
    assert_eq!(p, point());
    let o = depuis_hexa(JOURNAL_FIGE);
    let mut l = Lecteur(&o);
    l.entete(1);
    let j = ContenuJournal {
        lignes: l.octets(),
        registre: l.registre(),
    };
    assert_eq!(l.0, &[] as &[u8]);
    assert_eq!(j, journal());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    /// REM-64 : des octets au hasard ne font jamais paniquer les décodeurs des contenus ;
    /// ce qui se décode se réencode à l'identique.
    #[test]
    fn des_octets_au_hasard_ne_font_jamais_paniquer_les_contenus_rem64(
        o in proptest::collection::vec(any::<u8>(), 0..200),
        sorte in 0u8..2,
    ) {
        let mut v = VERSION_CASES.to_le_bytes().to_vec();
        v.push(sorte);
        v.extend_from_slice(&o);
        for x in [&o, &v] {
            if let Ok(p) = decoder_contenu_point(x) {
                prop_assert_eq!(&encoder_contenu_point(&p), x);
            }
            if let Ok(j) = decoder_contenu_journal(x) {
                prop_assert_eq!(&encoder_contenu_journal(&j), x);
            }
        }
    }
}
