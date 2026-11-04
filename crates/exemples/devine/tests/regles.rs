//! Les règles de `devine` sur sa copie native (INTERFACE.md, § 9 et § 8.3), dont
//! `refus_ne_change_rien` : une propriété sur toute suite d'entrées (PUN-27, HIS-44,
//! OUT-171, CAT-94, SQ-25, TL-02).

#![expect(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::let_underscore_untyped,
    clippy::wildcard_enum_match_arm,
    reason = "code de test : un échec s'y lit comme l'échec du test qui l'appelle"
)]

mod commun;

use commun::{A, B, Decide, Pilote};
use devine::{Avis, Devine, Etape, GesteTel, VueTel};
use jeu::prelude::*;
use jeu_abi::{
    Appel, ConstatFil, CoupFil, DemandeFil, EntreeFil, RegardFil, Retour, StyleFil, decoder,
    encoder,
};
use jeu_natif::InstanceNative;
use proptest::prelude::*;

fn pilote() -> Pilote<InstanceNative<Devine>> {
    let mut p = Pilote::new(InstanceNative::new());
    p.creer([3; 32], None);
    p
}

/// Une manche entière : rejoindre, lancer, proposer, révéler, écrire la ligne, la retirer.
#[test]
fn une_manche_complete() {
    let mut p = pilote();
    assert_eq!(p.vue_telephone(A), VueTel::Prenom);
    assert_eq!(p.rejoindre(A, "Léa", 1), Decide::Acceptee);
    assert_eq!(
        p.rejoindre(B, "\u{200B}", 1),
        Decide::Refusee(Avis::PasMaintenant)
    );
    assert_eq!(p.rejoindre(B, "Bob", 2), Decide::Acceptee);
    // Proposer avant la manche : trop tard pour la précédente, ou trop tôt.
    assert_eq!(
        p.geste(A, &GesteTel::Proposer, 3, 3),
        Decide::Refusee(Avis::TropTard)
    );
    assert_eq!(p.lancer(10, 9), Decide::Acceptee);
    assert_eq!(
        p.vue_telephone(A),
        VueTel::Choisis {
            curseur: Borne::PLUS_PETIT
        }
    );
    // Une deuxième manche ne se lance pas pendant les essais.
    assert_eq!(p.lancer(11, 10), Decide::Ignoree);
    assert_eq!(p.curseur(A, 7, 12), Decide::Acceptee);
    // Un geste fait sur une vue d'avant la manche tombe sur `quand.vu` (PUN-38).
    assert_eq!(
        p.geste(A, &GesteTel::Proposer, 13, 5),
        Decide::Refusee(Avis::TropTard)
    );
    assert_eq!(p.geste(A, &GesteTel::Proposer, 13, 12), Decide::Acceptee);
    assert_eq!(
        p.geste(A, &GesteTel::Proposer, 14, 13),
        Decide::Refusee(Avis::DejaPropose)
    );
    assert_eq!(p.vue_telephone(A), VueTel::Attends);
    // Le nombre est secret jusqu'à l'échéance : l'écran ne le montre pas (CAT-46).
    assert_eq!(p.textes_ecran(), [] as [std::string::String; 0]);
    let echeance = 10 + Duree::s(20).en_pas();
    assert_eq!(p.temps(echeance - 1), Decide::Ignoree);
    assert_eq!(p.temps(echeance), Decide::Acceptee);
    let textes = p.textes_ecran();
    assert_eq!(textes.len(), 1, "{textes:?}");
    let VueTel::Resultat { nombre, gagne } = p.vue_telephone(A) else {
        panic!("pas de résultat");
    };
    assert_eq!(textes[0], nombre.get().to_string());
    assert_eq!(
        gagne,
        if nombre.get() == 7 {
            Oui::Oui
        } else {
            Oui::Non
        }
    );
    // La ligne de résultats est une demande, listée jusqu'à sa réponse (TL-45, CAT-76).
    let (ids, nouvelles) = p.avec(&Appel::Etat { connues: vec![] }, |r| match r {
        Retour::Etat(e) => {
            assert_eq!(e.phase_indice, 2);
            (e.demandes.clone(), e.nouvelles.len())
        }
        autre => panic!("{autre:?}"),
    });
    assert_eq!(ids.len(), 1);
    assert_eq!(nouvelles, 1);
    let deja = p.avec(
        &Appel::Etat {
            connues: ids.clone(),
        },
        |r| match r {
            Retour::Etat(e) => {
                assert!(
                    matches!(e.nouvelles.as_slice(), []),
                    "une demande connue ne se redonne pas"
                );
                e.demandes.clone()
            }
            autre => panic!("{autre:?}"),
        },
    );
    assert_eq!(deja, ids);
    let reponse = EntreeFil::Reponse {
        id: ids[0],
        reponse: Reponse::Fait(Ok(())),
    };
    assert_eq!(p.entree(echeance + 1, &reponse), Decide::Acceptee);
    p.avec(&Appel::Etat { connues: ids }, |r| match r {
        Retour::Etat(e) => assert_eq!(e.demandes, [] as [u64; 0]),
        autre => panic!("{autre:?}"),
    });
    // Les invariants tiennent.
    p.avec(&Appel::Verifier, |r| match r {
        Retour::Verif(v) => assert!(v.iter().all(|(_, c)| *c == ConstatFil::Tenu), "{v:?}"),
        autre => panic!("{autre:?}"),
    });
}

/// La demande d'une ligne porte bien le flux des résultats.
#[test]
fn la_ligne_va_au_flux_des_resultats() {
    let mut p = pilote();
    p.rejoindre(A, "Léa", 1);
    p.lancer(2, 2);
    p.temps(2 + Duree::s(20).en_pas());
    p.avec(&Appel::Etat { connues: vec![] }, |r| match r {
        Retour::Etat(e) => match &e.nouvelles[0].1 {
            DemandeFil::Ligne { flux, octets } => {
                assert_eq!(flux.nom(), "resultats");
                let (manche, points): (u32, Vec<u32>) = decoder(octets).unwrap();
                assert_eq!(manche, 1);
                assert_eq!(points.len(), 1);
            }
            autre => panic!("{autre:?}"),
        },
        autre => panic!("{autre:?}"),
    });
}

/// Une entrée tirée au hasard.
#[derive(Debug, Clone)]
enum Coup {
    Rejoindre(u8, String),
    Proposer(u8, u8),
    Curseur(u8, u8),
    Lancer(u8),
    Temps,
    Depart(u8),
    Retour(u8),
    Reponse(u8),
}

fn coup() -> impl Strategy<Value = Coup> {
    prop_oneof![
        (
            0u8..3,
            prop::sample::select(vec!["Léa", "", "\u{200B}", "Bob", "ßß"]).prop_map(String::from)
        )
            .prop_map(|(d, n)| Coup::Rejoindre(d, n)),
        (0u8..3, 0u8..40).prop_map(|(d, v)| Coup::Proposer(d, v)),
        (0u8..3, 0u8..10).prop_map(|(d, c)| Coup::Curseur(d, c)),
        (0u8..40).prop_map(Coup::Lancer),
        Just(Coup::Temps),
        (0u8..3).prop_map(Coup::Depart),
        (0u8..3).prop_map(Coup::Retour),
        (0u8..4).prop_map(Coup::Reponse),
    ]
}

fn appareil(d: u8) -> u64 {
    [A, B, 0x9999][usize::from(d)]
}

fn jouer(p: &mut Pilote<InstanceNative<Devine>>, c: &Coup, pas: &mut u64) -> Decide {
    *pas += 1;
    let place = *pas;
    let vu = |v: u8| place.saturating_sub(u64::from(v));
    match c {
        Coup::Rejoindre(d, n) => p.rejoindre(appareil(*d), n, place),
        Coup::Proposer(d, v) => p.geste(appareil(*d), &GesteTel::Proposer, place, vu(*v)),
        Coup::Curseur(d, c) => p.curseur(appareil(*d), i64::from(*c), place),
        Coup::Lancer(v) => p.lancer(place, vu(*v)),
        Coup::Temps => {
            *pas += 400;
            p.temps(*pas)
        }
        Coup::Depart(d) => p.entree(
            place,
            &EntreeFil::Presence(Presence::Depart {
                appareil: Appareil::du_fil(appareil(*d)),
                cause: CauseDepart::Silence,
            }),
        ),
        Coup::Retour(d) => p.entree(
            place,
            &EntreeFil::Presence(Presence::Retour {
                appareil: Appareil::du_fil(appareil(*d)),
                capacites: Capacites::AUCUNE,
            }),
        ),
        Coup::Reponse(id) => p.entree(
            place,
            &EntreeFil::Reponse {
                id: u64::from(*id),
                reponse: Reponse::Fait(Ok(())),
            },
        ),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Un refus et un `ignorer` ne changent rien : la forme canonique, secrets et hasard
    /// compris, est la même avant et après (PUN-27, HIS-44, OUT-171, CAT-94, SQ-25, TL-02).
    /// Une lecture non plus (TL-01). Et les invariants tiennent à chaque pas (E4, en petit).
    #[test]
    fn refus_ne_change_rien(coups in prop::collection::vec(coup(), 1..60)) {
        let mut p = pilote();
        let mut pas = 0;
        let mut refus = 0;
        for c in &coups {
            let avant = p.canonique();
            let d = jouer(&mut p, c, &mut pas);
            let apres = p.canonique();
            match d {
                Decide::Refusee(_) | Decide::Ignoree => {
                    refus += 1;
                    prop_assert_eq!(&avant, &apres, "{:?} a changé l'état", c);
                }
                Decide::Acceptee => {}
                Decide::Attente => prop_assert!(false, "un secret manque sur l'autorité : {:?}", c),
            }
            let _ = p.vues(&[RegardFil::Ecran, RegardFil::Appareil(A), RegardFil::Appareil(B)]);
            let _ = p.brut(&Appel::Etat { connues: vec![] }).unwrap();
            let _ = p.brut(&Appel::Bot { regard: RegardFil::Appareil(A), style: StyleFil::Hostile, graine: 1, depuis_vue: 100 }).unwrap();
            prop_assert_eq!(&apres, &p.canonique(), "une lecture a changé l'état");
            p.avec(&Appel::Verifier, |r| match r {
                Retour::Verif(v) => assert!(v.iter().all(|(_, c)| *c == ConstatFil::Tenu), "{v:?}"),
                autre => panic!("{autre:?}"),
            });
        }
        let _ = refus;
    }

    /// Une vue ne lit les secrets qu'à travers son regard : la vue de l'état et la vue de
    /// sa projection pour ce regard sont les mêmes (§ 5.2, E8 en petit, CAT-46).
    #[test]
    fn vue_de_la_projection_est_la_vue(coups in prop::collection::vec(coup(), 1..40)) {
        let mut p = pilote();
        let mut pas = 0;
        for c in &coups {
            let _ = jouer(&mut p, c, &mut pas);
            for r in [RegardFil::Ecran, RegardFil::Appareil(A), RegardFil::Appareil(B)] {
                let vue = p.vues(&[r]).remove(0);
                let projection = p.projection(r);
                let mut q = Pilote::new(InstanceNative::<Devine>::new());
                q.avec(&Appel::Charger { projection: &projection }, |r| assert_eq!(r, Retour::Fait));
                prop_assert_eq!(q.vues(&[r]).remove(0), vue);
            }
        }
    }
}

/// La forme canonique se recharge : même état, même suite (D24, § 3.8).
#[test]
fn depuis_canonique_aller_retour() {
    let mut p = pilote();
    p.rejoindre(A, "Léa", 1);
    p.lancer(2, 2);
    let c = p.canonique();
    let forme = u64::from_le_bytes(c[..8].try_into().unwrap());
    assert_eq!(
        &c[c.len() - 8..],
        &0u64.to_le_bytes(),
        "aucun flottant non fini"
    );
    let mut q = Pilote::new(InstanceNative::<Devine>::new());
    q.avec(&Appel::DepuisCanonique { forme, octets: &c }, |r| {
        assert_eq!(r, Retour::Fait);
    });
    assert_eq!(q.canonique(), c);
    // La suite est la même, hasard compris.
    let fin = 2 + Duree::s(20).en_pas();
    for x in [&mut p, &mut q] {
        x.temps(fin);
        x.lancer(fin + 1, fin + 1);
    }
    assert_eq!(p.canonique(), q.canonique());
    // Une autre forme est refusée net, sans rien écraser (HIS-51, TL-13).
    let o = q
        .brut(&Appel::DepuisCanonique {
            forme: forme ^ 1,
            octets: &c,
        })
        .unwrap();
    assert!(matches!(
        decoder::<Retour<'_>>(&o).unwrap(),
        Retour::Illisible(_)
    ));
    assert_eq!(q.canonique(), p.canonique());
}

/// La projection d'un téléphone ne contient ni le nombre ni le hasard : rechargée, elle
/// n'ouvre pas le nombre, et un tirage y attend (D21, § 5.1).
#[test]
fn projection_cache_le_nombre_et_le_hasard() {
    let mut p = pilote();
    p.rejoindre(A, "Léa", 1);
    p.lancer(2, 2);
    let projection = p.projection(RegardFil::Appareil(A));
    let mut q = Pilote::new(InstanceNative::<Devine>::new());
    q.avec(
        &Appel::Charger {
            projection: &projection,
        },
        |r| assert_eq!(r, Retour::Fait),
    );
    let fin = 2 + Duree::s(20).en_pas();
    assert_eq!(q.temps(fin), Decide::Acceptee);
    // Sans le nombre, la révélation ne montre rien.
    assert_eq!(q.textes_ecran(), [] as [std::string::String; 0]);
    assert_eq!(q.lancer(fin + 1, fin + 1), Decide::Attente);
}

/// Une nouvelle partie garde les joueurs (CAT-73, CAT-75).
#[test]
fn nouvelle_partie_garde_les_joueurs() {
    let mut p = pilote();
    p.rejoindre(A, "Léa", 1);
    let garde = p.avec(&Appel::Garder, |r| match r {
        Retour::Octets(o) => o.to_vec(),
        autre => panic!("{autre:?}"),
    });
    let mut q = Pilote::new(InstanceNative::<Devine>::new());
    q.creer([9; 32], Some(&garde));
    assert_eq!(q.vue_telephone(A), VueTel::Attends);
    assert_eq!(q.vue_telephone(B), VueTel::Prenom);
}

/// Chaque cas a son pire cas : `cas(pire_cas(c)) = c`, invariants tenus (E16, en petit).
#[test]
fn pire_cas_de_chaque_etape() {
    for (i, etape) in Etape::TOUTES.iter().enumerate() {
        let mut p = pilote();
        let cas = p.avec(
            &Appel::PireCas {
                indice: u32::try_from(i).unwrap(),
            },
            |r| match r {
                Retour::PireCas { cas } => decoder::<Etape>(cas).unwrap(),
                autre => panic!("{autre:?}"),
            },
        );
        assert_eq!(cas, *etape);
        p.avec(&Appel::Verifier, |r| match r {
            Retour::Verif(v) => assert!(v.iter().all(|(_, c)| *c == ConstatFil::Tenu), "{v:?}"),
            autre => panic!("{autre:?}"),
        });
        let _ = p.vues(&[RegardFil::Ecran]);
    }
}

/// Un bot joue depuis sa vue : sans prénom, il rejoint ; il propose après 800 ms.
#[test]
fn le_bot_joue_depuis_sa_vue() {
    let mut p = pilote();
    let coups = p.avec(
        &Appel::Bot {
            regard: RegardFil::Appareil(A),
            style: StyleFil::Sage,
            graine: 5,
            depuis_vue: 0,
        },
        |r| match r {
            Retour::Bot(c) => c
                .iter()
                .map(|c| match c {
                    CoupFil::Geste(o) => decoder::<GesteTel>(o).map(|_| "geste").unwrap(),
                    CoupFil::Rien => "rien",
                    autre => panic!("{autre:?}"),
                })
                .collect::<Vec<_>>(),
            autre => panic!("{autre:?}"),
        },
    );
    assert_eq!(coups, ["geste"]);
    p.rejoindre(A, "Léa", 1);
    p.lancer(2, 2);
    let coups = p.avec(
        &Appel::Bot {
            regard: RegardFil::Appareil(A),
            style: StyleFil::Sage,
            graine: 5,
            depuis_vue: 60,
        },
        |r| match r {
            Retour::Bot(c) => c.len(),
            autre => panic!("{autre:?}"),
        },
    );
    assert_eq!(coups, 2, "un curseur, puis Proposer");
}

/// Des octets d'appareil mal formés sont `Malforme`, et un tenu dit sa clé (§ 3.5).
#[test]
fn valider_les_octets_d_un_appareil() {
    let mut p = pilote();
    let mut sale = Vec::new();
    encoder(&"\u{200B}Léa", &mut sale).unwrap();
    let mut geste = vec![0u8];
    geste.extend(&sale);
    p.avec(
        &Appel::Valider {
            sorte: jeu_abi::SorteOctets::GesteTelephone,
            octets: &geste,
        },
        |r| {
            assert!(
                matches!(r, Retour::Valide(jeu_abi::Validation::Malforme(_))),
                "{r:?}"
            );
        },
    );
    let mut trois = Vec::new();
    encoder(&devine::Curseur(Borne::new(3).unwrap()), &mut trois).unwrap();
    p.avec(
        &Appel::Valider {
            sorte: jeu_abi::SorteOctets::TenuTelephone,
            octets: &trois,
        },
        |r| match r {
            Retour::Valide(jeu_abi::Validation::Valide { tenu: Some(t) }) => {
                assert!(t.relache.is_none(), "un curseur ne se relâche pas seul");
                assert_eq!(t.est_relache, Oui::Non);
            }
            autre => panic!("{autre:?}"),
        },
    );
    p.avec(
        // 10, en zigzag : hors de 0..=9.
        &Appel::Valider {
            sorte: jeu_abi::SorteOctets::TenuTelephone,
            octets: &[20],
        },
        |r| {
            assert!(
                matches!(r, Retour::Valide(jeu_abi::Validation::Malforme(_))),
                "un chiffre hors de 0..=9"
            );
        },
    );
}

/// E10, E11 (§ 2.13) : les propriétés promises passent l'enveloppe (ABI 2) ; l'hypothèse
/// de la coupure transparente tient pour un joueur qui a déjà proposé, pas avant.
#[test]
fn proprietes_et_hypothese_passent_l_enveloppe() {
    let mut p = pilote();
    p.avec(&Appel::Proprietes, |r| match r {
        Retour::Proprietes(jeu_abi::ProprietesFil {
            coupure_transparente,
            permutation,
        }) => {
            assert_eq!(
                coupure_transparente,
                jeu_abi::ApplicableFil::Oui(Duree::s(10))
            );
            assert_eq!(permutation, jeu_abi::ApplicableFil::Oui(()));
        }
        autre => panic!("{autre:?}"),
    });
    let tient = |p: &mut Pilote<InstanceNative<Devine>>| {
        p.avec(&Appel::Hypothese { appareil: 7 }, |r| match r {
            Retour::Hypothese(o) => o,
            autre => panic!("{autre:?}"),
        })
    };
    assert_eq!(p.rejoindre(7, "ALICE", 1), Decide::Acceptee);
    assert_eq!(p.lancer(2, 1), Decide::Acceptee);
    assert_eq!(tient(&mut p), Oui::Non);
    assert_eq!(p.geste(7, &GesteTel::Proposer, 3, 2), Decide::Acceptee);
    assert_eq!(tient(&mut p), Oui::Oui);
}

/// Besoin 7 des jeux : l'attente lit les secrets. Pendant les essais, elle nomme les
/// joueurs qui n'ont pas encore proposé, alors que leur essai est caché à tous les
/// autres ; le premier qui propose en sort.
#[test]
fn l_attente_nomme_ceux_qui_n_ont_pas_propose() {
    let mut p = pilote();
    assert_eq!(p.rejoindre(A, "Léa", 1), Decide::Acceptee);
    assert_eq!(p.rejoindre(B, "Bob", 2), Decide::Acceptee);
    let attendus = |p: &mut Pilote<InstanceNative<Devine>>| -> Vec<u64> {
        p.avec(
            &Appel::Etat {
                connues: Vec::new(),
            },
            |r| match r {
                Retour::Etat(e) => e.attente.joueurs.iter().map(|(a, _)| *a).collect(),
                autre => panic!("{autre:?}"),
            },
        )
    };
    assert_eq!(attendus(&mut p), Vec::<u64>::new(), "au salon, l'écran");
    assert_eq!(p.lancer(10, 9), Decide::Acceptee);
    assert_eq!(attendus(&mut p), vec![A, B]);
    assert_eq!(p.geste(A, &GesteTel::Proposer, 12, 11), Decide::Acceptee);
    assert_eq!(attendus(&mut p), vec![B]);
}
