//! L'ordre du classement d'un piège (PHASE-3, § 2.3) : garde-fou, croissance refusée,
//! carburant, profondeur, croissance bornée, panique, hors mémoire, pile native, autre ;
//! une erreur du moteur hors du module avant tout. Deux constats ensemble, le premier
//! dans cet ordre décide.

use jeu_abi::SorteAppel;
use moteur_commun::{Bits, Piege};
use partie::{CausePanne, Echec, Faute, Instance};
use reecriture::{CARBURANT, CROISSANCE_BORNEE, CROISSANCE_REFUSEE, PROFONDEUR};

use crate::fausse::{Fausse, Scenario, compter_et_salir, instance};

/// Un constat après un piège, et ce qu'il donne seul.
struct Constat {
    nom: &'static str,
    /// Le piège brut, si ce constat en est un ; sinon, des globales ou un message.
    piege: Option<Piege>,
    globales: Vec<(&'static str, Bits)>,
    panique: Option<&'static str>,
    issue: Echec,
}

/// Les constats, dans l'ordre du classement.
fn constats() -> Vec<Constat> {
    let globale = |nom, b| Constat {
        nom,
        piege: None,
        globales: vec![(nom, b)],
        panique: None,
        issue: Echec::Faute(Faute::Budget),
    };
    let piege = |nom, p: Piege, issue| Constat {
        nom,
        piege: Some(p),
        globales: Vec::new(),
        panique: None,
        issue,
    };
    let interne = |quoi: &str| Echec::Panne(partie::PanneOutil::new(CausePanne::Interne, quoi));
    vec![
        piege(
            "erreur du moteur",
            Piege::Moteur(String::from("erreur du moteur")),
            interne("erreur du moteur"),
        ),
        piege(
            "garde-fou",
            Piege::Interrompu,
            Echec::Panne(partie::PanneOutil::new(
                CausePanne::GardeFouMural,
                "l'appel a dépassé la durée murale du garde-fou",
            )),
        ),
        Constat {
            issue: Echec::Panne(partie::PanneOutil::new(
                CausePanne::CroissanceRefusee,
                "un memory.grow permis a échoué : la machine manque de mémoire",
            )),
            ..globale(CROISSANCE_REFUSEE, Bits::I32(1))
        },
        Constat {
            issue: Echec::Faute(Faute::Budget),
            ..globale(CARBURANT, Bits::I64(-1))
        },
        Constat {
            issue: Echec::Faute(Faute::Profondeur),
            ..globale(PROFONDEUR, Bits::I32(101))
        },
        Constat {
            issue: Echec::Faute(Faute::Memoire),
            ..globale(CROISSANCE_BORNEE, Bits::I32(1))
        },
        Constat {
            nom: "panique",
            piege: None,
            globales: Vec::new(),
            panique: Some("le jeu panique"),
            issue: Echec::Faute(Faute::Panique {
                message: String::from("le jeu panique"),
            }),
        },
        piege(
            "hors mémoire",
            Piege::HorsMemoire,
            Echec::Faute(Faute::HorsMemoire),
        ),
        piege(
            "pile native",
            Piege::PileNative,
            interne("pile native épuisée avant soiree_profondeur : la pile du fil est trop petite"),
        ),
        piege(
            "autre",
            Piege::Autre(String::from("unreachable")),
            Echec::Faute(Faute::Piege {
                code: String::from("unreachable"),
            }),
        ),
    ]
}

/// L'issue d'une entrée dont l'appel laisse ces constats.
fn issue(constats: &[&Constat]) -> Echec {
    let mut i = instance(compter_et_salir);
    let piege = constats
        .iter()
        .find_map(|c| c.piege.clone())
        .unwrap_or_else(|| Piege::Autre(String::from("fond")));
    let machine: &mut Fausse = i.machine();
    machine.scenario = Some(Scenario {
        globales: constats.iter().flat_map(|c| c.globales.clone()).collect(),
        panique: constats.iter().find_map(|c| c.panique),
        piege,
    });
    i.appeler(SorteAppel::Ecriture, &crate::instantane::entree(), 1000)
        .expect_err("l'appel devait piéger")
}

#[test]
fn chaque_constat_seul_donne_son_issue() {
    for c in &constats() {
        assert_eq!(issue(&[c]), c.issue, "{}", c.nom);
    }
    // Sans aucun constat, le piège de fond est nommé.
    assert_eq!(
        issue(&[]),
        Echec::Faute(Faute::Piege {
            code: String::from("fond")
        })
    );
}

/// Deux constats ensemble : le premier dans l'ordre l'emporte, pour chaque paire (deux
/// pièges bruts ne vont pas ensemble : un appel n'en rend qu'un).
#[test]
fn classer_garde_son_ordre() {
    let tous = constats();
    let mut paires = 0;
    for (k, premier) in tous.iter().enumerate() {
        for second in tous.iter().skip(k + 1) {
            if premier.piege.is_some() && second.piege.is_some() {
                continue;
            }
            assert_eq!(
                issue(&[premier, second]),
                premier.issue,
                "{} devait l'emporter sur {}",
                premier.nom,
                second.nom
            );
            assert_eq!(
                issue(&[second, premier]),
                premier.issue,
                "{} devait l'emporter sur {}, donnés dans l'autre sens",
                premier.nom,
                second.nom
            );
            paires += 1;
        }
    }
    assert_eq!(paires, 35);
}

/// Les bornes exactes ne sont pas des fautes : la réécriture ne pique qu'un carburant
/// négatif (`i64.lt_s` contre 0) et une profondeur au-delà de `profondeur_max`
/// (`i32.gt_u`, `reecriture/src/passes.rs`). Un piège qui laisse le carburant à 0 tout
/// juste, ou la profondeur au maximum tout juste, est classé par ce qui suit.
#[test]
fn les_bornes_exactes_ne_sont_pas_des_fautes() {
    let fond = Echec::Faute(Faute::Piege {
        code: String::from("fond"),
    });
    let borne = |nom, b| Constat {
        nom,
        piege: None,
        globales: vec![(nom, b)],
        panique: None,
        issue: fond.clone(),
    };
    // Le carburant à 0 et la profondeur à `profondeur_max` (100 dans `fausse::plan`).
    let carburant = borne(CARBURANT, Bits::I64(0));
    let profondeur = borne(PROFONDEUR, Bits::I32(100));
    assert_eq!(issue(&[&carburant]), fond, "carburant à 0");
    assert_eq!(issue(&[&profondeur]), fond, "profondeur au maximum");
    assert_eq!(issue(&[&carburant, &profondeur]), fond, "les deux bornes");
    // Une unité plus loin, chacune est sa faute.
    assert_eq!(
        issue(&[&borne(CARBURANT, Bits::I64(-1))]),
        Echec::Faute(Faute::Budget)
    );
    assert_eq!(
        issue(&[&borne(PROFONDEUR, Bits::I32(101))]),
        Echec::Faute(Faute::Profondeur)
    );
}
