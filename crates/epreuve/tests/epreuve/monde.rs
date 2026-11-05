//! Le monde simulé fidèle au réel (CONTRAT, « Ce que le réel impose » ; RESTE-PHASE-1,
//! points 20 à 22 ; constats de Tier list et de Sidequest) : chaque comportement du
//! monde arrive vraiment, et des fautifs qui passaient dans le monde trop sage de la
//! phase 1 ne passent plus.

use epreuve::{IdControle, IssueControle, Monde, Plan, Rapport};
use fautifs::jeux;
use jeu_abi::Oui;
use jeu_natif::FabriqueNative;

use crate::{GRAINE, echoue_exactement, eprouver, lisible};

/// Ce que le monde réel doit avoir fait dans les soirées de `devine`, qui a des
/// téléphones, un état tenu et toutes les pannes (OUT-26 : un comportement qui n'a pas
/// eu lieu n'a rien éprouvé).
pub fn le_monde_reel_a_tout_fait(r: &Rapport) {
    let m = r.monde;
    let texte = lisible(r);
    // Point 20 : les bots décident sur la vue reçue, parfois en retard, et rien sans vue.
    assert!(m.decisions_sur_vue_recue > 0, "{texte}");
    assert!(m.decisions_sur_vue_en_retard > 0, "{texte}");
    assert!(m.sans_vue > 0, "{texte}");
    // Point 21 : `fait` se trompe dans les deux sens.
    assert!(m.faits_en_avance > 0 && m.faits_en_retard > 0, "{texte}");
    assert!(m.ecart_d_horloge_max > 0, "{texte}");
    // Point 22 et le réseau : retransmissions qui retiennent la file, tenus perdus,
    // poignées ratées, accueils qui réalignent, rappels du tenu, vues lues d'un coup,
    // arrière-plans.
    assert!(m.retransmissions > 0, "{texte}");
    assert!(m.tenus_perdus > 0, "{texte}");
    assert!(m.poignees_ratees > 0, "{texte}");
    assert!(m.realignements > 0, "{texte}");
    assert!(m.rappels_de_tenu > 0, "{texte}");
    assert!(m.vues_en_rafale > 0, "{texte}");
    assert!(m.arriere_plans > 0, "{texte}");
    // Revue de la phase 2, constats 9 et 16 : un départ pour de bon et un geste livré
    // deux fois ont eu lieu dans la soirée principale. Une panne du plan manquée
    // aurait rendu chaque contrôle des soirées « pas exécuté » ; le doublon reconnu se
    // compte ici (PUN-16 : un bulletin compte une fois).
    assert!(m.gestes_doubles > 0, "{texte}");
    for panne in [epreuve::Panne::Depart, epreuve::Panne::Doublon] {
        assert!(r.plan.pannes.contains(&panne), "{panne:?} au plan");
    }
    // E15 : la soirée longue a ses coupures, rechargements, arrière-plans et sommeils,
    // chacun suivi de son retour.
    let e15 = r.resultat(IdControle::E15).expect("au rapport");
    assert!(
        e15.detail.contains("allées et venues de la soirée longue"),
        "{}",
        e15.detail
    );
    for sorte in ["coupures", "rechargements", "arrière-plans", "sommeils"] {
        assert!(e15.detail.contains(sorte), "{sorte} : {}", e15.detail);
    }
}

/// Le plan court, réduit à un contrôle, dans ce monde.
fn plan(controle: IdControle, monde: Monde) -> Plan {
    Plan {
        controles: vec![controle],
        monde,
        ..Plan::court(GRAINE)
    }
}

/// Points 20 et 21 : un jeu qui juge sur `fait` ce qu'un joueur a vu. Dans le monde de
/// la phase 1, où le bot décidait sur l'état courant avec une horloge exacte, il passe ;
/// dans le monde réel, il échoue E4, et la vue reçue suffit à le faire échouer.
#[test]
fn juge_sur_le_fait_ne_passe_plus_grace_au_monde_reel() {
    let r = eprouver(
        "juge-sur-le-fait",
        &mut FabriqueNative::<jeux::JugeSurLeFait>::new(),
        &Plan::court(GRAINE),
    );
    echoue_exactement(&r, IdControle::E4);
    assert!(
        r.resultat(IdControle::E4).is_some_and(|c| c
            .detail
            .contains("un chiffre choisi est le chiffre affiché")),
        "{}",
        lisible(&r)
    );
    let sage = eprouver(
        "juge-sur-le-fait",
        &mut FabriqueNative::<jeux::JugeSurLeFait>::new(),
        &plan(IdControle::E4, Monde::PHASE_1),
    );
    assert_eq!(
        sage.resultat(IdControle::E4).map(|c| c.issue.clone()),
        Some(IssueControle::Reussi),
        "{}",
        lisible(&sage)
    );
    let vues = eprouver(
        "juge-sur-le-fait",
        &mut FabriqueNative::<jeux::JugeSurLeFait>::new(),
        &plan(
            IdControle::E4,
            Monde {
                vues_recues: Oui::Oui,
                ..Monde::PHASE_1
            },
        ),
    );
    assert_eq!(
        vues.resultat(IdControle::E4).map(|c| c.issue.clone()),
        Some(IssueControle::EchecDuJeu),
        "{}",
        lisible(&vues)
    );
}

/// Constat de Tier list : aucune soirée ne jouait les fichiers. Le monde réel sert de
/// vrais contenus, et la soirée hostile en abîme : un jeu qui lit un fichier sans en
/// vérifier la taille échoue E13, et lui seul ; dans le monde de la phase 1, qui
/// refusait toute lecture, il passait.
#[test]
fn fichier_qui_panique_ne_passe_plus_grace_au_monde_reel() {
    let r = eprouver(
        "fichier-qui-panique",
        &mut FabriqueNative::<jeux::FichierQuiPanique>::new(),
        &Plan::court(GRAINE),
    );
    echoue_exactement(&r, IdControle::E13);
    let m = r.monde;
    assert!(
        m.fichiers_recus > 0 && m.fichiers_abimes > 0,
        "{}",
        lisible(&r)
    );
    assert!(m.fichiers_gros > 0, "{}", lisible(&r));
    assert!(m.demandes_reussies > 0, "{}", lisible(&r));
    assert!(m.demandes_servies > m.demandes_reussies, "{}", lisible(&r));
    let sage = eprouver(
        "fichier-qui-panique",
        &mut FabriqueNative::<jeux::FichierQuiPanique>::new(),
        &plan(IdControle::E13, Monde::PHASE_1),
    );
    assert_eq!(
        sage.resultat(IdControle::E13).map(|c| c.issue.clone()),
        Some(IssueControle::Reussi),
        "{}",
        lisible(&sage)
    );
    assert_eq!(sage.monde.fichiers_recus, 0);
}

/// Constat de Sidequest : la période d'échantillonnage de l'hypothèse de E11 est une
/// constante publique du commun, qu'un jeu vérifie à la compilation.
#[test]
fn la_periode_de_l_hypothese_est_publique() {
    const _: () = assert!(jeu::PERIODE_HYPOTHESE.en_pas() <= 30);
    assert_eq!(jeu::PERIODE_HYPOTHESE, jeu_abi::PERIODE_HYPOTHESE);
}
