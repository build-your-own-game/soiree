//! L'épreuve elle-même, éprouvée (INTERFACE.md, § 8.8, « Tests attendus ») : `devine`
//! et le socle sain passent chaque contrôle écrit ; chaque jeu de `exemples/fautifs`
//! fait échouer exactement le contrôle qu'il vise, et aucun autre (OUT-27, HIS-03,
//! SAL-168) ; une panne déclarée qui n'a pas eu lieu, ou un contrôle qui n'a rien
//! évalué, ne compte jamais comme réussi (OUT-26, OUT-07) ; le rejeu d'un échec
//! refait sa faute (HIS-06, HIS-07) ; et le binaire `soiree-epreuve` dit tout cela par
//! son code de sortie et son rapport JSON.
//!
//! Chaque épreuve joue le plan court (`Plan::court`) : une soirée principale de deux
//! minutes avec toutes les pannes, sa seconde exécution, une soirée hostile et une
//! soirée longue d'une heure. Le plan standard (`soiree-epreuve` sans `--court`)
//! joue deux soirées de trois minutes et six heures pour E15.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "code de test : un échec s'y lit comme l'échec du test qui l'appelle"
)]

mod modules;
mod monde;
mod pile;
mod plan_standard;

use epreuve::{
    Etape, FabriqueDynamique, IdControle, IssueControle, IssueEtape, Panne, Plan, Rapport,
};
use fautifs::jeux;
use fautifs_statiques::jeux as statiques;
use jeu_abi::Duree;
use jeu_natif::FabriqueNative;
use partie::Faute;

/// La graine des épreuves de ce fichier : fixée, pour qu'un échec se rejoue.
const GRAINE: [u8; 32] = [0x2a; 32];

/// L'épreuve du module `nom`, avec sa copie native, sur ce plan.
fn eprouver(nom: &str, natif: &mut dyn FabriqueDynamique, plan: &Plan) -> Rapport {
    let (moteur, octets) = modules::module(nom);
    epreuve::eprouver_octets(&moteur, &octets, Some(natif), plan)
}

/// Le résumé lisible d'un rapport, pour les messages d'échec.
fn lisible(r: &Rapport) -> String {
    r.texte()
}

/// Chaque contrôle du plan est réussi ou sans objet, et chaque réussite a des
/// évaluations ; hors du plan, il n'y a que E20 (un contrôle long, que seul
/// `--navigateurs` demande : PHASE-3, § 8.3), « pas exécuté », jamais réussi.
/// Le module est donné tout compilé : l'étape 1 n'est pas faite, et c'est la seule
/// étape exigée qui manque (code 3, jamais 0 ; RESTE-PHASE-1, point 13).
fn passe_tout(r: &Rapport) {
    let hors_du_plan: Vec<IdControle> = r
        .controles
        .iter()
        .filter(|c| !r.plan.demande(c.id))
        .map(|c| c.id)
        .collect();
    assert_eq!(hors_du_plan, [IdControle::E20], "{}", lisible(r));
    let e20 = r.resultat(IdControle::E20).expect("au rapport");
    assert_eq!(e20.issue, IssueControle::PasExecute, "{}", lisible(r));
    for c in r.controles.iter().filter(|c| r.plan.demande(c.id)) {
        assert!(
            matches!(c.issue, IssueControle::Reussi | IssueControle::SansObjet(_)),
            "{} : {:?}\n{}",
            c.id,
            c.issue,
            lisible(r)
        );
        if c.issue == IssueControle::Reussi {
            assert!(c.evaluations > 0, "{} réussi sans évaluation", c.id);
        }
    }
    assert_eq!(r.code_des_controles(), 0, "{}", lisible(r));
    let manquent: Vec<Etape> = r
        .etapes()
        .into_iter()
        .filter(epreuve::ResultatEtape::manque)
        .map(|e| e.etape)
        .collect();
    assert_eq!(manquent, [Etape::Compiler], "{}", lisible(r));
    for e in r.etapes() {
        if matches!(e.etape, Etape::Constater | Etape::Mesurer) {
            assert_eq!(e.issue, IssueEtape::Faite, "{:?}\n{}", e.etape, lisible(r));
        }
        // Revue de la phase 2, constat 15 : l'étape 4 du CONTRAT compare chaque partie
        // avec et sans prédiction, que cette version n'a pas ; elle n'est que faite en
        // partie, et le verdict d'un code 0 le dit.
        if e.etape == Etape::Chasser {
            assert!(
                matches!(e.issue, IssueEtape::FaiteEnPartie(_)),
                "{:?}\n{}",
                e.issue,
                lisible(r)
            );
        }
    }
    let v = r.verdict_du_code_0();
    assert!(v.contains("4. Chasser faite en partie"), "{v}");
    assert!(v.contains("3. Examiner pas faite"), "{v}");
    assert_eq!(r.code_de_sortie(), 3, "{}", lisible(r));
}

/// Le fautif fait échouer exactement `vise`, et aucun autre contrôle écrit ; le
/// rapport le nomme, avec de quoi le rejouer quand il vient d'une soirée.
fn echoue_exactement(r: &Rapport, vise: IdControle) {
    assert_eq!(r.echecs(), [vise], "{}", lisible(r));
    for c in &r.controles {
        assert!(
            !matches!(c.issue, IssueControle::PanneDeLOutil),
            "{} : panne de l'outil\n{}",
            c.id,
            lisible(r)
        );
    }
    let e = r.resultat(vise).expect("le contrôle visé est au rapport");
    assert!(!e.detail.is_empty(), "le rapport dit ce qui a échoué");
    assert_eq!(r.code_de_sortie(), 1);
    // Un pire cas se refait par son indice, que le détail nomme : il n'a pas de soirée ;
    // une page, par sa vue et ses locaux (E21), sur une instance nue.
    if ![
        IdControle::E1,
        IdControle::E3,
        IdControle::E16,
        IdControle::E21,
    ]
    .contains(&vise)
    {
        assert!(
            e.rejeu.is_some(),
            "{vise} : un échec d'une soirée porte son rejeu"
        );
    }
}

#[test]
fn devine_passe_tout() {
    let r = eprouver(
        "devine",
        &mut FabriqueNative::<devine::Devine>::new(),
        &Plan::court(GRAINE),
    );
    passe_tout(&r);
    // Le monde réel a vraiment joué chacun de ses comportements (RESTE-PHASE-1, points
    // 20 à 22).
    monde::le_monde_reel_a_tout_fait(&r);
    // `devine` n'a pas de fin : E2 est sans objet, avec la raison qu'il déclare.
    assert_eq!(
        r.resultat(IdControle::E2).map(|c| c.issue.clone()),
        Some(IssueControle::SansObjet(String::from(
            "les manches s'enchaînent sans fin de partie"
        )))
    );
    assert!(r.natif);
}

#[test]
fn le_socle_sain_passe_tout() {
    let r = eprouver(
        "sain",
        &mut FabriqueNative::<jeux::Sain>::new(),
        &Plan::court(GRAINE),
    );
    passe_tout(&r);
    // Le socle a une fin : E2 y est jugé, pas sans objet.
    assert_eq!(
        r.resultat(IdControle::E2).map(|c| c.issue.clone()),
        Some(IssueControle::Reussi)
    );
    // Sans fenêtre, E12 juge le découpage d'un pas (PHASE-3, P7), et il a tourné.
    let e12 = r.resultat(IdControle::E12).expect("au rapport");
    assert_eq!(e12.issue, IssueControle::Reussi, "{}", lisible(&r));
    assert!(e12.evaluations > 0);
    assert!(e12.detail.contains("découpage"), "{}", e12.detail);
    // E16 mesure vraiment les textes de ses pires cas (PHASE-3, § 2.9, lot 8) : le
    // prénom de douze « W » tient dans ses six unités, à sa plus petite taille.
    let e16 = r.resultat(IdControle::E16).expect("au rapport");
    assert!(
        e16.detail.contains("texte(s) mesuré(s) dans leur boîte")
            && e16.detail.contains(": 0 débordent"),
        "{}",
        e16.detail
    );
    // Le compte se lit au début de sa ligne (les notes du détail sont jointes par des
    // fins de ligne) : un compte nul serait un E16 qui ne mesure rien.
    let mesures: u64 = e16
        .detail
        .lines()
        .find_map(|l| {
            l.split_once(" texte(s) mesuré(s) dans leur boîte")
                .map(|(n, _)| n)
        })
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("aucun compte de textes mesurés : {}", e16.detail));
    assert!(mesures > 0, "E16 n'a mesuré aucun texte : {}", e16.detail);
}

/// Le contrôle négatif de E12 pour le découpage d'un pas (PHASE-3, § 8.3 et P7) :
/// `etale-mal`, un défaut du commun et non d'un jeu, qui laisse une arrivée reçue
/// pendant un pas découpé entrer dans ce pas. Le socle sain fait alors échouer E12, et
/// lui seul.
#[test]
fn etale_mal_fait_echouer_e12_et_lui_seul() {
    let plan = Plan {
        commun_fautif: Some(epreuve::CommunFautif::EtaleMal),
        ..Plan::court(GRAINE)
    };
    let r = eprouver("sain", &mut FabriqueNative::<jeux::Sain>::new(), &plan);
    echoue_exactement(&r, IdControle::E12);
    let e12 = r.resultat(IdControle::E12).expect("au rapport");
    assert!(e12.detail.contains("un pas découpé"), "{}", e12.detail);
}

/// Besoins 2 et 3 des jeux (`conception/BESOINS-DES-JEUX.md`, Tier list n° 2 et n° 3) :
/// un jeu sans téléphone ne vit aucune panne de téléphone. Elles sont sans objet, dites
/// dans le rapport, et ses soirées sont jugées au lieu de finir « pas exécuté » (code 3).
#[test]
fn un_jeu_sans_telephone_passe_tout_sans_pannes_de_telephone() {
    let r = eprouver(
        "ardoise",
        &mut FabriqueNative::<ardoise::Ardoise>::new(),
        &Plan::court(GRAINE),
    );
    passe_tout(&r);
    assert!(
        r.joue
            .contains("pannes sans objet, le jeu n'a pas de téléphone : coupure de 1500 ms"),
        "le rapport dit les pannes sans objet : {}",
        r.joue
    );
    // Les autres pannes du plan restent jouées et constatées.
    assert!(
        !r.joue.contains("redémarrage de l'autorité, "),
        "{}",
        r.joue
    );
    for c in [
        IdControle::E4,
        IdControle::E9,
        IdControle::E14,
        IdControle::E19,
    ] {
        assert_eq!(
            r.resultat(c).map(|c| c.issue.clone()),
            Some(IssueControle::Reussi),
            "{c}\n{}",
            lisible(&r)
        );
    }
}

macro_rules! fautif {
    ($test:ident, $nom:literal, $jeu:ty, $vise:expr) => {
        #[test]
        fn $test() {
            let r = eprouver(
                $nom,
                &mut FabriqueNative::<$jeu>::new(),
                &Plan::court(GRAINE),
            );
            echoue_exactement(&r, $vise);
        }
    };
}

fautif!(
    declaration_incoherente_echoue_e1,
    "declaration-incoherente",
    jeux::DeclarationIncoherente,
    IdControle::E1
);
fautif!(
    jamais_fini_echoue_e2,
    "jamais-fini",
    jeux::JamaisFini,
    IdControle::E2
);
fautif!(
    phase_inaccessible_echoue_e3,
    "phase-inaccessible",
    jeux::PhaseInaccessible,
    IdControle::E3
);
fautif!(
    invariant_viole_echoue_e4,
    "invariant-viole",
    jeux::InvariantViole,
    IdControle::E4
);
// CAT-78.
fautif!(
    attend_un_absent_echoue_e5,
    "attend-un-absent",
    jeux::AttendUnAbsent,
    IdControle::E5
);
/// PUN-27, HIS-44 : un refus ne change rien. Ce fautif montre à l'écran un `static`
/// qu'un refus écrit : E6 le voit au refus. Depuis que E8 recharge la projection dans
/// une instance neuve (revue de la phase 2, constat 1), E8 le voit aussi : la vue de
/// l'écran n'est plus une fonction de la projection, puisque le `static` n'est pas
/// dans l'état. Les deux échecs sont justes, et seuls ces deux-là ont lieu ; l'étape 1
/// refuserait ce `static` avant de jouer (« rien hors de l'état »).
#[test]
fn refuse_apres_avoir_ecrit_echoue_e6_et_e8() {
    let r = eprouver(
        "refuse-apres-avoir-ecrit",
        &mut FabriqueNative::<statiques::RefuseApresAvoirEcrit>::new(),
        &Plan::court(GRAINE),
    );
    assert_eq!(
        r.echecs(),
        [IdControle::E6, IdControle::E8],
        "{}",
        lisible(&r)
    );
    assert_eq!(r.code_de_sortie(), 1);
}
fautif!(
    vue_qui_ecrit_echoue_e7,
    "vue-qui-ecrit",
    statiques::VueQuiEcrit,
    IdControle::E7
);
// CAT-46.
fautif!(
    vue_qui_fuit_echoue_e8,
    "vue-qui-fuit",
    jeux::VueQuiFuit,
    IdControle::E8
);
fautif!(
    natif_different_echoue_e9,
    "natif-different",
    jeux::NatifDifferent,
    IdControle::E9
);
// SAL-117, REM-58, REM-59.
fautif!(
    octet_qui_panique_echoue_e13,
    "octet-qui-panique",
    jeux::OctetQuiPanique,
    IdControle::E13
);
fautif!(
    etat_hors_enveloppe_echoue_e14,
    "etat-hors-enveloppe",
    statiques::EtatHorsEnveloppe,
    IdControle::E14
);
// OUT-84, HIS-10, SQ-76, PUN-15.
fautif!(
    fuit_la_memoire_echoue_e15,
    "fuit-la-memoire",
    jeux::FuitLaMemoire,
    IdControle::E15
);
// CAT-27.
fautif!(
    depend_de_l_appareil_echoue_e10,
    "depend-de-l-appareil",
    jeux::DependDeLAppareil,
    IdControle::E10
);
// PUN-73, SQ-28.
fautif!(
    coupure_qui_compte_echoue_e11,
    "coupure-qui-compte",
    jeux::CoupureQuiCompte,
    IdControle::E11
);
// § 4.3, D12.
fautif!(
    compte_le_recu_echoue_e12,
    "compte-le-recu",
    jeux::CompteLeRecu,
    IdControle::E12
);
// § 6.1, D26.
fautif!(
    pire_cas_qui_deborde_echoue_e16,
    "pire-cas-qui-deborde",
    jeux::PireCasQuiDeborde,
    IdControle::E16
);
// CAT-05, SQ-68.
fautif!(
    nan_dans_l_etat_echoue_e17,
    "nan-dans-l-etat",
    jeux::NanDansLEtat,
    IdControle::E17
);
// PUN-34, D14, D15.
fautif!(
    gel_qui_enchaine_echoue_e18,
    "gel-qui-enchaine",
    jeux::GelQuiEnchaine,
    IdControle::E18
);
fautif!(
    boucle_au_vote_echoue_e19,
    "boucle-au-vote",
    jeux::BoucleAuVote,
    IdControle::E19
);
// RESTE-PHASE-1, point 19 : une vue que seuls les pires cas paient.
fautif!(
    pire_cas_trop_lourd_echoue_e19,
    "pire-cas-trop-lourd",
    jeux::PireCasTropLourd,
    IdControle::E19
);
// RESTE-PHASE-1, point 18 : une lecture courante trop lourde, qu'une vue de pire cas
// pourrait être.
fautif!(
    fin_trop_lourde_echoue_e19,
    "fin-trop-lourde",
    jeux::FinTropLourde,
    IdControle::E19
);
// MESURES-PHASE-1, proposition 1 : le carburant seul ne borne pas la durée d'un appel ;
// la remise à zéro des zones compte.
fautif!(
    zones_lourdes_echoue_e19,
    "zones-lourdes",
    jeux::ZonesLourdes,
    IdControle::E19
);
// RESTE-PHASE-1, point 23 : une variante de `Phase` sans pire cas, jamais atteinte.
fautif!(
    phase_sans_cas_echoue_e3,
    "phase-sans-cas",
    jeux::PhaseSansCas,
    IdControle::E3
);
// OUT-01, RESTE-PHASE-1, point 24 : un invariant déclaré qui n'a jamais tourné.
fautif!(
    invariant_jamais_evalue_echoue_e4,
    "invariant-jamais-evalue",
    jeux::InvariantJamaisEvalue,
    IdControle::E4
);
// PUN-27, HIS-44, RESTE-PHASE-1, point 11 : un `static` écrit dans `Avant`, que la
// forme canonique et les vues ne montrent pas au refus.
fautif!(
    retient_un_refus_echoue_e6,
    "retient-un-refus",
    statiques::RetientUnRefus,
    IdControle::E6
);

// RESTE-PHASE-1, point 16 ; CAT-46 : un secret d'un octet, qu'un geste écrit et que
// le choix public montre. Aucune recherche d'octets ne le trouve : la variation, si.
fautif!(
    choix_qui_fuit_echoue_e8,
    "choix-qui-fuit",
    jeux::ChoixQuiFuit,
    IdControle::E8
);

// Revue de la phase 2, constat 1 (INTERFACE, § 2.2) : le mot secret recopié dans un
// `static`, que l'écran montre. L'instance de projection de E8 partait de la mémoire
// de la définitive, et le `static` y passait avec elle : `vue(état) = vue(projection)`
// tenait à tort. Elle part désormais d'une instance neuve.
fautif!(
    secret_dans_un_static_echoue_e8,
    "secret-dans-un-static",
    statiques::SecretDansUnStatic,
    IdControle::E8
);

// Revue de la phase 2, constat 14 (INTERFACE, § 2.13) : des pires cas sans joueur
// passent leur cas, leurs invariants et leur scène ; l'écran d'une soirée coûte plus
// que le plus lourd d'entre eux.
fautif!(
    pire_cas_trop_leger_echoue_e16,
    "pire-cas-trop-leger",
    jeux::PireCasTropLeger,
    IdControle::E16
);
// PHASE-3, § 2.8, lot 6 : un module que le Worker refuserait au chargement le soir même
// échoue à l'épreuve ; le reste du jeu est le jeu sain.
fautif!(
    module_trop_gros_echoue_e1,
    "module-trop-gros",
    jeux::ModuleTropGros,
    IdControle::E1
);
// PHASE-3, § 8.3, lot 6 : des locaux tirés au hasard ne font jamais fauter une page
// (REM-57 : ce que la page reçoit est borné, ce qu'elle en fait aussi).
fautif!(
    page_qui_panique_echoue_e21,
    "page-qui-panique",
    jeux::PageQuiPanique,
    IdControle::E21
);
// PHASE-3, § 2.10, lot 6 : ce qu'une page envoie est validé comme ce qui vient de
// l'extérieur ; un geste au-delà de `limites.geste` ne passe pas.
fautif!(
    page_qui_envoie_un_geste_invalide_echoue_e21,
    "page-qui-envoie-un-geste-invalide",
    jeux::PageQuiEnvoieUnGesteInvalide,
    IdControle::E21
);

/// PHASE-3, P33, lot 7 (REPONSES-PROVISOIRES 14 ; SAL-162) : F8 pose la marque au
/// journal, et n'est qu'au commun. Une déclaration qui la prend pour son diagnostic est
/// refusée au chargement, par le moteur que partagent wasmtime et le navigateur : E1
/// échoue, et lui seul, en le disant.
#[test]
fn diagnostic_f8_echoue_e1() {
    let r = eprouver(
        "diagnostic-f8",
        &mut FabriqueNative::<jeux::DiagnosticF8>::new(),
        &Plan::court(GRAINE),
    );
    echoue_exactement(&r, IdControle::E1);
    let e1 = r.resultat(IdControle::E1).expect("au rapport");
    assert!(
        e1.detail.contains("diagnostic") && e1.detail.contains("F8"),
        "{}",
        e1.detail
    );
}

/// PHASE-3, § 2.10 et § 8.3, lot 7 : un jeu à téléphones qui garde la page par défaut
/// de `Jeu::page`, vide, laisse chaque téléphone sans rien à montrer ni à toucher, sans
/// un mot (REM-38). E21 échoue, et lui seul, en nommant la page par défaut.
#[test]
fn page_vide_echoue_e21() {
    let r = eprouver(
        "page-vide",
        &mut FabriqueNative::<jeux::PageVide>::new(),
        &Plan::court(GRAINE),
    );
    echoue_exactement(&r, IdControle::E21);
    let e21 = r.resultat(IdControle::E21).expect("au rapport");
    assert!(
        e21.detail
            .contains("la page est celle par défaut de Jeu::page"),
        "{}",
        e21.detail
    );
}

/// PHASE-3, P22, lot 8 : le commun ne lit que les polices sfnt (TTF, OTF). Une police
/// déclarée en WOFF2, celle d'une page web, est refusée par E1, qui le dit et dit quoi
/// faire ; aucun texte ne s'en sert, et rien d'autre n'échoue.
#[test]
fn police_woff2_echoue_e1() {
    let r = eprouver(
        "police-woff2",
        &mut FabriqueNative::<jeux::PoliceWoff2>::new(),
        &Plan::court(GRAINE),
    );
    echoue_exactement(&r, IdControle::E1);
    let e1 = r.resultat(IdControle::E1).expect("au rapport");
    assert!(
        e1.detail.contains("WOFF2") && e1.detail.contains("TTF ou OTF"),
        "{}",
        e1.detail
    );
}

/// PHASE-3, § 2.9, lot 8 (PUN-53, PUN-54) : E16 mesure chaque texte des pires cas dans
/// sa boîte. Une phrase trop longue pour sa ligne, même à la plus petite taille de son
/// style, fait échouer E16, et lui seul, en nommant le texte et la boîte.
#[test]
fn texte_qui_deborde_echoue_e16() {
    let r = eprouver(
        "texte-qui-deborde",
        &mut FabriqueNative::<jeux::TexteQuiDeborde>::new(),
        &Plan::court(GRAINE),
    );
    echoue_exactement(&r, IdControle::E16);
    let e16 = r.resultat(IdControle::E16).expect("au rapport");
    assert!(
        e16.detail.contains("« Record absolu")
            && e16
                .detail
                .contains("ne tient pas dans sa boîte de 16.00 × 1.00")
            && e16.detail.contains("même à sa plus petite taille, 0.50"),
        "{}",
        e16.detail
    );
}

/// La reprise (§ 4.6 ; INTERFACE, § 8.8, `panique_au_temps` ; RESTE-PHASE-1, point 9) :
/// trois `Temps` qui paniquent mettent la partie en panne ; l'épreuve la fait repartir
/// (`Partie::repartir`), et la soirée continue jusqu'au bout. Seul E2 échoue, avec la
/// faute ; le rapport dit la reprise.
#[test]
fn panique_au_temps_echoue_e2_et_la_partie_repart() {
    let r = eprouver(
        "panique-au-temps",
        &mut FabriqueNative::<jeux::PaniqueAuTemps>::new(),
        &Plan::court(GRAINE),
    );
    echoue_exactement(&r, IdControle::E2);
    let e2 = r.resultat(IdControle::E2).expect("au rapport");
    assert!(
        e2.detail.contains("trois `Temps` écartés de suite"),
        "{}",
        e2.detail
    );
    assert!(
        r.joue.contains("reprise(s) après trois `Temps` écartés"),
        "{}",
        r.joue
    );
}

/// La fenêtre et la tête (§ 4.2, D12) : le socle sain, avec une fenêtre de douze pas,
/// passe chaque contrôle, et E12 y est jugé, pas sans objet.
#[test]
fn avec_fenetre_passe_tout() {
    let r = eprouver(
        "avec-fenetre",
        &mut FabriqueNative::<jeux::AvecFenetre>::new(),
        &Plan::court(GRAINE),
    );
    passe_tout(&r);
    assert_eq!(
        r.resultat(IdControle::E12).map(|c| c.issue.clone()),
        Some(IssueControle::Reussi)
    );
}

/// `panne_injectee_constatee` (OUT-26) : un gel plus long que la soirée ne peut pas
/// avoir lieu ; les contrôles lus dans la soirée sont « pas exécuté », jamais réussis,
/// et le rapport nomme la panne.
#[test]
fn panne_injectee_constatee() {
    let plan = Plan {
        soirees: 1,
        duree: Duree::s(20),
        pannes: vec![Panne::Gel {
            duree: Duree::s(60),
        }],
        controles: vec![IdControle::E4, IdControle::E6, IdControle::E7],
        ..Plan::court(GRAINE)
    };
    let r = eprouver("sain", &mut FabriqueNative::<jeux::Sain>::new(), &plan);
    for c in [IdControle::E4, IdControle::E6, IdControle::E7] {
        let res = r.resultat(c).expect("au rapport");
        assert_eq!(res.issue, IssueControle::PasExecute, "{c}\n{}", lisible(&r));
        assert!(res.detail.contains("gel de 60000 ms"), "{}", res.detail);
    }
    assert_eq!(r.code_de_sortie(), 3);
}

/// `zero_evaluation_n_est_pas_un_succes` (OUT-07) : une soirée trop courte pour qu'une
/// entrée soit acceptée n'évalue aucun invariant ; E4 n'y est pas réussi.
#[test]
fn zero_evaluation_n_est_pas_un_succes() {
    let plan = Plan {
        soirees: 1,
        duree: Duree::pas(20),
        pannes: Vec::new(),
        controles: vec![IdControle::E4],
        ..Plan::court(GRAINE)
    };
    let r = eprouver("sain", &mut FabriqueNative::<jeux::Sain>::new(), &plan);
    let e4 = r.resultat(IdControle::E4).expect("au rapport");
    assert_eq!(e4.evaluations, 0, "{}", lisible(&r));
    assert_eq!(e4.issue, IssueControle::PasExecute);
    assert!(e4.detail.contains("OUT-07"), "{}", e4.detail);
    // Hors du plan, chaque autre contrôle figure au rapport, « pas exécuté » (OUT-10).
    assert_eq!(r.controles.len(), IdControle::TOUS.len());
    assert_eq!(r.controles.len(), 21);
    for c in &r.controles {
        if c.id != IdControle::E4 {
            assert_eq!(c.issue, IssueControle::PasExecute, "{}", c.id);
        }
    }
}

/// `rejeu_refait_la_faute` (HIS-06, HIS-07) : le rejeu que le rapport du fautif de
/// E13 porte, rejoué seul sur une instance neuve, refait la même panique.
#[test]
fn rejeu_refait_la_faute() {
    let (moteur, octets) = modules::module("octet-qui-panique");
    let charge = moteur.charger(&octets).expect("le module se charge");
    let plan = Plan {
        controles: vec![IdControle::E13],
        ..Plan::court(GRAINE)
    };
    let r = epreuve::eprouver(&charge, None, &plan);
    let e13 = r.resultat(IdControle::E13).expect("au rapport");
    assert_eq!(e13.issue, IssueControle::EchecDuJeu, "{}", lisible(&r));
    let rejeu = e13.rejeu.as_ref().expect("un rejeu");
    let refaite = epreuve::rejouer(&charge, rejeu).expect("la faute se refait");
    assert!(
        matches!(&refaite.faute, Faute::Panique { message } if message.contains("index out of bounds")),
        "{refaite:?}"
    );
    let premiere = rejeu
        .journal
        .entrees()
        .find(|e| matches!(e.issue, partie::IssueJournal::Ecartee { .. }))
        .expect("une entrée écartée");
    assert_eq!(
        partie::IssueJournal::Ecartee {
            faute: refaite.faute.clone()
        },
        premiere.issue
    );
}

/// Le binaire : son code de sortie et son rapport JSON.
#[test]
fn le_binaire_rend_un_code_et_un_json() {
    let bin = env!("CARGO_BIN_EXE_soiree-epreuve");
    let dossier = std::env::temp_dir().join(format!("soiree-epreuve-{}", std::process::id()));
    std::fs::create_dir_all(&dossier).expect("dossier");
    // Un module seul ne rend jamais 0 : l'étape 1 n'est pas faite (point 13).
    for (nom, code) in [("devine", 3), ("declaration-incoherente", 1)] {
        let chemin = dossier.join(format!("{nom}.wasm"));
        std::fs::write(&chemin, modules::module(nom).1).expect("écrire le module");
        let json = dossier.join(format!("{nom}.json"));
        let sortie = std::process::Command::new(bin)
            .arg(&chemin)
            .args(["--court", "--controles", "E1", "--json"])
            .arg(&json)
            .output()
            .expect("lancer soiree-epreuve");
        let texte = String::from_utf8_lossy(&sortie.stdout);
        assert_eq!(sortie.status.code(), Some(code), "{nom} :\n{texte}");
        assert!(texte.contains("E1 Constat"), "{texte}");
        // Chaque étape du CONTRAT est nommée, faite ou non ; le module seul et la
        // copie native absente aussi.
        for etape in Etape::TOUTES {
            let ligne = format!("{}. {} — ", etape.numero(), etape.nom());
            assert!(texte.contains(&ligne), "{ligne} : {texte}");
        }
        assert!(texte.contains("1. Compiler — pas faite"), "{texte}");
        assert!(texte.contains("Module seul"), "{texte}");
        assert!(texte.contains("Copie native absente"), "{texte}");
        let j = std::fs::read_to_string(&json).expect("le rapport JSON");
        assert!(j.contains("\"etapes\""), "{j}");
        assert!(j.contains("\"compilation\": false"), "{j}");
        assert!(j.contains(&format!("\"code_de_sortie\": {code}")), "{j}");
        for c in IdControle::TOUS {
            assert!(
                j.contains(&format!("\"id\": \"{c}\"")),
                "{c} absent du JSON"
            );
        }
    }
    let mal = std::process::Command::new(bin)
        .arg("--inconnue")
        .output()
        .expect("lancer soiree-epreuve");
    assert_eq!(mal.status.code(), Some(64));
    std::fs::remove_dir_all(&dossier).expect("nettoyer");
}
