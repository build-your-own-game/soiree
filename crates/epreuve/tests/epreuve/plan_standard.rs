//! Chaque fautif sur le plan qui fait foi (CONTRAT, « Un contrôle doit savoir
//! échouer » ; RESTE-PHASE-1, contrôle final) : le plan standard de
//! `soiree-epreuve`, sa graine par défaut, avec la copie native. Chaque fautif y fait
//! échouer exactement son contrôle ; `devine`, `ardoise`, le socle sain et le socle à
//! fenêtre passent tout. Le plan court des autres tests ne le garantit pas : au
//! contrôle final de la phase 1, `etat-hors-enveloppe` échouait E14 sur le plan court
//! et passait sur le plan standard.
//!
//! Le plan standard joue six heures de soirée longue par jeu : près d'une minute par
//! jeu sur la machine de l'hôte. Le test les joue sur quatre fils, et ne tourne qu'à
//! la demande :
//!
//! ```text
//! cargo test -p epreuve --test epreuve -- --ignored plan_standard
//! ```

use std::sync::Mutex;

use epreuve::{FabriqueDynamique, IdControle, Plan};
use fautifs::jeux;
use fautifs_statiques::jeux as statiques;
use jeu_natif::FabriqueNative;

use crate::modules;

/// La graine par défaut de `soiree-epreuve` : celle du plan qui fait foi.
const GRAINE_STANDARD: [u8; 32] = [0x5e; 32];

/// Les fils qui jouent les jeux : quatre, pour la mémoire de la machine de l'hôte.
const FILS: usize = 4;

/// La copie native de ce jeu, et le contrôle qu'il doit faire échouer (`None` : il
/// passe tout).
fn jeu(nom: &str) -> (Box<dyn FabriqueDynamique>, Option<IdControle>) {
    macro_rules! n {
        ($t:ty) => {
            Box::new(FabriqueNative::<$t>::new())
        };
    }
    match nom {
        "devine" => (n!(devine::Devine), None),
        "ardoise" => (n!(ardoise::Ardoise), None),
        "sain" => (n!(jeux::Sain), None),
        "avec-fenetre" => (n!(jeux::AvecFenetre), None),
        "declaration-incoherente" => (n!(jeux::DeclarationIncoherente), Some(IdControle::E1)),
        "jamais-fini" => (n!(jeux::JamaisFini), Some(IdControle::E2)),
        "panique-au-temps" => (n!(jeux::PaniqueAuTemps), Some(IdControle::E2)),
        "phase-inaccessible" => (n!(jeux::PhaseInaccessible), Some(IdControle::E3)),
        "phase-sans-cas" => (n!(jeux::PhaseSansCas), Some(IdControle::E3)),
        "invariant-viole" => (n!(jeux::InvariantViole), Some(IdControle::E4)),
        "invariant-jamais-evalue" => (n!(jeux::InvariantJamaisEvalue), Some(IdControle::E4)),
        "attend-un-absent" => (n!(jeux::AttendUnAbsent), Some(IdControle::E5)),
        "refuse-apres-avoir-ecrit" => (n!(statiques::RefuseApresAvoirEcrit), Some(IdControle::E6)),
        "retient-un-refus" => (n!(statiques::RetientUnRefus), Some(IdControle::E6)),
        "vue-qui-ecrit" => (n!(statiques::VueQuiEcrit), Some(IdControle::E7)),
        "vue-qui-fuit" => (n!(jeux::VueQuiFuit), Some(IdControle::E8)),
        "choix-qui-fuit" => (n!(jeux::ChoixQuiFuit), Some(IdControle::E8)),
        "secret-dans-un-static" => (n!(statiques::SecretDansUnStatic), Some(IdControle::E8)),
        "pire-cas-trop-leger" => (n!(jeux::PireCasTropLeger), Some(IdControle::E16)),
        "natif-different" => (n!(jeux::NatifDifferent), Some(IdControle::E9)),
        "depend-de-l-appareil" => (n!(jeux::DependDeLAppareil), Some(IdControle::E10)),
        "coupure-qui-compte" => (n!(jeux::CoupureQuiCompte), Some(IdControle::E11)),
        "compte-le-recu" => (n!(jeux::CompteLeRecu), Some(IdControle::E12)),
        "octet-qui-panique" => (n!(jeux::OctetQuiPanique), Some(IdControle::E13)),
        "etat-hors-enveloppe" => (n!(statiques::EtatHorsEnveloppe), Some(IdControle::E14)),
        "fuit-la-memoire" => (n!(jeux::FuitLaMemoire), Some(IdControle::E15)),
        "pire-cas-qui-deborde" => (n!(jeux::PireCasQuiDeborde), Some(IdControle::E16)),
        "nan-dans-l-etat" => (n!(jeux::NanDansLEtat), Some(IdControle::E17)),
        "gel-qui-enchaine" => (n!(jeux::GelQuiEnchaine), Some(IdControle::E18)),
        "boucle-au-vote" => (n!(jeux::BoucleAuVote), Some(IdControle::E19)),
        "juge-sur-le-fait" => (n!(jeux::JugeSurLeFait), Some(IdControle::E4)),
        "fichier-qui-panique" => (n!(jeux::FichierQuiPanique), Some(IdControle::E13)),
        "pire-cas-trop-lourd" => (n!(jeux::PireCasTropLourd), Some(IdControle::E19)),
        "fin-trop-lourde" => (n!(jeux::FinTropLourde), Some(IdControle::E19)),
        "zones-lourdes" => (n!(jeux::ZonesLourdes), Some(IdControle::E19)),
        "module-trop-gros" => (n!(jeux::ModuleTropGros), Some(IdControle::E1)),
        "page-qui-panique" => (n!(jeux::PageQuiPanique), Some(IdControle::E21)),
        "page-qui-envoie-un-geste-invalide" => (
            n!(jeux::PageQuiEnvoieUnGesteInvalide),
            Some(IdControle::E21),
        ),
        "diagnostic-f8" => (n!(jeux::DiagnosticF8), Some(IdControle::E1)),
        "page-vide" => (n!(jeux::PageVide), Some(IdControle::E21)),
        "police-woff2" => (n!(jeux::PoliceWoff2), Some(IdControle::E1)),
        "texte-qui-deborde" => (n!(jeux::TexteQuiDeborde), Some(IdControle::E16)),
        autre => panic!("jeu sans attendu : {autre}"),
    }
}

/// Ce qu'un jeu a donné sur le plan standard : `None` s'il fait ce qu'on attend de
/// lui, sinon ce qui ne va pas.
fn juger(nom: &str) -> Option<String> {
    let (mut natif, vise) = jeu(nom);
    let (moteur, octets) = modules::module(nom);
    let r = epreuve::eprouver_octets(
        &moteur,
        &octets,
        Some(natif.as_mut()),
        &Plan::standard(GRAINE_STANDARD),
    );
    let mut attendu: Vec<IdControle> = vise.into_iter().collect();
    // Son `static`, montré à l'écran, fait aussi échouer E8 (revue de la phase 2,
    // constat 1 ; `refuse_apres_avoir_ecrit_echoue_e6_et_e8`).
    if nom == "refuse-apres-avoir-ecrit" {
        attendu.push(IdControle::E8);
    }
    let pannes: Vec<IdControle> = r
        .controles
        .iter()
        .filter(|c| c.issue == epreuve::IssueControle::PanneDeLOutil)
        .map(|c| c.id)
        .collect();
    let code = u8::from(vise.is_some());
    (r.echecs() != attendu || !pannes.is_empty() || r.code_des_controles() != code).then(|| {
        format!(
            "{nom} : échecs {:?} au lieu de {attendu:?}, pannes de l'outil {pannes:?}, code des contrôles {}\n{}",
            r.echecs(),
            r.code_des_controles(),
            r.texte()
        )
    })
}

/// Chaque fautif fait échouer exactement son contrôle sur le plan standard, et chaque
/// jeu sain passe tout.
#[test]
#[ignore = "le plan standard : près d'une minute par jeu ; cargo test -p epreuve --test epreuve -- --ignored plan_standard"]
fn chaque_fautif_sur_le_plan_standard() {
    for nom in modules::JEUX {
        let _: (Box<dyn FabriqueDynamique>, Option<IdControle>) = jeu(nom);
    }
    // Les modules se compilent une fois, avant les fils.
    let _: (moteur_wasmtime::Moteur, Vec<u8>) = modules::module("sain");
    let suivant = Mutex::new(0usize);
    let fautes = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..FILS {
            s.spawn(|| {
                loop {
                    let k = {
                        let mut n = suivant.lock().expect("le compteur");
                        let k = *n;
                        *n = n.saturating_add(1);
                        k
                    };
                    let Some(nom) = modules::JEUX.get(k) else {
                        break;
                    };
                    #[expect(
                        clippy::disallowed_methods,
                        reason = "une durée de test, pour qui lit sa sortie : rien n'en dépend"
                    )]
                    let debut = std::time::Instant::now();
                    let verdict = juger(nom);
                    eprintln!(
                        "{nom} : {} en {} s",
                        if verdict.is_none() {
                            "comme attendu"
                        } else {
                            "pas comme attendu"
                        },
                        debut.elapsed().as_secs()
                    );
                    if let Some(v) = verdict {
                        fautes.lock().expect("les fautes").push(v);
                    }
                }
            });
        }
    });
    let fautes = fautes.into_inner().expect("les fautes");
    assert!(fautes.is_empty(), "{}", fautes.join("\n\n"));
}
