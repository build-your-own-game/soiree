//! `devine` joué par la partie qui fait foi sur ses deux instances (INTERFACE.md,
//! § 8.9, étape 3) : la copie native (`jeu-natif`) et le module compilé pour
//! `wasm32-unknown-unknown`, réécrit et chargé par `moteur-wasmtime`.
//!
//! Une soirée complète (`soiree.rs`), avec des fautes injectées et réelles, un gel,
//! une demande, un départ et un retour, est menée du même pas sur les deux parties
//! (`duo.rs`) : même histoire, même état, à chaque pas. Puis son journal est rejoué
//! entrée par entrée sur des instances nues (`rejeu.rs`), et la partie reprend de
//! trois points de reprise sur chaque instance.
//!
//! Ces tests vivent dans `epreuve`, la seule crate qui voit à la fois le moteur, la
//! copie native et `devine` (§ 8, le graphe) ; ils sont le noyau de E9.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::wildcard_enum_match_arm,
    reason = "code de test : un échec s'y lit comme l'échec du test qui l'appelle"
)]

mod brut;
mod duo;
mod module;
mod piste;
mod rejeu;
mod soiree;

use devine::Devine;
use jeu_abi::{EntreeFil, Oui, decoder};
use jeu_natif::FabriqueNative;
use partie::{Faute, IssueJournal, LieuFaute, Partie};

use crate::duo::{Duo, ouverture};
use crate::piste::Piste;
use crate::soiree::{Cote, PANIQUE_INJECTEE, regles};

/// La soirée sur les deux parties, avec ses fautes.
fn soiree(charge: &moteur_wasmtime::ModuleCharge) -> (Duo, soiree::Bilan) {
    let mut d = Duo::ouvrir(charge, regles(Cote::Natif), regles(Cote::Module));
    // F3 est une panique réelle du module, injectée dans la copie native : leurs
    // messages diffèrent, et sont vérifiés à part.
    d.panique_libre = true;
    let b = soiree::jouer(&mut d);
    (d, b)
}

/// Les ordres des entrées écartées au journal.
fn ecartees(j: &partie::Journal) -> Vec<(u64, Faute)> {
    j.entrees()
        .filter_map(|e| match &e.issue {
            IssueJournal::Ecartee { faute } => Some((e.ordre, faute.clone())),
            _ => None,
        })
        .collect()
}

/// Les fautes de la soirée, dans l'ordre, avec leur lieu, des deux côtés ; le message
/// de la panique réelle du module (F3), et celui des paniques injectées.
fn verifier_les_fautes(d: &Duo) {
    // Les fautes, dans l'ordre, avec leur lieu.
    let lieux = |f: &[partie::RapportFaute]| -> Vec<(String, &'static str)> {
        f.iter()
            .map(|r| {
                let lieu = match &r.lieu {
                    LieuFaute::Entree { .. } => "entrée",
                    LieuFaute::Lecture { appel } => appel,
                    LieuFaute::Creation => "création",
                };
                let faute = match &r.faute {
                    Faute::Panique { .. } => String::from("panique"),
                    autre => format!("{autre:?}"),
                };
                (faute, lieu)
            })
            .collect()
    };
    let attendues = [
        ("panique", "entrée"),
        ("Budget", "entrée"),
        ("panique", "Regarder"),
        ("panique", "entrée"),
        ("panique", "entrée"),
        ("panique", "entrée"),
    ]
    .map(|(f, l)| (String::from(f), l));
    assert_eq!(lieux(&d.fautes.0), attendues);
    assert_eq!(lieux(&d.fautes.1), attendues);
    // F3 : la panique réelle du module porte le message de sa zone de panique.
    let Faute::Panique { message } = &d.fautes.1[2].faute else {
        panic!("F3 n'est pas une panique");
    };
    assert!(message.contains("memory allocation"), "{message}");
    for r in [&d.fautes.0[0], &d.fautes.1[0]] {
        assert_eq!(
            r.faute,
            Faute::Panique {
                message: String::from(PANIQUE_INJECTEE)
            }
        );
    }
}

/// § 8.9, étape 3, et § 4.6 : la même soirée sur la copie native et sur le module
/// donne la même histoire, pas après pas, et le même état ; chaque faute, injectée ou
/// réelle, est isolée de la même façon des deux côtés (HIS-06, HIS-07).
#[test]
fn devine_meme_histoire_sur_les_deux_instances() {
    let (_moteur, charge) = module::charger();
    let (mut d, bilan) = soiree(&charge);
    assert!(
        d.comparaisons > 300,
        "{} comparaisons d'état",
        d.comparaisons
    );

    verifier_les_fautes(&d);

    // Les entrées écartées : F1, F2 et trois `Temps` ; le même journal des deux côtés.
    let en = ecartees(d.natif.journal());
    assert_eq!(en, ecartees(d.module.journal()));
    assert_eq!(en.len(), 5);
    let temps_ecartes = d
        .natif
        .journal()
        .entrees()
        .filter(|e| matches!(e.issue, IssueJournal::Ecartee { .. }))
        .filter(|e| {
            matches!(
                decoder::<EntreeFil<'_>>(&e.octets),
                Ok(EntreeFil::Temps { .. })
            )
        })
        .count();
    assert_eq!(temps_ecartes, 3);
    // Trois `Temps` écartés de suite : la partie est en panne, des deux côtés, et le
    // temps n'est plus livré.
    assert_eq!(d.avancees.last().map(|a| a.en_panne), Some(Oui::Oui));
    // La demande de la première manche est partie une fois, et a reçu sa réponse.
    let lancees: usize = d.avancees.iter().map(|a| a.lancer.len()).sum();
    assert_eq!(lancees, 1);
    assert_eq!(d.natif.demandes_en_cours(), Vec::new());
    assert_eq!(
        bilan.demande,
        d.avancees
            .iter()
            .find_map(|a| a.lancer.first())
            .expect("lancée")
            .0
    );
    // Au moins une empreinte notée au journal (SAL-158).
    assert!(
        d.natif
            .journal()
            .lignes()
            .iter()
            .any(|l| matches!(l, partie::Ligne::Empreinte { .. }))
    );

    // Ce que le cœur a demandé aux instances, compté des deux côtés.
    let (rn, rm) = d.releves();
    assert_eq!(rn.declenchees, ["F1", "F2", "F3", "F4", "F4", "F4"]);
    assert_eq!(rm.declenchees, ["F1", "F2", "F4", "F4", "F4"]);
    // `sans_fenetre_une_seule_instance` (§ 8.6) : une seule instance, et aucun
    // instantané hors des points : deux à l'ouverture, les trois points demandés, et
    // un tous les 600 pas (600, 1200, 1800, 2400).
    for r in [&rn, &rm] {
        assert_eq!(r.neuves, 1);
        assert_eq!(r.instantanes, 2 + bilan.points_explicites + 4);
        // Une restauration par faute, lecture comprise.
        assert_eq!(r.restaurations, 6);
    }
    // `faute_isolee_ne_se_rejoue_pas` (§ 8.6) : une entrée écartée n'a reçu que l'appel
    // qui a fauté, même après les reconstructions qui ont suivi.
    for (ordre, _) in &en {
        assert_eq!(rn.appels_de(*ordre), 1, "ordre {ordre}, copie native");
        assert_eq!(rm.appels_de(*ordre), 1, "ordre {ordre}, module");
    }
    d.comparer("la fin de la soirée");
    // Les bords de la forme canonique (`jeu_abi::lire_canonique`) : aucun flottant non
    // fini dans l'état de `devine` (E17, CAT-05).
    let forme = d.module.canonique().expect("forme canonique");
    let bords = jeu_abi::lire_canonique(&forme).expect("une forme canonique a ses bords");
    assert_eq!(bords.non_finis, 0);
    eprintln!(
        "soirée : {} pas, {} lignes au journal dont {} entrées, {} comparaisons d'état, \
         {} instantanés et {} restaurations par instance",
        d.definitif(),
        d.natif.journal().lignes().len(),
        d.natif.journal().entrees().count(),
        d.comparaisons,
        rn.instantanes,
        rn.restaurations,
    );
}

/// § 4.5 : depuis chaque point de reprise, sur chaque instance, la reprise rejoue le
/// journal sans les entrées écartées et retrouve le même état : même cœur, même
/// empreinte que la partie d'origine, même forme canonique d'une instance à l'autre
/// (`reprise_rend_la_meme_empreinte`, `demande_lancee_une_fois_meme_apres_reprise`,
/// CAT-76).
#[test]
fn reprise_rend_le_meme_etat_sur_les_deux_instances() {
    let (_moteur, charge) = module::charger();
    let (mut d, bilan) = soiree(&charge);
    let en = ecartees(d.natif.journal());
    let forme = d.natif.canonique().expect("forme canonique");
    let (empreinte_n, empreinte_m) = (
        d.natif.empreinte().expect("empreinte"),
        d.module.empreinte().expect("empreinte"),
    );
    for (pas, (pn, pm)) in &bilan.points {
        let mut rn = Partie::reprendre(
            Piste::new(FabriqueNative::<Devine>::new(), Vec::new()),
            ouverture(),
            pn,
            d.natif.journal(),
        )
        .unwrap_or_else(|e| panic!("reprise native du pas {pas} : {e:?}"));
        let mut rm = Partie::reprendre(
            Piste::new(charge.fabrique(), Vec::new()),
            ouverture(),
            pm,
            d.module.journal(),
        )
        .unwrap_or_else(|e| panic!("reprise du module au pas {pas} : {e:?}"));
        assert!(
            rn.coeur() == d.natif.coeur(),
            "cœur de la reprise native du pas {pas}"
        );
        assert!(
            rm.coeur() == d.module.coeur(),
            "cœur de la reprise du module du pas {pas}"
        );
        assert_eq!(rn.empreinte().expect("empreinte"), empreinte_n, "pas {pas}");
        assert_eq!(rm.empreinte().expect("empreinte"), empreinte_m, "pas {pas}");
        assert!(rn.canonique().expect("forme") == forme, "pas {pas}");
        assert!(rm.canonique().expect("forme") == forme, "pas {pas}");
        // Rien ne repart : la demande de la première manche n'est pas relancée.
        let an = rn.avancer(rn.definitif()).expect("avancer");
        assert_eq!(an.lancer.len(), 0);
        // La reprise ne rappelle aucune entrée écartée, et ne restaure que son point.
        for r in [rn.fabrique().releve(), rm.fabrique().releve()] {
            assert_eq!(r.neuves, 1);
            assert_eq!(r.restaurations, 1);
            for (ordre, _) in &en {
                assert_eq!(
                    r.appels_de(*ordre),
                    0,
                    "ordre {ordre} rejoué depuis le pas {pas}"
                );
            }
        }
    }
}

/// E9 en petit, `reecrire_preserve_le_sens` sur `devine` (§ 8.5) et
/// `deux_instances_meme_journal_meme_empreinte` (§ 8.7) : le journal de la soirée,
/// rejoué entrée par entrée sur la copie native, deux instances du module réécrit et
/// le module brut, donne les mêmes octets et le même état après chaque entrée.
#[test]
fn entree_par_entree_meme_etat() {
    let (_moteur, charge) = module::charger();
    let (d, _) = soiree(&charge);
    let c = rejeu::rejouer(&ouverture(), &charge, d.natif.journal(), d.module.journal());
    eprintln!("rejeu : {c:?}");
    assert!(c.entrees > 300, "{c:?}");
    assert_eq!(c.ecartees, 5, "{c:?}");
    assert!(c.empreintes >= 1, "{c:?}");
}
