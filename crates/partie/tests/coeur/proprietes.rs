//! Les propriétés de la partie qui fait foi, sur le modèle (§ 4 ; CONTRAT, « Ce que
//! le réel impose ») : des soirées tirées au hasard, avec des fautes, des états
//! tenus périmés et des reprises depuis des points quelconques.

use jeu_abi::{EntreeFil, decoder};
use partie::modele::{EtatModele, FabriqueModele, GesteModele, ReglageModele};
use partie::{EtatCoeur, IssueJournal, Journal, Partie, Placement, PointDeReprise};
use proptest::prelude::*;

use crate::commun::*;

/// Un geste du scénario : ce qu'il fait au jeu.
#[derive(Debug, Clone, Copy)]
enum Quoi {
    Ajouter(u8),
    Refuser,
    Paniquer,
    Boucler,
}

impl Quoi {
    fn geste(self) -> GesteModele {
        match self {
            Quoi::Ajouter(n) => GesteModele::Ajouter(n),
            Quoi::Refuser => GesteModele::Refuser,
            Quoi::Paniquer => GesteModele::Paniquer,
            Quoi::Boucler => GesteModele::Boucler,
        }
    }
}

/// Une action du scénario, pendant un pas.
#[derive(Debug, Clone, Copy)]
enum Action {
    /// Un geste de l'appareil `de`, fait `retard` pas avant sa réception.
    Geste { de: u64, quoi: Quoi, retard: u64 },
    /// Un tenu ; `perime` : un état plus ancien arrive aussi, après lui (REM-14).
    Tenu {
        de: u64,
        cle: u8,
        valeur: u8,
        perime: bool,
    },
    /// Un point de reprise, pris par le commun.
    Point,
    /// L'autorité s'arrête après ce pas, et reprend depuis le point `k` modulo leur nombre.
    Panne { k: usize },
}

fn action() -> impl Strategy<Value = Action> {
    let quoi = prop_oneof![
        6 => (1u8..5).prop_map(Quoi::Ajouter),
        1 => Just(Quoi::Refuser),
        1 => Just(Quoi::Paniquer),
        1 => Just(Quoi::Boucler),
    ];
    prop_oneof![
        6 => (1u64..4, quoi, 0u64..3).prop_map(|(de, quoi, retard)| Action::Geste { de, quoi, retard }),
        4 => (1u64..4, 0u8..2, 0u8..4, any::<bool>()).prop_map(|(de, cle, valeur, perime)| Action::Tenu { de, cle, valeur, perime }),
        1 => Just(Action::Point),
        1 => any::<usize>().prop_map(|k| Action::Panne { k }),
    ]
}

fn scenario() -> impl Strategy<Value = (Vec<Vec<Action>>, u16, u64)> {
    (
        proptest::collection::vec(proptest::collection::vec(action(), 0..5), 1..40),
        1u16..70,
        2u64..12,
    )
}

/// L'autorité s'arrête-t-elle quand le scénario le dit ?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pannes {
    Sans,
    Avec,
}

/// Ce qu'une soirée a produit.
struct Bilan {
    journal: Journal,
    empreinte: [u8; 32],
    coeur: EtatCoeur,
    etat: EtatModele,
    points: Vec<PointDeReprise<partie::modele::InstantaneModele>>,
    /// Les gestes envoyés : identité, appareil, ce qu'il fait, dans l'ordre d'envoi.
    envoyes: Vec<(u64, u64, Quoi)>,
    /// Pour chaque tenu, le plus grand numéro envoyé et sa valeur.
    derniers_tenus: Vec<((u64, u8), (u64, u8))>,
    reprises: usize,
}

fn reglage(debit: u16) -> ReglageModele {
    let mut r = ReglageModele::ordinaire();
    r.tenu.debit_max = debit;
    r.rattrapage = 3;
    r
}

/// Joue une soirée ; avec `pannes`, l'autorité s'arrête et reprend où le scénario le dit.
fn soiree(
    pas_de_scenario: &[Vec<Action>],
    debit: u16,
    point_tous_les: u64,
    pannes: Pannes,
) -> Bilan {
    let mut ouv = ouverture();
    ouv.point_tous_les = point_tous_les;
    let mut p = Partie::ouvrir(FabriqueModele::new(reglage(debit)), ouv.clone()).unwrap();
    let mut points = vec![p.point().unwrap()];
    let mut envoyes = Vec::new();
    let mut numeros: Vec<(u64, u64)> = Vec::new();
    let mut derniers_tenus: Vec<((u64, u8), (u64, u8))> = Vec::new();
    let mut id = 0u64;
    let mut reprises = 0;
    for (i, actions) in pas_de_scenario.iter().enumerate() {
        let s = i as u64 + 1;
        let mut panne = None;
        for act in actions {
            match *act {
                Action::Geste { de, quoi, retard } => {
                    id += 1;
                    let r = p
                        .arriver(
                            geste(de, quoi.geste(), s.saturating_sub(retard), id),
                            instant(s),
                        )
                        .unwrap();
                    assert!(matches!(r, Placement::Place { .. }), "{r:?}");
                    envoyes.push((id, de, quoi));
                }
                Action::Tenu {
                    de,
                    cle,
                    valeur,
                    perime,
                } => {
                    let n = if let Some((_, n)) = numeros.iter_mut().find(|(d, _)| *d == de) {
                        *n += 2;
                        *n
                    } else {
                        numeros.push((de, 2));
                        2
                    };
                    let _: Placement = p.arriver(tenu(de, cle, valeur, n, s), instant(s)).unwrap();
                    derniers_tenus.retain(|(k, _)| *k != (de, cle));
                    derniers_tenus.push(((de, cle), (n, valeur)));
                    if perime {
                        let r = p
                            .arriver(tenu(de, cle, (valeur + 1) % 4, n - 1, s), instant(s))
                            .unwrap();
                        // Placé (il partage le pas du récent, qui le précède) ou périmé dès l'arrivée.
                        assert!(
                            matches!(r, Placement::Place { .. } | Placement::Perimee),
                            "{r:?}"
                        );
                    }
                }
                Action::Point => points.push(p.point().unwrap()),
                Action::Panne { k } => panne = Some(k),
            }
        }
        let _: partie::Avancee = p.avancer(pas(s)).unwrap();
        if pannes == Pannes::Avec
            && let Some(k) = panne
        {
            let pt = &points[k % points.len()];
            p = Partie::reprendre(
                FabriqueModele::new(reglage(debit)),
                ouv.clone(),
                pt,
                p.journal(),
            )
            .unwrap();
            reprises += 1;
        }
    }
    // Assez de pas pour que le débit laisse passer le dernier de chaque tenu.
    let fin = p.definitif().numero() + 120;
    while p.definitif().numero() < fin {
        let s = p.definitif().numero() + 3;
        let _: partie::Avancee = p.avancer(pas(s)).unwrap();
    }
    Bilan {
        journal: p.journal().clone(),
        empreinte: p.empreinte().unwrap(),
        coeur: p.coeur().clone(),
        etat: etat(&p).clone(),
        points,
        envoyes,
        derniers_tenus,
        reprises,
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, .. ProptestConfig::default() })]

    /// Un événement compte exactement une fois, à travers les fautes et les reprises
    /// (CONTRAT, « Ce que le réel impose » ; CAT-76, REM-16).
    #[test]
    fn un_evenement_compte_exactement_une_fois((s, debit, tous_les) in scenario()) {
        for pannes in [Pannes::Sans, Pannes::Avec] {
            let b = soiree(&s, debit, tous_les, pannes);
            let ajouts: Vec<u64> = b.envoyes.iter().filter(|(_, _, q)| matches!(q, Quoi::Ajouter(_))).map(|(i, _, _)| *i).collect();
            let mut recus = b.etat.gestes.clone();
            recus.sort_unstable();
            prop_assert_eq!(&recus, &ajouts, "chaque ajout une fois, rien d'autre (pannes : {:?})", pannes);
            let total: u64 = b.envoyes.iter().map(|(_, _, q)| match q { Quoi::Ajouter(n) => u64::from(*n), Quoi::Refuser | Quoi::Paniquer | Quoi::Boucler => 0 }).sum();
            prop_assert_eq!(b.etat.total, total);
            // Au journal aussi, chaque geste une fois.
            let mut au_journal: Vec<u64> = b.journal.entrees().filter_map(|e| match decoder::<EntreeFil<'_>>(&e.octets) {
                Ok(EntreeFil::Geste { quand, .. }) => Some(quand.id),
                _ => None,
            }).collect();
            au_journal.sort_unstable();
            let mut tous: Vec<u64> = b.envoyes.iter().map(|(i, _, _)| *i).collect();
            tous.sort_unstable();
            prop_assert_eq!(au_journal, tous);
        }
    }

    /// L'ordre est stable : celui de `fait`, et celui de chaque appareil (§ 4.3).
    #[test]
    fn l_ordre_est_stable((s, debit, tous_les) in scenario()) {
        let b = soiree(&s, debit, tous_les, Pannes::Sans);
        // Les gestes, dans l'ordre où la définitive les a reçus.
        let joues: Vec<(u64, jeu_abi::QuandFil, jeu_abi::Pas)> = b.journal.entrees().filter_map(|e| match decoder::<EntreeFil<'_>>(&e.octets) {
            Ok(EntreeFil::Geste { de, quand, .. }) => Some((de, quand, e.place)),
            _ => None,
        }).collect();
        for w in joues.windows(2) {
            let (_, q0, p0) = w[0];
            let (_, q1, p1) = w[1];
            prop_assert!((p0, q0.fait, q0.fraction) <= (p1, q1.fait, q1.fraction), "par pas, puis par fait");
        }
        for de in 1..4 {
            let envoyes: Vec<u64> = b.envoyes.iter().filter(|(_, d, _)| *d == de).map(|(i, _, _)| *i).collect();
            let recus: Vec<u64> = joues.iter().filter(|(d, _, _)| *d == de).map(|(_, q, _)| q.id).collect();
            prop_assert_eq!(recus, envoyes, "l'appareil {} garde son ordre", de);
        }
    }

    /// Une faute n'emporte que son entrée (§ 4.6) : les écartées sont exactement les
    /// gestes fautifs, et rien d'autre ne manque.
    #[test]
    fn une_faute_n_emporte_que_son_entree((s, debit, tous_les) in scenario()) {
        let b = soiree(&s, debit, tous_les, Pannes::Avec);
        let fautifs: Vec<u64> = b.envoyes.iter().filter(|(_, _, q)| matches!(q, Quoi::Paniquer | Quoi::Boucler)).map(|(i, _, _)| *i).collect();
        let mut ecartes: Vec<u64> = b.journal.entrees().filter(|e| matches!(e.issue, IssueJournal::Ecartee { .. })).map(|e| match decoder::<EntreeFil<'_>>(&e.octets) {
            Ok(EntreeFil::Geste { quand, .. }) => quand.id,
            autre => panic!("une écartée qui n'est pas un geste fautif : {autre:?}"),
        }).collect();
        ecartes.sort_unstable();
        prop_assert_eq!(ecartes, fautifs);
        let refuses = b.journal.entrees().filter(|e| matches!(e.issue, IssueJournal::Refusee { .. })).count();
        prop_assert_eq!(refuses, b.envoyes.iter().filter(|(_, _, q)| matches!(q, Quoi::Refuser)).count());
    }

    /// Une reprise depuis n'importe quel point donne la même histoire (§ 4.5, E14) :
    /// même journal, même empreinte, même cœur, avec ou sans pannes en chemin.
    #[test]
    fn une_reprise_depuis_n_importe_quel_point_donne_la_meme_histoire((s, debit, tous_les) in scenario()) {
        let sans = soiree(&s, debit, tous_les, Pannes::Sans);
        let avec = soiree(&s, debit, tous_les, Pannes::Avec);
        prop_assert_eq!(&avec.journal, &sans.journal, "{} reprises", avec.reprises);
        prop_assert_eq!(avec.empreinte, sans.empreinte);
        prop_assert_eq!(&avec.coeur, &sans.coeur);
        let mut ouv = ouverture();
        ouv.point_tous_les = tous_les;
        for pt in &sans.points {
            let mut r = Partie::reprendre(FabriqueModele::new(reglage(debit)), ouv.clone(), pt, &sans.journal).unwrap();
            prop_assert_eq!(r.empreinte().unwrap(), sans.empreinte);
            prop_assert_eq!(r.coeur(), &sans.coeur);
        }
    }

    /// Un état périmé est remplacé par le plus récent (REM-13, REM-14) : à la fin,
    /// chaque tenu vaut ce que porte son plus grand numéro.
    #[test]
    fn un_etat_perime_est_remplace_par_le_plus_recent((s, debit, tous_les) in scenario()) {
        let b = soiree(&s, debit, tous_les, Pannes::Avec);
        for ((de, cle), (_, valeur)) in &b.derniers_tenus {
            let tenu = b.etat.tenus.iter().find(|(d, c, _)| d == de && c == cle).map(|(_, _, v)| *v);
            prop_assert_eq!(tenu, Some(*valeur), "tenu ({}, {})", de, cle);
        }
    }
}

/// Un contrôle qui n'a pas tourné ne compte pas (OUT-07) : ce scénario écrit à la
/// main passe par des fautes, des tenus périmés et trois reprises, dont une depuis
/// le point de la création, et rend la même histoire que sans panne.
#[test]
fn une_soiree_ecrite_a_la_main_reprend_la_meme_histoire() {
    use Action::{Geste as G, Panne, Point, Tenu as T};
    let s = vec![
        vec![
            G {
                de: 1,
                quoi: Quoi::Ajouter(1),
                retard: 0,
            },
            T {
                de: 2,
                cle: 0,
                valeur: 3,
                perime: true,
            },
        ],
        vec![
            G {
                de: 2,
                quoi: Quoi::Paniquer,
                retard: 1,
            },
            G {
                de: 1,
                quoi: Quoi::Ajouter(2),
                retard: 0,
            },
            Point,
        ],
        vec![Panne { k: 0 }],
        vec![
            G {
                de: 3,
                quoi: Quoi::Boucler,
                retard: 0,
            },
            T {
                de: 2,
                cle: 1,
                valeur: 1,
                perime: false,
            },
        ],
        vec![
            G {
                de: 1,
                quoi: Quoi::Refuser,
                retard: 2,
            },
            Panne { k: 1 },
        ],
        vec![
            T {
                de: 2,
                cle: 0,
                valeur: 0,
                perime: true,
            },
            G {
                de: 2,
                quoi: Quoi::Ajouter(4),
                retard: 0,
            },
        ],
        vec![Panne { k: 0 }],
    ];
    let sans = soiree(&s, 10, 3, Pannes::Sans);
    let avec = soiree(&s, 10, 3, Pannes::Avec);
    assert_eq!(avec.reprises, 3);
    assert_eq!(avec.journal, sans.journal);
    assert_eq!(avec.empreinte, sans.empreinte);
    assert_eq!(avec.coeur, sans.coeur);
    assert_eq!(avec.etat.total, 7);
    assert_eq!(ecartees(&avec.journal), 2);
    assert_eq!(avec.etat.tenus, vec![(2, 1, 1), (2, 0, 0)]);
}
