//! Un pas qui se finit sur plusieurs tours (PHASE-3, § 2.11, P7) : les arriérés gardent
//! leur pas, les arrivées d'un pas se figent au tour où il commence, et le journal ne
//! dépend pas du découpage (CAT-14).

use jeu_abi::{Duree, IdGeste, Oui, Pas, RegardFil, SceneFil, SorteNoeud, decoder};
use partie::modele::{
    CLE_TOTAL, CLE_TOTAL_DEFINITIF, FabriqueModele, GesteModele, ReglageModele, texte_du_total,
};
use partie::{
    ARRIERE_PAS, ARRIERES_PAR_TOUR, Arrivee, EtatCoeur, Journal, Partie, Placement, QuoiPlace, Vue,
};
use proptest::prelude::*;

use crate::commun::*;

/// Un geste fait à `fait`, sur la vue du pas `vu`, reçu sans latence.
fn geste_vu(de: u64, g: GesteModele, fait: u64, vu: u64, id: u64) -> Arrivee {
    Arrivee::Geste {
        de: a(de),
        octets: octets(&g),
        fait: instant(fait),
        vu: pas(vu),
        id: IdGeste::du_fil(id),
        demi_aller_retour: Duree::ZERO,
    }
}

/// Un modèle qui ne demande le temps qu'à chaque pas et rattrape trois pas.
fn reglage() -> ReglageModele {
    ReglageModele::ordinaire()
}

/// Rend la partie avec une vue de l'écran et de chaque appareil au pas courant : ce
/// qui borne le vu des gestes qui suivent.
fn regarder(p: &mut PartieModele) -> Vec<Vue> {
    let regards: Vec<RegardFil> = (1..=3).map(RegardFil::Appareil).collect();
    p.regarder(&regards).unwrap()
}

// ---------------------------------------------------------------------------
// Les arriérés, un à un
// ---------------------------------------------------------------------------

/// Un pas de vingt gestes frais se joue d'un tour, quel que soit le budget : seuls les
/// arriérés comptent (PHASE-3, § 2.11).
#[test]
fn une_entree_fraiche_n_est_jamais_retenue() {
    let mut p = ouvrir(reglage());
    for id in 0..20 {
        let de = 1 + id % 3;
        let _: Placement = p
            .arriver(geste(de, GesteModele::Ajouter(1), 1, id), instant(1))
            .unwrap();
    }
    let av = p.avancer_borne(pas(1), 1).unwrap();
    assert_eq!(av.en_cours, None);
    assert_eq!(p.definitif(), pas(1));
    assert_eq!(etat(&p).gestes.len(), 20);
}

/// Des gestes faits sur une vue de plus de `ARRIERE_PAS` pas avant leur réception sont
/// des arriérés : `ARRIERES_PAR_TOUR` au plus par tour ; le pas reste en cours, `Temps`
/// vient après eux, et le pas ne change pas (P7).
#[test]
fn les_arrieres_d_un_pas_se_jouent_sur_plusieurs_tours() {
    let mut p = ouvrir(reglage());
    avancer(&mut p, 100);
    let _: Vec<Vue> = regarder(&mut p);
    let recu = 100 + ARRIERE_PAS + 5;
    avancer(&mut p, recu - 1);
    let n = u64::from(ARRIERES_PAR_TOUR) * 2 + 3;
    for id in 0..n {
        let de = 1 + id % 3;
        // Fait juste avant la réception, sur la vue du pas 100 : un arriéré.
        let r = p
            .arriver(
                geste_vu(de, GesteModele::Ajouter(1), recu, 100, id),
                instant(recu),
            )
            .unwrap();
        assert_eq!(r, Placement::Place { pas: pas(recu) });
    }
    let temps_avant = temps(p.journal()).len();
    let mut tours = 0;
    loop {
        tours += 1;
        let av = p.avancer_borne(pas(recu), ARRIERES_PAR_TOUR).unwrap();
        if av.en_cours.is_none() {
            break;
        }
        assert_eq!(av.en_cours, Some(pas(recu)));
        assert_eq!(
            av.definitif,
            pas(recu - 1),
            "le pas en cours n'est pas définitif"
        );
        assert_eq!(p.pas_en_cours(), Some(pas(recu)));
    }
    assert_eq!(tours, 3, "{n} arriérés à {ARRIERES_PAR_TOUR} par tour");
    assert_eq!(p.definitif(), pas(recu));
    assert_eq!(etat(&p).gestes.len(), usize::try_from(n).unwrap());
    // Toutes à leur pas, puis le `Temps` de ce pas, après elles.
    let j = p.journal();
    let du_pas: Vec<_> = entrees(j)
        .into_iter()
        .filter(|e| e.place == pas(recu))
        .collect();
    assert_eq!(du_pas.len(), usize::try_from(n).unwrap() + 1);
    assert_eq!(temps(j).len(), temps_avant + 1);
    assert_eq!(temps(j).last().map(|t| t.0), Some(pas(recu)));
}

/// Une arrivée reçue pendant un pas inachevé va au pas suivant au plus tôt : les
/// arrivées d'un pas se figent au tour où il commence (P7).
#[test]
fn une_arrivee_pendant_un_pas_inacheve_va_au_pas_suivant() {
    let mut p = ouvrir(reglage());
    for id in 0..3 {
        let _: Placement = p
            .arriver(geste(1, GesteModele::Ajouter(1), 1, id), instant(1))
            .unwrap();
    }
    let av = p.avancer_decoupe(pas(1), 1).unwrap();
    assert_eq!(av.en_cours, Some(pas(1)));
    let r = p
        .arriver(geste(2, GesteModele::Ajouter(5), 1, 9), instant(1))
        .unwrap();
    assert_eq!(r, Placement::Place { pas: pas(2) });
    let _: partie::Avancee = p.avancer(pas(2)).unwrap();
    let j = p.journal();
    assert_eq!(quand_du_geste(j, 9).id, 9);
    let place = entrees(j)
        .into_iter()
        .find(|e| matches!(jeu_abi::decoder::<jeu_abi::EntreeFil<'_>>(&e.octets), Ok(jeu_abi::EntreeFil::Geste { quand, .. }) if quand.id == 9))
        .map(|e| e.place);
    assert_eq!(place, Some(pas(2)));
}

/// Pendant un pas inachevé, la définitive est au milieu de ce pas : une vue est celle
/// d'un pas fini (SQ-86), et un point se prend au bord d'un pas.
#[test]
fn un_pas_inacheve_ne_se_regarde_ni_ne_se_garde() {
    let mut p = ouvrir(reglage());
    let avant = regarder(&mut p);
    let point_avant = p.point().unwrap();
    for id in 0..3 {
        let _: Placement = p
            .arriver(geste(1, GesteModele::Ajouter(1), 1, id), instant(1))
            .unwrap();
    }
    let _: partie::Avancee = p.avancer_decoupe(pas(1), 1).unwrap();
    assert_eq!(p.pas_en_cours(), Some(pas(1)));
    let lignes = p.journal().fin();
    assert_eq!(
        regarder(&mut p),
        avant,
        "les dernières vues valides, sans lire"
    );
    assert_eq!(p.journal().fin(), lignes, "aucune vue notée au journal");
    assert_eq!(
        p.point().unwrap(),
        point_avant,
        "le dernier point, au bord du pas 0"
    );
    assert!(p.fin_du_dernier_pas() < p.journal().fin());
    let _: partie::Avancee = p.avancer(pas(1)).unwrap();
    assert_eq!(p.pas_en_cours(), None);
    assert_eq!(p.fin_du_dernier_pas(), p.journal().fin());
}

/// Une pause se fait entre deux pas : pendant un pas inachevé, elle le finit d'abord,
/// et le journal se rejoue.
#[test]
fn une_pause_finit_le_pas_inacheve() {
    let mut p = ouvrir(reglage());
    let depart = p.point().unwrap();
    for id in 0..3 {
        let _: Placement = p
            .arriver(geste(1, GesteModele::Ajouter(1), 1, id), instant(1))
            .unwrap();
    }
    let _: partie::Avancee = p.avancer_decoupe(pas(1), 1).unwrap();
    assert_eq!(p.pause(Oui::Oui).unwrap(), Oui::Oui);
    assert_eq!(p.pas_en_cours(), None);
    assert_eq!(p.definitif(), pas(1));
    avancer(&mut p, 4);
    let r = Partie::reprendre(
        FabriqueModele::new(reglage()),
        ouverture(),
        &depart,
        p.journal(),
    )
    .unwrap();
    assert_eq!(r.coeur(), p.coeur());
}

/// Le total que la scène de l'écran montre, et son total définitif : la tête, et la
/// définitive (§ 4.4).
fn totaux_de_l_ecran(p: &mut PartieModele) -> (Pas, Option<String>, Option<String>) {
    let texte = |s: &SceneFil, cle: jeu_abi::Cle| {
        s.noeuds.iter().find(|n| n.cle == cle).and_then(|n| {
            if let SorteNoeud::Texte { texte, .. } = &n.sorte {
                Some(texte.clone())
            } else {
                None
            }
        })
    };
    let v = p.regarder(&[RegardFil::Ecran]).unwrap().remove(0);
    let s: SceneFil = decoder(&v.octets).unwrap();
    (v.pas, texte(&s, CLE_TOTAL), texte(&s, CLE_TOTAL_DEFINITIF))
}

/// La place d'un geste au journal, par son identité.
fn place_du_geste(j: &Journal, id: u64) -> Option<Pas> {
    j.lignes().iter().find_map(|l| {
        let partie::Ligne::Arrivees { arrivees, .. } = l else {
            return None;
        };
        arrivees.iter().find_map(|a| {
            let QuoiPlace::Geste { id: i, .. } = &a.quoi else {
                return None;
            };
            (i.fil() == id).then_some(a.place)
        })
    })
}

/// Vérification du lot 1, constat 1 : une arrivée reçue au milieu d'une avancée de
/// plusieurs pas, faite à un pas que la tête avait déjà joué et avant la borne de la
/// définitive, entrait dans l'avancée (`max(fait.pas, pas en cours + 1)`) : la tête
/// ne la voyait jamais, et l'écran montrait pour de bon un autre total que la
/// définitive. Elle va désormais après la fin de l'avancée, comme d'un seul tour, et
/// la tête se rejoue.
#[test]
fn une_arrivee_au_milieu_d_une_avancee_va_apres_elle() {
    let jouer = |decoupe: bool| {
        let mut r = reglage();
        r.reveil = jeu_abi::ReveilFil::Jamais;
        r.fenetre = jeu_abi::ApplicableFil::Oui(Duree::pas(2));
        let mut p = ouvrir(r);
        // L'horloge va pas à pas : avec une fenêtre, la définitive la suit à deux pas.
        for s in 1..=6 {
            let _: partie::Avancee = p.avancer(pas(s)).unwrap();
        }
        for id in 1..=2 {
            let _: Placement = p
                .arriver(geste(1, GesteModele::Ajouter(1), 6, id), instant(6))
                .unwrap();
        }
        let _: partie::Avancee = p.avancer(pas(7)).unwrap();
        assert_eq!(
            (p.definitif(), p.tete()),
            (pas(5), partie::Tete::Separee { pas: pas(7) })
        );
        if decoupe {
            // Rattrapage de 3 : la définitive doit aller de 5 à 7, la tête à 9 ; le
            // pas 6 reste en cours.
            let av = p.avancer_decoupe(pas(9), 1).unwrap();
            assert_eq!(av.en_cours, Some(pas(6)));
            assert_eq!(p.avancee_en_cours(), Some((pas(7), Duree::ZERO)));
        } else {
            let _: partie::Avancee = p.avancer(pas(9)).unwrap();
        }
        // Fait au pas 7, que la tête a joué, et avant la borne.
        let r = p
            .arriver(geste(2, GesteModele::Ajouter(5), 7, 3), instant(7))
            .unwrap();
        assert_eq!(r, Placement::Place { pas: pas(8) }, "après l'avancée");
        while p.avancer_decoupe(pas(9), 1).unwrap().en_cours.is_some() {}
        let au_pas_9 = totaux_de_l_ecran(&mut p);
        for s in 10..=14 {
            let _: partie::Avancee = p.avancer(pas(s)).unwrap();
        }
        assert_eq!(p.definitif(), pas(12));
        let au_pas_14 = totaux_de_l_ecran(&mut p);
        (
            place_du_geste(p.journal(), 3),
            au_pas_9,
            au_pas_14,
            p.journal().clone(),
        )
    };
    let entier = jouer(false);
    let decoupe = jouer(true);
    assert_eq!(entier.0, Some(pas(8)));
    assert_eq!(
        entier.1,
        (pas(9), Some(texte_du_total(7)), Some(texte_du_total(2)))
    );
    assert_eq!(
        entier.2,
        (pas(14), Some(texte_du_total(7)), Some(texte_du_total(7)))
    );
    assert_eq!(decoupe.0, entier.0);
    assert_eq!(decoupe.1, entier.1, "la tête voit l'arrivée au pas 9");
    assert_eq!(decoupe.2, entier.2, "et encore au pas 14");
    assert_eq!(decoupe.3, entier.3);
}

/// Une arrivée reçue pendant une avancée qui finira par un saut va après le saut,
/// comme d'un seul tour : même journal, même cœur (la dernière arrivée de son
/// appareil comprise).
#[test]
fn une_arrivee_avant_un_saut_va_apres_lui() {
    let g = |de: u64, quoi: u8, retard: u64| Op::Geste {
        de,
        quoi,
        retard,
        age: 0,
    };
    let script = [
        Op::Avancer { delta: 2, tours: 1 },
        g(1, 0, 0),
        g(1, 1, 0),
        g(2, 2, 0),
        // Rattrapage de 3 : un saut ; découpée, l'avancée reste au pas 3.
        Op::Avancer { delta: 6, tours: 1 },
        g(3, 3, 2),
        g(1, 4, 1),
        Op::Avancer { delta: 1, tours: 3 },
        Op::Regarder,
    ];
    let entier = jouer_script(&script, Mode::Entier, 0);
    let decoupe = jouer_script(&script, Mode::Decoupe(1), 0);
    assert!(decoupe.avant_la_fin >= 2, "reçues avant le saut");
    assert!(
        entier
            .journal
            .lignes()
            .iter()
            .any(|l| matches!(l, partie::Ligne::Saut { .. })),
        "l'avancée saute"
    );
    assert_eq!(decoupe.journal, entier.journal);
    assert_eq!(decoupe.coeur, entier.coeur);
    assert_eq!(decoupe.ecran, entier.ecran);
}

// ---------------------------------------------------------------------------
// Le même journal, quel que soit le budget du tour (CAT-14)
// ---------------------------------------------------------------------------

/// Une opération du script.
#[derive(Debug, Clone, Copy)]
enum Op {
    /// Un geste de l'appareil `de`, fait `retard` pas avant sa réception, sur une vue
    /// de `age` pas plus vieille que ce qu'il a fait.
    Geste {
        de: u64,
        quoi: u8,
        retard: u64,
        age: u64,
    },
    /// Un tenu de l'appareil `de`.
    Tenu { de: u64, cle: u8, valeur: u8 },
    /// L'horloge avance de `delta` pas : un `Avancer`. Découpé, il joue au plus
    /// `tours` tours ; ce qui suit arrive pendant l'avancée, si elle reste en cours,
    /// à n'importe lequel de ses pas, avant son saut s'il y en a un.
    Avancer { delta: u64, tours: u32 },
    /// Une vue de chaque regard.
    Regarder,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        8 => (1u64..4, 0u8..8, 0u64..3, prop_oneof![Just(0u64), Just(ARRIERE_PAS + 10)])
            .prop_map(|(de, quoi, retard, age)| Op::Geste { de, quoi, retard, age }),
        2 => (1u64..4, 0u8..2, 0u8..4).prop_map(|(de, cle, valeur)| Op::Tenu { de, cle, valeur }),
        4 => (prop_oneof![4 => Just(1u64), 1 => 2u64..6], 1u32..4)
            .prop_map(|(delta, tours)| Op::Avancer { delta, tours }),
        1 => Just(Op::Regarder),
    ]
}

/// Le geste du modèle de ce tirage : surtout des ajouts, dont l'ordre se lit dans
/// l'état ; parfois un refus ou une panique, qui reconstruit la définitive au milieu
/// d'un pas.
fn quoi(q: u8) -> GesteModele {
    match q {
        6 => GesteModele::Refuser,
        7 => GesteModele::Paniquer,
        n => GesteModele::Ajouter(n + 1),
    }
}

/// Comment le script avance.
#[derive(Debug, Clone, Copy)]
enum Mode {
    /// `avancer`, d'un seul tour.
    Entier,
    /// `avancer_decoupe` à `n` entrées par tour : chaque `Avancer` joue au plus son
    /// nombre de tours ; les gestes et tenus qui suivent arrivent alors pendant
    /// l'avancée laissée, à quelque pas qu'elle en soit ; toute autre opération la
    /// finit d'abord.
    Decoupe(u32),
    /// Comme `Decoupe`, avec le contrôle négatif `etale-mal` armé.
    EtaleMal(u32),
}

struct Joue {
    journal: Journal,
    coeur: EtatCoeur,
    etat: partie::modele::EtatModele,
    /// Chaque vue de l'écran, à chaque `Regarder` et à la fin : celle de la tête quand
    /// elle est en avance (§ 4.4), que le journal ne dit pas.
    ecran: Vec<Vue>,
    tours_inacheves: u32,
    /// Les arrivées reçues pendant une avancée inachevée.
    pendant: u32,
    /// Celles reçues avant le dernier pas de l'avancée, ou avant son saut : là où
    /// `max(fait.pas, pas en cours + 1)` les aurait fait entrer dans l'avancée.
    avant_la_fin: u32,
}

fn jouer_script(script: &[Op], mode: Mode, fenetre: u64) -> Joue {
    let mut r = reglage();
    if fenetre > 0 {
        r.fenetre = jeu_abi::ApplicableFil::Oui(Duree::pas(fenetre));
    }
    let mut partie = ouvrir(r);
    let (borne, fautif) = match mode {
        Mode::Entier => (None, false),
        Mode::Decoupe(n) => (Some(n), false),
        Mode::EtaleMal(n) => (Some(n), true),
    };
    if fautif {
        partie.etaler_mal();
    }
    let mut horloge = 0u64;
    let mut id = 0u64;
    let mut numero = 0u64;
    let mut tours_inacheves = 0;
    let mut ecran = Vec::new();
    let mut pendant = 0;
    let mut avant_la_fin = 0;
    // Compte une arrivée reçue pendant une avancée inachevée.
    let compter = |partie: &PartieModele, pendant: &mut u32, avant_la_fin: &mut u32| {
        if let (Some(s), Some(reste)) = (partie.pas_en_cours(), partie.avancee_en_cours()) {
            *pendant += 1;
            if reste != (s, Duree::ZERO) {
                *avant_la_fin += 1;
            }
        }
    };
    // Finir le pas en cours, sans commencer d'avancée neuve : `avancer_decoupe` vers la
    // même cible en commencerait une, qui rejouerait la tête, ce qu'un `avancer` d'un
    // seul tour n'a pas fait.
    let finir = |partie: &mut PartieModele, _: u64, tours: &mut u32| {
        if partie.pas_en_cours().is_some() {
            *tours += 1;
            let _: partie::Avancee = partie.finir_le_pas().unwrap();
        }
    };
    for o in script {
        match *o {
            Op::Geste {
                de,
                quoi: q,
                retard,
                age,
            } => {
                id += 1;
                let recu = horloge + 1;
                let fait = recu.saturating_sub(retard);
                let vu = fait.saturating_sub(age);
                compter(&partie, &mut pendant, &mut avant_la_fin);
                let _: Placement = partie
                    .arriver(geste_vu(de, quoi(q), fait, vu, id), instant(recu))
                    .unwrap();
            }
            Op::Tenu { de, cle, valeur } => {
                numero += 1;
                compter(&partie, &mut pendant, &mut avant_la_fin);
                let _: Placement = partie
                    .arriver(
                        tenu(de, cle, valeur, numero, horloge + 1),
                        instant(horloge + 1),
                    )
                    .unwrap();
            }
            Op::Avancer { delta, tours } => {
                finir(&mut partie, horloge, &mut tours_inacheves);
                horloge += delta;
                match borne {
                    None => {
                        let _: partie::Avancee = partie.avancer(pas(horloge)).unwrap();
                    }
                    Some(n) => {
                        // Le premier tour commence l'avancée ; les suivants, vers la
                        // même cible, la poursuivent sans en commencer une neuve tant
                        // qu'elle reste en cours.
                        for _ in 0..tours {
                            let av = partie.avancer_decoupe(pas(horloge), n).unwrap();
                            if av.en_cours.is_none() {
                                break;
                            }
                            tours_inacheves += 1;
                        }
                    }
                }
            }
            Op::Regarder => {
                finir(&mut partie, horloge, &mut tours_inacheves);
                let _: Vec<Vue> = regarder(&mut partie);
                // L'écran voit la tête, quand elle est en avance (§ 4.4).
                ecran.extend(partie.regarder(&[RegardFil::Ecran]).unwrap());
            }
        }
    }
    finir(&mut partie, horloge, &mut tours_inacheves);
    let _: partie::Avancee = partie.avancer(pas(horloge + 1)).unwrap();
    ecran.extend(partie.regarder(&[RegardFil::Ecran]).unwrap());
    Joue {
        journal: partie.journal().clone(),
        coeur: partie.coeur().clone(),
        etat: etat(&partie).clone(),
        ecran,
        tours_inacheves,
        pendant,
        avant_la_fin,
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, .. ProptestConfig::default() })]

    /// CAT-14, PHASE-3 P7 : le même script, joué d'un seul tour par avancée ou découpé
    /// à 1, 2 ou 3 entrées par tour, avec des gestes et des tenus qui arrivent pendant
    /// une avancée inachevée, à n'importe lequel de ses pas et avant son saut, des
    /// arriérés, des refus et des paniques, donne le même journal, ligne pour ligne, le
    /// même cœur, le même état du jeu et les mêmes vues de l'écran ; sans fenêtre, ou
    /// avec une fenêtre et sa tête, que l'écran regarde.
    ///
    /// La première version n'injectait d'arrivées que pendant le dernier pas d'une
    /// avancée, sans saut après lui, et ne comparait pas les vues de l'écran : elle
    /// cachait une tête restée fausse pour de bon (vérification du lot 1, constat 1 ;
    /// `une_arrivee_au_milieu_d_une_avancee_va_apres_elle`).
    #[test]
    fn meme_journal_quel_que_soit_le_budget_de_tour(
        script in proptest::collection::vec(op(), 1..80),
        n in 1u32..4,
        fenetre in prop_oneof![Just(0u64), 1u64..5],
    ) {
        let entier = jouer_script(&script, Mode::Entier, fenetre);
        let decoupe = jouer_script(&script, Mode::Decoupe(n), fenetre);
        prop_assert_eq!(&decoupe.journal, &entier.journal);
        prop_assert_eq!(&decoupe.coeur, &entier.coeur);
        prop_assert_eq!(&decoupe.etat, &entier.etat);
        prop_assert_eq!(&decoupe.ecran, &entier.ecran);
    }
}

/// Le contrôle négatif de la propriété (`etale-mal`, PHASE-3 § 8.3) : une arrivée qui
/// entre dans le pas inachevé au lieu d'aller au suivant change le journal.
#[test]
fn etale_mal_change_le_journal() {
    let script = [
        Op::Geste {
            de: 1,
            quoi: 0,
            retard: 0,
            age: 0,
        },
        Op::Geste {
            de: 1,
            quoi: 1,
            retard: 0,
            age: 0,
        },
        Op::Geste {
            de: 2,
            quoi: 2,
            retard: 0,
            age: 0,
        },
        Op::Avancer { delta: 1, tours: 1 },
        // Reçu pendant le pas 1, découpé : fait au pas 1, il va au pas 2.
        Op::Geste {
            de: 3,
            quoi: 3,
            retard: 1,
            age: 0,
        },
        Op::Avancer { delta: 1, tours: 1 },
    ];
    let entier = jouer_script(&script, Mode::Entier, 0);
    let decoupe = jouer_script(&script, Mode::Decoupe(1), 0);
    let fautif = jouer_script(&script, Mode::EtaleMal(1), 0);
    assert!(decoupe.tours_inacheves > 0, "le pas 1 doit être découpé");
    assert!(decoupe.pendant > 0, "un geste reçu pendant le pas découpé");
    assert_eq!(decoupe.journal, entier.journal);
    assert_ne!(fautif.journal, entier.journal);
}
