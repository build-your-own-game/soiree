//! Les tests attendus du cœur (INTERFACE.md, § 8.6), sur le modèle.

use jeu_abi::{
    ApplicableFil, CauseTenu, Cle, Duree, EntreeFil, FinSaisie, Instant, Oui, Pas, PauseFil,
    Presence, RegardFil, RoleAppareil, decoder,
};
use partie::modele::{
    ActionModele, CLE_CHAMP, FabriqueModele, GesteModele, MESSAGE_DE_PANIQUE, ReglageModele,
};
use partie::{
    Arrivee, CausePanne, ComposantArrivee, Destinataire, EcranArrivee, Faute, IssueJournal,
    LieuFaute, Ligne, Partie, Placement, RefusReprise, ReponseArrivee, Tete,
};

use crate::commun::*;

fn ordinaire() -> ReglageModele {
    ReglageModele::ordinaire()
}

fn dormant() -> ReglageModele {
    let mut r = ordinaire();
    r.reveil = jeu_abi::ReveilFil::Jamais;
    r
}

// --- Le journal, les fautes, la reprise -------------------------------------

/// D3 : le journal garde les refus, et leur rejeu refait la même mémoire.
#[test]
fn refus_ecrit_au_journal_et_rejoue() {
    let mut p = ouvrir(dormant());
    let depart = p.point().unwrap();
    let av = jouer(&mut p, geste(1, GesteModele::Refuser, 1, 10));
    assert_eq!(
        av.avis,
        vec![(a(1), vec![0])],
        "l'avis va au seul appareil du geste"
    );
    let _: Vec<_> = jouer(&mut p, geste(1, GesteModele::Ajouter(2), 2, 11)).fautes;
    let issues: Vec<_> = entrees(p.journal()).into_iter().map(|e| e.issue).collect();
    assert!(matches!(issues[0], IssueJournal::Refusee { .. }));
    assert!(matches!(issues[1], IssueJournal::Acceptee { .. }));
    let empreinte = p.empreinte().unwrap();
    let mut r = Partie::reprendre(
        FabriqueModele::new(dormant()),
        ouverture(),
        &depart,
        p.journal(),
    )
    .unwrap();
    assert_eq!(r.empreinte().unwrap(), empreinte);
    assert_eq!(r.journal(), p.journal());
}

/// § 4.6 : une entrée fautive est écartée, et ni le rejeu ni la reprise ne la refont.
#[test]
fn faute_isolee_ne_se_rejoue_pas() {
    let mut p = ouvrir(dormant());
    let depart = p.point().unwrap();
    let s = 1;
    for (i, g) in [
        GesteModele::Ajouter(1),
        GesteModele::Paniquer,
        GesteModele::Ajouter(2),
    ]
    .into_iter()
    .enumerate()
    {
        let _: Placement = p
            .arriver(geste(1, g, s, 20 + i as u64), instant(s))
            .unwrap();
    }
    let av = p.avancer(pas(s)).unwrap();
    assert_eq!(etat(&p).total, 3, "la faute n'emporte que son entrée");
    assert_eq!(etat(&p).gestes, vec![20, 22]);
    assert_eq!(av.ecartes, vec![Destinataire::Appareil(a(1))]);
    assert_eq!(av.fautes.len(), 1);
    let rapport = &av.fautes[0];
    assert_eq!(
        rapport.faute,
        Faute::Panique {
            message: String::from(MESSAGE_DE_PANIQUE)
        }
    );
    assert!(matches!(rapport.lieu, LieuFaute::Entree { ordre: 1, .. }));
    assert_eq!(ecartees(p.journal()), 1);

    let f = FabriqueModele::new(dormant());
    let compteurs = std::rc::Rc::clone(&f.compteurs);
    let mut r = Partie::reprendre(f, ouverture(), &depart, p.journal()).unwrap();
    assert_eq!(r.empreinte().unwrap(), p.empreinte().unwrap());
    // Deux entrées rejouées, pas trois : l'écartée n'est pas rappelée.
    assert_eq!(compteurs.ecritures.get(), 2);
    assert_eq!(
        compteurs.restaurations.get(),
        1,
        "seule la restauration du point"
    );
}

/// § 4.6 : trois `Temps` écartés de suite mettent la partie en panne, sans couper personne.
#[test]
fn trois_temps_fautifs_mettent_en_panne() {
    let mut p = ouvrir(ordinaire());
    let av = jouer(&mut p, geste(1, GesteModele::PaniquerAuTemps, 1, 1));
    assert_eq!(av.en_panne, Oui::Non);
    let avs = avancer(&mut p, 10);
    let temps_ecartes = p
        .journal()
        .entrees()
        .filter(|e| {
            matches!(e.issue, IssueJournal::Ecartee { .. })
                && matches!(
                    decoder::<EntreeFil<'_>>(&e.octets),
                    Ok(EntreeFil::Temps { .. })
                )
        })
        .count();
    assert_eq!(temps_ecartes, 3, "le temps cesse après trois fautes");
    assert_eq!(avs.last().unwrap().en_panne, Oui::Oui);
    // Les gestes passent toujours.
    let _: partie::Avancee = jouer(&mut p, geste(1, GesteModele::Ajouter(4), 11, 2));
    assert_eq!(etat(&p).total, 4);
}

/// § 4.6, MANIFESTE (« quoi que fasse un jeu, il ne peut ni planter la partie ni la
/// figer ») : après trois `Temps` écartés, la partie repart du dernier point sain,
/// sans les entrées qui le suivent. Le temps revient, le saut perdu est dit au jeu,
/// et une reprise du journal retrouve la même partie.
#[test]
fn panne_au_temps_repart_du_dernier_point_sain() {
    let mut p = ouvrir(ordinaire());
    let depart = p.point().unwrap();
    // Rien à reprendre tant que la partie n'est pas en panne.
    assert_eq!(p.repartir().unwrap(), Oui::Non);
    let _: partie::Avancee = jouer(&mut p, geste(1, GesteModele::Ajouter(2), 1, 1));
    let _: Vec<partie::Avancee> = avancer(&mut p, 5);
    let _: partie::Avancee = jouer(&mut p, geste(1, GesteModele::PaniquerAuTemps, 6, 2));
    let avs = avancer(&mut p, 12);
    assert_eq!(avs.last().unwrap().en_panne, Oui::Oui);
    let figee = etat(&p).temps;
    let _: Vec<partie::Avancee> = avancer(&mut p, 15);
    assert_eq!(etat(&p).temps, figee, "en panne, plus de temps");

    assert_eq!(p.repartir().unwrap(), Oui::Oui);
    let avs = avancer(&mut p, 20);
    assert_eq!(avs.last().unwrap().en_panne, Oui::Non);
    // Le point sain est celui de la création : l'ajout et le geste fautif sont
    // laissés derrière, et le temps est de nouveau livré, saut compris.
    let e = etat(&p);
    assert_eq!(e.total, 0);
    assert!(e.temps > 0);
    assert!(
        e.sauts >= 15,
        "le temps perdu est dit par un saut : {}",
        e.sauts
    );
    assert_eq!(
        p.journal()
            .lignes()
            .iter()
            .filter(|l| matches!(l, Ligne::Reprise { .. }))
            .count(),
        1
    );

    // La reprise du journal depuis la création retrouve la même partie.
    let mut r = Partie::reprendre(
        FabriqueModele::new(ordinaire()),
        ouverture(),
        &depart,
        p.journal(),
    )
    .unwrap();
    assert_eq!(r.empreinte().unwrap(), p.empreinte().unwrap());
    assert_eq!(r.coeur(), p.coeur());
}

/// § 4.6 : une lecture fautive garde la dernière vue valide.
#[test]
fn faute_en_lecture_garde_la_derniere_vue() {
    let mut p = ouvrir(dormant());
    let regards = [RegardFil::Ecran, RegardFil::Appareil(1)];
    let avant = p.regarder(&regards).unwrap();
    assert_eq!(avant.len(), 2);
    let _: partie::Avancee = jouer(&mut p, geste(1, GesteModele::PaniquerEnVue, 1, 1));
    let apres = p.regarder(&regards).unwrap();
    assert_eq!(
        apres, avant,
        "les vues gardées sont celles d'avant la faute"
    );
    let av = p.avancer(pas(2)).unwrap();
    assert!(
        av.fautes
            .iter()
            .any(|f| matches!(f.lieu, LieuFaute::Lecture { appel: "Regarder" }))
    );
}

/// § 6.1 : une scène qui ne se décode pas n'est pas montrée.
#[test]
fn vue_invalide_garde_la_derniere_vue() {
    let mut p = ouvrir(dormant());
    let avant = p.regarder(&[RegardFil::Ecran]).unwrap();
    let _: partie::Avancee = jouer(&mut p, geste(1, GesteModele::VueInvalide, 1, 1));
    let apres = p.regarder(&[RegardFil::Ecran]).unwrap();
    assert_eq!(apres, avant);
    let av = p.avancer(pas(2)).unwrap();
    assert!(
        av.fautes
            .iter()
            .any(|f| matches!(f.faute, Faute::VueInvalide { .. }))
    );
}

/// § 4.5 : reprendre depuis chaque point, sur un journal de 10 000 entrées, rend la
/// même empreinte et le même cœur (SAL-158).
#[test]
fn reprise_rend_la_meme_empreinte() {
    let mut p = ouvrir(ordinaire());
    let mut points = vec![p.point().unwrap()];
    let mut id = 0;
    while p.journal().entrees().count() < 10_000 {
        let s = p.definitif().numero() + 1;
        for de in 1..=3 {
            id += 1;
            let _: Placement = p
                .arriver(geste(de, GesteModele::Ajouter(1), s, id), instant(s))
                .unwrap();
        }
        let _: partie::Avancee = p.avancer(pas(s)).unwrap();
        if s.is_multiple_of(500) {
            points.push(p.point().unwrap());
        }
    }
    assert!(p.journal().entrees().count() >= 10_000);
    assert!(points.len() >= 5);
    let empreinte = p.empreinte().unwrap();
    for pt in &points {
        let mut r = Partie::reprendre(
            FabriqueModele::new(ordinaire()),
            ouverture(),
            pt,
            p.journal(),
        )
        .unwrap();
        assert_eq!(
            r.empreinte().unwrap(),
            empreinte,
            "depuis le pas {:?}",
            pt.pas()
        );
        assert_eq!(r.coeur(), p.coeur());
        assert_eq!(r.journal(), p.journal());
    }
}

/// § 4.5 : un rejeu qui ne retrouve pas l'empreinte notée est une panne de l'outil.
#[test]
fn divergence_de_rejeu_est_une_panne_de_l_outil() {
    let mut p = ouvrir(dormant());
    let depart = p.point().unwrap();
    for s in 1..=300 {
        let _: Placement = p
            .arriver(geste(1, GesteModele::Ajouter(1), s, s), instant(s))
            .unwrap();
        let _: partie::Avancee = p.avancer(pas(s)).unwrap();
    }
    let mut autre = dormant();
    autre.divergent = Oui::Oui;
    let refus = Partie::reprendre(
        FabriqueModele::new(autre),
        ouverture(),
        &depart,
        p.journal(),
    )
    .err()
    .unwrap();
    match refus {
        RefusReprise::Panne(panne) => {
            assert_eq!(panne.cause, CausePanne::RejeuDivergent);
            assert!(panne.detail.contains("empreinte"), "{}", panne.detail);
        }
        autre @ (RefusReprise::Ouverture(_) | RefusReprise::PointHorsJournal) => {
            panic!("attendu une panne de rejeu, obtenu {autre:?}")
        }
    }
}

/// Une fenêtre nulle se joue comme sans fenêtre : une seule instance. La fenêtre
/// non nulle a ses tests dans `fenetre.rs`.
#[test]
fn fenetre_nulle_comme_sans_fenetre() {
    let mut nulle = ordinaire();
    nulle.fenetre = ApplicableFil::Oui(Duree::ZERO);
    let mut p = Partie::ouvrir(FabriqueModele::new(nulle), ouverture()).unwrap();
    let _: partie::Avancee = p.avancer(pas(10)).unwrap();
    assert_eq!(p.definitif(), pas(10));
    assert_eq!(p.tete(), Tete::Definitive);
}

/// § 4.2 : sans fenêtre, une seule instance, et aucun instantané hors des points.
#[test]
fn sans_fenetre_une_seule_instance() {
    let f = FabriqueModele::new(ordinaire());
    let c = std::rc::Rc::clone(&f.compteurs);
    let mut o = ouverture();
    o.point_tous_les = 100;
    let mut p = Partie::ouvrir(f, o).unwrap();
    assert_eq!(p.tete(), Tete::Definitive);
    // L'ouverture prend deux instantanés : le point de la création, avant la lecture
    // de l'état, puis celui d'après.
    assert_eq!(c.instantanes.get(), 2);
    for s in 1..=1000 {
        let _: Placement = p
            .arriver(geste(1, GesteModele::Ajouter(1), s, s), instant(s))
            .unwrap();
        let _: partie::Avancee = p.avancer(pas(s)).unwrap();
        assert_eq!(c.vivantes.get(), 1);
    }
    assert_eq!(c.fabriquees.get(), 1);
    assert_eq!(
        c.instantanes.get(),
        2 + 10,
        "un point tous les 100 pas, rien d'autre"
    );
    assert_eq!(c.restaurations.get(), 0);
}

// --- Le temps ----------------------------------------------------------------

/// CAT-15, SQ-62 : après un gel, le cœur rattrape au plus `rattrapage` pas, puis le
/// numéro saute, et le premier `Temps` après le saut porte la durée jetée.
#[test]
fn gel_rattrape_au_plus_puis_saute() {
    let mut p = ouvrir(ordinaire());
    let av = p.avancer(pas(1000)).unwrap();
    assert_eq!(av.definitif, pas(1000));
    assert_eq!(av.saut, Duree::pas(997));
    assert_eq!(etat(&p).temps, 3, "trois pas rattrapés, pas un de plus");
    assert!(p.journal().lignes().contains(&Ligne::Saut {
        de: pas(3),
        a: pas(1000)
    }));
    let _: partie::Avancee = p.avancer(pas(1001)).unwrap();
    assert_eq!(temps(p.journal()).last(), Some(&(pas(1001), 997)));
    assert_eq!(etat(&p).sauts, 997);
    let _: partie::Avancee = p.avancer(pas(1002)).unwrap();
    assert_eq!(
        temps(p.journal()).last(),
        Some(&(pas(1002), 0)),
        "le saut n'est dit qu'une fois"
    );
}

/// CAT-14 : le même temps, découpé autrement sous le plafond de rattrapage, donne la
/// même partie.
#[test]
fn meme_temps_autrement_decoupe_meme_partie() {
    let mut un = ouvrir(ordinaire());
    let mut trois = ouvrir(ordinaire());
    for bloc in 0..20u64 {
        let s = bloc * 3 + 1;
        for p in [&mut un, &mut trois] {
            let _: Placement = p
                .arriver(geste(1, GesteModele::Ajouter(1), s, s), instant(s))
                .unwrap();
            let _: Placement = p
                .arriver(tenu(2, 0, (bloc % 4) as u8, bloc + 1, s), instant(s))
                .unwrap();
        }
        avancer(&mut un, s + 2);
        let _: partie::Avancee = trois.avancer(pas(s + 2)).unwrap();
    }
    assert_eq!(un.journal(), trois.journal());
    assert_eq!(un.empreinte().unwrap(), trois.empreinte().unwrap());
}

// --- Placer et ordonner ------------------------------------------------------

/// SAL-129 : un appareil ne peut pas antidater au-delà de sa demi-latence et de la marge.
#[test]
fn antidatage_borne() {
    let mut r = dormant();
    r.antidatage_max = Duree::pas(2);
    let mut p = ouvrir(r);
    avancer(&mut p, 100);
    let arrivee = Arrivee::Geste {
        de: a(1),
        octets: octets(&GesteModele::Ajouter(1)),
        fait: instant(10),
        vu: pas(10),
        id: jeu_abi::IdGeste::du_fil(7),
        demi_aller_retour: Duree::pas(3),
    };
    let _: Placement = p.arriver(arrivee, instant(101)).unwrap();
    avancer(&mut p, 101);
    assert_eq!(quand_du_geste(p.journal(), 7).fait, 101 - 3 - 2);
    // Dans les bornes, l'instant annoncé est gardé.
    let arrivee = Arrivee::Geste {
        de: a(1),
        octets: octets(&GesteModele::Ajouter(1)),
        fait: instant(100),
        vu: pas(10),
        id: jeu_abi::IdGeste::du_fil(8),
        demi_aller_retour: Duree::pas(3),
    };
    let _: Placement = p.arriver(arrivee, instant(102)).unwrap();
    avancer(&mut p, 102);
    assert_eq!(quand_du_geste(p.journal(), 8).fait, 100);
}

/// CAT-27 : à instant égal, l'ordre ne dépend pas de la valeur des appareils.
#[test]
fn egalite_de_fait_ne_depend_pas_de_l_appareil() {
    let ordre = |numeros: [u64; 4]| {
        let mut p = ouvrir(dormant());
        for (i, n) in numeros.into_iter().enumerate() {
            let _: Placement = p
                .arriver(
                    geste(n, GesteModele::Ajouter(1), 1, 100 + i as u64),
                    instant(1),
                )
                .unwrap();
        }
        avancer(&mut p, 1);
        etat(&p).gestes.clone()
    };
    let premier = ordre([1, 2, 3, 4]);
    assert_eq!(ordre([900, 5, 77, 31]), premier);
    assert_eq!(ordre([u64::MAX, 0, 12, 13]), premier);
}

/// PUN-38, SAL-115 : un appareil ne peut pas prétendre avoir vu une vue qu'on ne lui a
/// pas envoyée.
#[test]
fn vu_borne_par_la_derniere_vue() {
    let mut p = ouvrir(dormant());
    let arrivee = |id: u64, vu: u64| Arrivee::Geste {
        de: a(1),
        octets: octets(&GesteModele::Ajouter(1)),
        fait: instant(1),
        vu: pas(vu),
        id: jeu_abi::IdGeste::du_fil(id),
        demi_aller_retour: Duree::ZERO,
    };
    let _: Placement = p.arriver(arrivee(1, 50), instant(1)).unwrap();
    avancer(&mut p, 5);
    assert_eq!(quand_du_geste(p.journal(), 1).vu, 0, "aucune vue envoyée");
    let _: Vec<partie::Vue> = p.regarder(&[RegardFil::Appareil(1)]).unwrap();
    avancer(&mut p, 10);
    let _: Placement = p.arriver(arrivee(2, 50), instant(11)).unwrap();
    let _: Placement = p.arriver(arrivee(3, 4), instant(11)).unwrap();
    avancer(&mut p, 11);
    assert_eq!(
        quand_du_geste(p.journal(), 2).vu,
        5,
        "borné par la vue du pas 5"
    );
    assert_eq!(
        quand_du_geste(p.journal(), 3).vu,
        4,
        "une vue plus vieille est gardée"
    );
}

/// L'ordre d'un appareil est gardé, même s'il annonce des instants qui reculent.
#[test]
fn l_ordre_d_un_appareil_est_garde() {
    let mut p = ouvrir(dormant());
    let _: Placement = p
        .arriver(geste(1, GesteModele::Ajouter(1), 5, 1), instant(5))
        .unwrap();
    let _: Placement = p
        .arriver(geste(1, GesteModele::Ajouter(1), 3, 2), instant(5))
        .unwrap();
    avancer(&mut p, 5);
    assert_eq!(etat(&p).gestes, vec![1, 2]);
}

// --- Les tenus ---------------------------------------------------------------

fn avec_tenu(fraicheur: Option<u64>, age: Option<u64>, debit: u16) -> ReglageModele {
    let mut r = dormant();
    r.tenu.fraicheur = fraicheur.map_or(
        ApplicableFil::SansObjet(String::from("jamais relâché seul ici")),
        |f| ApplicableFil::Oui(Duree::pas(f)),
    );
    r.tenu.age_max = age.map_or(
        ApplicableFil::SansObjet(String::from("aucun âge borné ici")),
        |f| ApplicableFil::Oui(Duree::pas(f)),
    );
    r.tenu.debit_max = debit;
    r
}

/// REM-14 : un état d'un numéro plus petit que le dernier reçu est écarté.
#[test]
fn tenu_numero_ancien_ecarte() {
    let mut p = ouvrir(avec_tenu(None, None, 60));
    let _: partie::Avancee = jouer(&mut p, tenu(1, 0, 5, 10, 1));
    assert_eq!(
        p.arriver(tenu(1, 0, 3, 9, 2), instant(2)).unwrap(),
        Placement::Perimee
    );
    assert_eq!(
        p.arriver(tenu(1, 0, 3, 10, 2), instant(2)).unwrap(),
        Placement::Perimee
    );
    avancer(&mut p, 3);
    assert_eq!(etat(&p).tenus, vec![(1, 0, 5)]);
    // Deux anciens arrivés dans le même pas que le récent : le récent gagne.
    let _: Placement = p.arriver(tenu(1, 0, 7, 12, 4), instant(4)).unwrap();
    let _: Placement = p.arriver(tenu(1, 0, 6, 11, 4), instant(4)).unwrap();
    avancer(&mut p, 4);
    assert_eq!(etat(&p).tenus, vec![(1, 0, 7)]);
}

/// REM-13, PUN-03 : au plus `debit_max` changements par seconde, et le dernier gagne.
#[test]
fn tenu_dernier_gagne_au_debit() {
    let mut p = ouvrir(avec_tenu(None, None, 10));
    for n in 1..=30u64 {
        let _: Placement = p
            .arriver(
                tenu(1, 0, u8::try_from(n % 7 + 1).unwrap(), n, n),
                instant(n),
            )
            .unwrap();
        avancer(&mut p, n);
    }
    avancer(&mut p, 60);
    let livres: Vec<Pas> = p
        .journal()
        .entrees()
        .filter(|e| {
            matches!(
                decoder::<EntreeFil<'_>>(&e.octets),
                Ok(EntreeFil::Tenu { .. })
            )
        })
        .map(|e| e.place)
        .collect();
    for paire in livres.windows(2) {
        assert!(paire[1].numero() - paire[0].numero() >= 6, "{livres:?}");
    }
    assert_eq!(etat(&p).tenus, vec![(1, 0, 30 % 7 + 1)], "le dernier gagne");
}

/// SQ-02, CAT-25 : un état trop vieux ne fait bouger personne ; un relâchement passe.
#[test]
fn tenu_trop_vieux_ecarte_sauf_relache() {
    let mut r = avec_tenu(None, Some(30), 60);
    r.antidatage_max = Duree::pas(1000);
    let mut p = ouvrir(r);
    avancer(&mut p, 200);
    let vieux = |valeur: u8, numero: u64| Arrivee::Tenu {
        de: a(1),
        octets: octets(&partie::modele::TenuModele { cle: 0, valeur }),
        numero,
        fait: instant(100),
        demi_aller_retour: Duree::pas(500),
    };
    let _: partie::Avancee = jouer(&mut p, vieux(9, 1));
    assert_eq!(etat(&p).tenus, vec![], "l'état vieux est écarté");
    let _: partie::Avancee = jouer(&mut p, vieux(0, 2));
    assert_eq!(
        etat(&p).tenus,
        vec![(1, 0, 0)],
        "le relâchement vieux passe"
    );
    let _: partie::Avancee = jouer(&mut p, tenu(1, 0, 4, 3, 203));
    assert_eq!(etat(&p).tenus, vec![(1, 0, 4)], "un état frais passe");
}

/// SQ-01 : une valeur tenue se relâche seule quand l'appareil se tait plus que
/// `fraicheur` ; à `fraicheur` pile, elle tient encore (B6 de Sidequest).
#[test]
fn tenu_silence_relache() {
    let mut p = ouvrir(avec_tenu(Some(27), None, 60));
    let _: partie::Avancee = jouer(&mut p, tenu(1, 0, 5, 1, 1));
    avancer(&mut p, 28);
    assert_eq!(etat(&p).tenus, vec![(1, 0, 5)]);
    avancer(&mut p, 29);
    assert_eq!(etat(&p).tenus, vec![(1, 0, 0)]);
    assert_eq!(
        etat(&p).changements.last(),
        Some(&(1, 0, 0, CauseTenu::Silence))
    );
    avancer(&mut p, 100);
    assert_eq!(etat(&p).changements.len(), 2, "un seul relâchement");
}

/// Un départ relâche ce que l'appareil tenait, avant que le jeu lise le départ.
#[test]
fn tenu_depart_relache() {
    let mut p = ouvrir(avec_tenu(None, None, 60));
    let _: partie::Avancee = jouer(&mut p, tenu(1, 0, 5, 1, 1));
    let _: partie::Avancee = jouer(&mut p, tenu(1, 1, 2, 2, 2));
    let _: partie::Avancee = jouer(
        &mut p,
        Arrivee::Presence(Presence::Depart {
            appareil: a(1),
            cause: jeu_abi::CauseDepart::Silence,
        }),
    );
    let fin: Vec<_> = etat(&p).changements.iter().rev().take(2).copied().collect();
    assert!(fin.contains(&(1, 0, 0, CauseTenu::Depart)));
    assert!(fin.contains(&(1, 1, 0, CauseTenu::Depart)));
}

/// § 4.1 : une pause du commun relâche tout, et le temps ne passe plus.
#[test]
fn pause_du_commun_relache_tout() {
    let mut r = avec_tenu(None, None, 60);
    r.reveil = jeu_abi::ReveilFil::ChaquePas;
    let mut p = ouvrir(r);
    let _: partie::Avancee = jouer(&mut p, tenu(1, 0, 5, 1, 1));
    let _: partie::Avancee = jouer(
        &mut p,
        Arrivee::Manette {
            de: a(2),
            etat: manette_tenue(),
            numero: 1,
            fait: instant(2),
        },
    );
    let temps_avant = etat(&p).temps;
    assert_eq!(p.pause(Oui::Oui).unwrap(), Oui::Oui);
    avancer(&mut p, 10);
    assert_eq!(etat(&p).temps, temps_avant, "pas de temps pendant la pause");
    assert_eq!(etat(&p).tenus, vec![(1, 0, 0)]);
    assert_eq!(
        etat(&p).changements.last(),
        Some(&(1, 0, 0, CauseTenu::Pause))
    );
    assert_eq!(
        etat(&p).manettes[0].1,
        manette_tenue().relachee(),
        "relâchée, appuis gardés (REM-16)"
    );
    assert_eq!(p.pause(Oui::Non).unwrap(), Oui::Oui);
    avancer(&mut p, 12);
    assert_eq!(etat(&p).temps, temps_avant + 2);
    // Sans pause du commun déclarée, rien ne change.
    let mut r = ordinaire();
    r.pause = PauseFil::DuJeu;
    let mut q = ouvrir(r);
    assert_eq!(q.pause(Oui::Oui).unwrap(), Oui::Non);
}

fn manette_tenue() -> jeu_abi::Manette {
    let mut m = jeu_abi::Manette::REPOS;
    m.boutons[0] = jeu_abi::Bouton {
        etat: jeu_abi::Appui::Tenu,
        appuis: jeu_abi::Cumul::new(1),
    };
    m
}

// --- Les demandes ------------------------------------------------------------

/// TL-45, SAL-164 : la réponse à une demande retirée est jetée.
#[test]
fn reponse_d_une_demande_retiree_jetee() {
    let mut p = ouvrir(dormant());
    let av = jouer(&mut p, geste(1, GesteModele::Demander, 1, 1));
    assert_eq!(av.lancer.len(), 1);
    let id = av.lancer[0].0;
    // Une réponse placée, puis la demande retirée avant qu'elle ne soit jouée.
    let _: Placement = p
        .arriver(
            Arrivee::Reponse {
                id,
                reponse: ReponseArrivee::Fait(Ok(())),
            },
            instant(3),
        )
        .unwrap();
    let av = jouer(&mut p, geste(1, GesteModele::Abandonner, 2, 2));
    assert_eq!(av.annuler, vec![id]);
    avancer(&mut p, 5);
    assert_eq!(etat(&p).reponses, Vec::<u64>::new());
    assert_eq!(
        etat(&p).reponses_inattendues,
        0,
        "la réponse n'a pas atteint le jeu"
    );
    // Arrivée après le retrait : écartée dès l'arrivée.
    let r = p
        .arriver(
            Arrivee::Reponse {
                id,
                reponse: ReponseArrivee::Fait(Ok(())),
            },
            instant(6),
        )
        .unwrap();
    assert_eq!(r, Placement::Perimee);
}

/// Une réponse à une demande encore listée atteint le jeu.
#[test]
fn reponse_d_une_demande_listee_livree() {
    let mut p = ouvrir(dormant());
    let av = jouer(&mut p, geste(1, GesteModele::Demander, 1, 1));
    let id = av.lancer[0].0;
    let av = jouer(
        &mut p,
        Arrivee::Reponse {
            id,
            reponse: ReponseArrivee::Fait(Ok(())),
        },
    );
    assert_eq!(etat(&p).reponses, vec![id.fil()]);
    assert!(
        av.annuler.is_empty(),
        "une demande répondue n'est pas annulée"
    );
    assert_eq!(p.demandes_en_cours(), Vec::new());
}

/// CAT-76 : une demande ne part qu'une fois, même à travers une reprise.
#[test]
fn demande_lancee_une_fois_meme_apres_reprise() {
    let mut p = ouvrir(dormant());
    let depart = p.point().unwrap();
    let av = jouer(&mut p, geste(1, GesteModele::Demander, 1, 1));
    assert_eq!(av.lancer.len(), 1);
    let apres = p.point().unwrap();
    for pt in [&depart, &apres] {
        let mut r = Partie::reprendre(FabriqueModele::new(dormant()), ouverture(), pt, p.journal())
            .unwrap();
        let av = r.avancer(pas(5)).unwrap();
        assert!(av.lancer.is_empty(), "rien ne repart après la reprise");
        assert_eq!(r.demandes_en_cours(), p.demandes_en_cours());
        // Une demande neuve, elle, part.
        let av = jouer(&mut r, geste(1, GesteModele::Demander, 6, 2));
        assert_eq!(av.lancer.len(), 1);
        assert_ne!(av.lancer[0].0, p.demandes_en_cours()[0]);
    }
}

/// CAT-76, TL-45 : le compteur d'identités du jeu repart de 1 sur l'instance neuve
/// d'une nouvelle partie. La demande 1 de la nouvelle partie part quand même, sous une
/// autre identité que celle de la partie d'avant, et une réponse tardive à l'ancienne
/// est périmée au lieu d'être livrée à la nouvelle. C'est la règle que demande le
/// besoin 11 des jeux (Sidequest B7) : une demande en vol à une nouvelle partie est
/// annulée.
#[test]
fn demande_d_une_nouvelle_partie_part_et_l_ancienne_reponse_est_perimee() {
    let mut p = ouvrir(dormant());
    let av = jouer(&mut p, geste(1, GesteModele::Demander, 1, 1));
    let ancienne = av.lancer[0].0;
    let av = jouer(&mut p, geste(1, GesteModele::NouvellePartie, 2, 2));
    assert_eq!(av.annuler, vec![ancienne]);
    let av = jouer(&mut p, geste(1, GesteModele::Demander, 3, 3));
    assert_eq!(av.lancer.len(), 1, "la demande de la nouvelle partie part");
    let nouvelle = av.lancer[0].0;
    assert_ne!(nouvelle, ancienne);
    assert_eq!(p.demandes_en_cours(), vec![nouvelle]);
    let r = p
        .arriver(
            Arrivee::Reponse {
                id: ancienne,
                reponse: ReponseArrivee::Fait(Ok(())),
            },
            instant(4),
        )
        .unwrap();
    assert_eq!(r, Placement::Perimee);
    let _: partie::Avancee = jouer(
        &mut p,
        Arrivee::Reponse {
            id: nouvelle,
            reponse: ReponseArrivee::Fait(Ok(())),
        },
    );
    assert_eq!(
        etat(&p).reponses.len(),
        1,
        "la réponse à la nouvelle est livrée"
    );
    assert_eq!(p.demandes_en_cours(), Vec::new());
}

// --- Les sorties -------------------------------------------------------------

/// D10 : une sortie s'exécute au pas définitif, et l'appareil oublié revient comme un neuf.
#[test]
fn sortie_executee_au_definitif_seulement() {
    let mut p = ouvrir(dormant());
    let _: Placement = p
        .arriver(geste(1, GesteModele::Oublier, 3, 1), instant(3))
        .unwrap();
    let av = p.avancer(pas(2)).unwrap();
    assert!(av.oublier.is_empty(), "le geste n'est pas encore joué");
    let av = p.avancer(pas(3)).unwrap();
    assert_eq!(av.oublier, vec![a(1)]);
    let av = jouer(&mut p, geste(2, GesteModele::Retirer, 4, 2));
    assert_eq!(av.retirer, vec![a(2)]);
}

/// Besoin 12 des jeux (Sidequest B8 ; PUN-15, SQ-76) : un appareil oublié qui revient
/// est annoncé par une arrivée, comme un neuf, sous le même `Appareil` : le cœur ne
/// renomme jamais un appareil, il joue celui que le transport lui donne. Ses gestes
/// atteignent le jeu de nouveau.
#[test]
fn appareil_oublie_revient_sous_le_meme_appareil() {
    let mut p = ouvrir(dormant());
    let arrivee = || {
        Arrivee::Presence(Presence::Arrivee {
            appareil: a(1),
            role: RoleAppareil::Telephone,
            capacites: jeu_abi::Capacites::AUCUNE,
        })
    };
    let _: partie::Avancee = jouer(&mut p, arrivee());
    let av = jouer(&mut p, geste(1, GesteModele::Oublier, 2, 1));
    assert_eq!(av.oublier, vec![a(1)]);
    let _: partie::Avancee = jouer(
        &mut p,
        Arrivee::Presence(Presence::Depart {
            appareil: a(1),
            cause: jeu_abi::CauseDepart::Fermeture,
        }),
    );
    assert_eq!(etat(&p).presents, Vec::<u64>::new());
    let _: partie::Avancee = jouer(&mut p, arrivee());
    assert_eq!(
        etat(&p).presents,
        vec![1],
        "il revient sous le même appareil"
    );
    let _: partie::Avancee = jouer(&mut p, geste(1, GesteModele::Ajouter(4), 5, 2));
    assert_eq!(etat(&p).total, 4, "son geste atteint le jeu");
}

/// CAT-73, CAT-75 : une nouvelle partie est une instance neuve, avec les mêmes
/// appareils et ce que le jeu garde ; l'ancienne mémoire est rendue.
#[test]
fn nouvelle_partie_instance_neuve_memoire_rendue() {
    let f = FabriqueModele::new(dormant());
    let c = std::rc::Rc::clone(&f.compteurs);
    let mut p = Partie::ouvrir(f, ouverture()).unwrap();
    let depart = p.point().unwrap();
    let _: partie::Avancee = jouer(
        &mut p,
        Arrivee::Presence(Presence::Arrivee {
            appareil: a(4),
            role: RoleAppareil::Telephone,
            capacites: jeu_abi::Capacites::AUCUNE,
        }),
    );
    let _: partie::Avancee = jouer(&mut p, geste(4, GesteModele::Ajouter(9), 2, 1));
    let _: partie::Avancee = jouer(&mut p, geste(4, GesteModele::Demander, 3, 2));
    let av = jouer(&mut p, geste(4, GesteModele::NouvellePartie, 4, 3));
    assert_eq!(c.fabriquees.get(), 2);
    assert_eq!(c.vivantes.get(), 1, "l'ancienne instance est rendue");
    assert_eq!(etat(&p).total, 0);
    assert_eq!(etat(&p).garde, 9);
    assert_eq!(etat(&p).presents, vec![4]);
    assert_eq!(
        av.annuler.len(),
        1,
        "les demandes de l'ancienne partie sont annulées"
    );
    assert!(
        p.journal()
            .lignes()
            .iter()
            .any(|l| matches!(l, Ligne::Partie { numero: 1, .. }))
    );
    let _: partie::Avancee = jouer(&mut p, geste(4, GesteModele::Ajouter(1), 5, 4));
    let derniere = p.journal().entrees().last().unwrap().clone();
    assert_eq!(
        (derniere.partie, derniere.ordre),
        (1, 0),
        "l'ordre repart à zéro"
    );
    // La reprise depuis la première partie traverse la nouvelle.
    let mut r = Partie::reprendre(
        FabriqueModele::new(dormant()),
        ouverture(),
        &depart,
        p.journal(),
    )
    .unwrap();
    assert_eq!(r.empreinte().unwrap(), p.empreinte().unwrap());
    assert_eq!(r.coeur(), p.coeur());
}

// --- L'écran -----------------------------------------------------------------

/// SAL-152 : une action que la scène affichée ne portait pas n'atteint pas le jeu.
#[test]
fn action_ecran_hors_scene_refusee() {
    let mut p = ouvrir(dormant());
    let remettre = || Arrivee::Ecran {
        quoi: EcranArrivee::Action(octets(&ActionModele::Remettre)),
        vu: pas(0),
    };
    assert_eq!(
        p.arriver(remettre(), instant(1)).unwrap(),
        Placement::HorsScene,
        "aucune scène envoyée"
    );
    let _: Vec<partie::Vue> = p.regarder(&[RegardFil::Ecran]).unwrap();
    assert!(matches!(
        p.arriver(remettre(), instant(1)).unwrap(),
        Placement::Place { .. }
    ));
    let autre = Arrivee::Ecran {
        quoi: EcranArrivee::Action(vec![1]),
        vu: pas(0),
    };
    assert_eq!(p.arriver(autre, instant(1)).unwrap(), Placement::HorsScene);
    let touche = Arrivee::Ecran {
        quoi: EcranArrivee::Touche(jeu_abi::Touche {
            code: jeu_abi::CodeTouche::new("KeyA"),
            mods: jeu_abi::Mods::AUCUN,
            phase: jeu_abi::PhaseTouche::Enfoncee,
            repetition: Oui::Non,
        }),
        vu: pas(0),
    };
    assert_eq!(
        p.arriver(touche, instant(1)).unwrap(),
        Placement::HorsScene,
        "pas de clavier brut"
    );
}

/// SAL-152, TL-27 : une saisie au-delà de la limite du champ est refusée.
#[test]
fn saisie_au_dela_de_la_limite_refusee() {
    let mut p = ouvrir(dormant());
    let _: Vec<partie::Vue> = p.regarder(&[RegardFil::Ecran]).unwrap();
    let saisie = |texte: &str, champ: Cle| Arrivee::Ecran {
        quoi: EcranArrivee::Composant(ComposantArrivee::Saisie {
            champ,
            texte: String::from(texte),
            fin: FinSaisie::EnCours,
        }),
        vu: pas(0),
    };
    assert_eq!(
        p.arriver(saisie("neuf lettr", CLE_CHAMP), instant(1))
            .unwrap(),
        Placement::HorsScene
    );
    assert_eq!(
        p.arriver(saisie("ok", Cle::du_fil(99)), instant(1))
            .unwrap(),
        Placement::HorsScene
    );
    assert!(matches!(
        p.arriver(saisie("huit éàç", CLE_CHAMP), instant(1))
            .unwrap(),
        Placement::Place { .. }
    ));
    avancer(&mut p, 1);
    assert_eq!(etat(&p).saisies, 1);
}

// --- Les limites -------------------------------------------------------------

/// Un geste au-delà de `limites.geste`, ou qui ne se lit pas, n'entre pas au journal.
#[test]
fn geste_malforme_ou_trop_long_n_entre_pas() {
    let mut r = dormant();
    r.limites.geste = 4;
    let mut p = ouvrir(r);
    let long = Arrivee::Geste {
        de: a(1),
        octets: vec![0; 5],
        fait: instant(1),
        vu: pas(0),
        id: jeu_abi::IdGeste::du_fil(1),
        demi_aller_retour: Duree::ZERO,
    };
    assert!(matches!(
        p.arriver(long, instant(1)).unwrap(),
        Placement::Malforme(_)
    ));
    let illisible = Arrivee::Geste {
        de: a(1),
        octets: vec![200],
        fait: instant(1),
        vu: pas(0),
        id: jeu_abi::IdGeste::du_fil(2),
        demi_aller_retour: Duree::ZERO,
    };
    assert!(matches!(
        p.arriver(illisible, instant(1)).unwrap(),
        Placement::Malforme(_)
    ));
    avancer(&mut p, 3);
    assert_eq!(p.journal().entrees().count(), 0);
}

/// `limites.file` : un appareil ne remplit pas la file à lui seul.
#[test]
fn file_pleine() {
    let mut r = dormant();
    r.limites.file = 2;
    let mut p = ouvrir(r);
    for id in 0..2 {
        assert!(matches!(
            p.arriver(geste(1, GesteModele::Ajouter(1), 1, id), instant(1))
                .unwrap(),
            Placement::Place { .. }
        ));
    }
    assert_eq!(
        p.arriver(geste(1, GesteModele::Ajouter(1), 1, 3), instant(1))
            .unwrap(),
        Placement::FilePleine
    );
    assert!(matches!(
        p.arriver(geste(2, GesteModele::Ajouter(1), 1, 4), instant(1))
            .unwrap(),
        Placement::Place { .. }
    ));
}

/// § 2.7 : `Attente` sur la partie qui fait foi est une faute du jeu.
#[test]
fn attente_sur_la_definitive_est_une_faute() {
    let mut p = ouvrir(dormant());
    let av = jouer(&mut p, geste(1, GesteModele::Attendre, 1, 1));
    assert_eq!(av.fautes[0].faute, Faute::Attente);
    assert_eq!(ecartees(p.journal()), 1);
}

/// Un budget épuisé est une faute, rejouée à l'identique (SQ-59) : écartée.
#[test]
fn budget_epuise_ecarte_l_entree() {
    let mut p = ouvrir(dormant());
    let av = jouer(&mut p, geste(1, GesteModele::Boucler, 1, 1));
    assert_eq!(av.fautes[0].faute, Faute::Budget);
    assert_eq!(av.fautes[0].carburant, Some(1_000));
    let _: partie::Avancee = jouer(&mut p, geste(1, GesteModele::Ajouter(1), 2, 2));
    assert_eq!(etat(&p).total, 1);
}

/// Le temps n'est livré que si le réveil le demande.
#[test]
fn reveil_jamais_ne_livre_pas_le_temps() {
    let mut p = ouvrir(ordinaire());
    avancer(&mut p, 3);
    assert_eq!(etat(&p).temps, 3);
    let _: partie::Avancee = jouer(&mut p, geste(1, GesteModele::Dormir, 4, 1));
    avancer(&mut p, 20);
    assert_eq!(
        etat(&p).temps,
        3,
        "le réveil est lu après les entrées du pas : plus de Temps dès le pas 4"
    );
    let _: partie::Avancee = jouer(&mut p, geste(1, GesteModele::Reveiller, 21, 2));
    avancer(&mut p, 25);
    assert_eq!(
        etat(&p).temps,
        3 + 5,
        "le Temps revient dès le pas du réveil"
    );
}

/// Après une faute, la reprise ne rapporte pas la faute une seconde fois.
#[test]
fn reprise_ne_redit_rien() {
    let mut p = ouvrir(dormant());
    let depart = p.point().unwrap();
    let _: partie::Avancee = jouer(&mut p, geste(1, GesteModele::Paniquer, 1, 1));
    let _: partie::Avancee = jouer(&mut p, geste(1, GesteModele::Refuser, 2, 2));
    let mut r = Partie::reprendre(
        FabriqueModele::new(dormant()),
        ouverture(),
        &depart,
        p.journal(),
    )
    .unwrap();
    let av = r.avancer(pas(3)).unwrap();
    assert!(av.fautes.is_empty() && av.avis.is_empty() && av.ecartes.is_empty());
}

/// Un point au-delà du journal est refusé.
#[test]
fn point_hors_journal_refuse() {
    let mut p = ouvrir(dormant());
    let _: partie::Avancee = jouer(&mut p, geste(1, GesteModele::Ajouter(1), 1, 1));
    let pt = p.point().unwrap();
    let q = ouvrir(dormant());
    let refus = Partie::reprendre(
        FabriqueModele::new(dormant()),
        ouverture(),
        &pt,
        q.journal(),
    )
    .err()
    .unwrap();
    assert_eq!(refus, RefusReprise::PointHorsJournal);
}

/// Les deux pas de la soirée sont distincts de l'heure : `Temps` porte l'heure déduite du pas.
#[test]
fn l_heure_du_temps_se_deduit_du_pas() {
    let mut p = ouvrir(ordinaire());
    avancer(&mut p, 60);
    let derniere = p.journal().entrees().last().unwrap().clone();
    match decoder::<EntreeFil<'_>>(&derniere.octets).unwrap() {
        EntreeFil::Temps { heure, .. } => assert_eq!(heure, 1_000_000 + 1_000),
        autre @ (EntreeFil::Ecran { .. }
        | EntreeFil::Geste { .. }
        | EntreeFil::Tenu { .. }
        | EntreeFil::Manette { .. }
        | EntreeFil::Presence(_)
        | EntreeFil::Reponse { .. }) => panic!("{autre:?}"),
    }
    let _: Instant = instant(0);
}
