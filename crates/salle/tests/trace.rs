//! Les traces de l'entrée au pixel (PHASE-3, § 8.4 ; lot 13) : le découpage par segment,
//! les distributions (jamais une moyenne), le panneau de diagnostic ; et, par la vraie
//! autorité dans la soirée simulée, chaque entrée placée qui revient en trace après la
//! scène du tour qui a joué son pas.

#![cfg(test)]

use std::cell::RefCell;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::sync::OnceLock;

use jeu_abi::{EcranFil, Oui, Pas, SceneFil};
use moteur_wasmtime::{Moteur, ReglagesMoteur};
use salle::ecran::{DeLaPage, IdTrace, Trace, VersPage};
use salle::simulation::{Pannes, Soiree};
use salle::trace::{
    BUDGET_CLAVIER_US, Diagnostic, GESTES_PAR_SOURCE, SEGMENTS, Segment, Source, a_son_pixel,
    distribution, duree,
};

fn trace(evenement: u64) -> Trace {
    Trace {
        id: IdTrace::Ecran(1),
        evenement_us: evenement,
        envoi_us: evenement.saturating_add(300),
        relais: None,
        recu_us: evenement.saturating_add(500),
        place: Pas::du_fil(10),
        pas_debut_us: evenement.saturating_add(4_000),
        pas_fin_us: evenement.saturating_add(4_200),
        vue_fin_us: evenement.saturating_add(4_500),
        page_recue_us: evenement.saturating_add(4_900),
        image_us: evenement.saturating_add(10_000),
        soumis_us: evenement.saturating_add(12_000),
        gpu_fini_us: evenement.saturating_add(14_000),
        incertitude_us: 0,
    }
}

#[test]
fn chaque_segment_va_d_une_borne_a_la_suivante_et_le_total_les_somme() {
    let t = trace(1_000_000);
    let attendus = [
        (Segment::Evenement, 300),
        (Segment::Transport, 200),
        (Segment::Attente, 3_500),
        (Segment::Pas, 200),
        (Segment::Vues, 300),
        (Segment::VersLaPage, 400),
        (Segment::AttenteImage, 5_100),
        (Segment::Dessin, 2_000),
        (Segment::Gpu, 2_000),
        (Segment::Total, 14_000),
    ];
    for (s, d) in attendus {
        assert_eq!(duree(&t, s), Some(d), "{}", s.nom());
    }
    let somme: i64 = SEGMENTS
        .iter()
        .filter(|s| **s != Segment::Total)
        .filter_map(|s| duree(&t, *s))
        .sum();
    assert_eq!(Some(somme), duree(&t, Segment::Total));
}

/// Un segment qui franchit deux horloges (la page et le Worker) porte leur écart : sa
/// durée peut être négative, et se dit telle (M14).
#[test]
fn un_segment_entre_deux_horloges_peut_etre_negatif() {
    let t = Trace {
        recu_us: 1_000_000 + 100,
        ..trace(1_000_000)
    };
    assert_eq!(duree(&t, Segment::Transport), Some(-200));
}

/// Une entrée dont la scène n'a pas changé n'a ni scène reçue, ni dessin, ni GPU : ces
/// segments et le total manquent, les autres restent.
#[test]
fn une_trace_sans_pixel_n_a_ni_dessin_ni_total() {
    let t = Trace {
        page_recue_us: 0,
        image_us: 0,
        soumis_us: 0,
        gpu_fini_us: 0,
        ..trace(1_000_000)
    };
    assert!(!a_son_pixel(&t));
    for s in [
        Segment::VersLaPage,
        Segment::AttenteImage,
        Segment::Dessin,
        Segment::Gpu,
        Segment::Total,
    ] {
        assert_eq!(duree(&t, s), None, "{}", s.nom());
    }
    assert_eq!(duree(&t, Segment::Attente), Some(3_500));
}

/// Les centiles sont au rang le plus proche : une valeur vraiment observée, jamais une
/// moyenne ni une interpolation (§ 8.4).
#[test]
fn les_centiles_sont_au_rang_le_plus_proche() {
    let mut v: Vec<i64> = (1..=100).rev().collect();
    let d = distribution(&mut v).unwrap();
    assert_eq!(
        (d.n, d.min, d.mediane, d.c95, d.c99, d.max),
        (100, 1, 50, 95, 99, 100)
    );
    let mut v = vec![7, 3];
    let d = distribution(&mut v).unwrap();
    assert_eq!((d.mediane, d.c95, d.c99, d.max), (3, 7, 7, 7));
    let mut un = vec![42];
    let d = distribution(&mut un).unwrap();
    assert_eq!((d.min, d.mediane, d.c99, d.max), (42, 42, 42, 42));
    assert_eq!(distribution(&mut []), None);
}

/// Le panneau garde les 200 derniers gestes de chaque source, dit le budget du CONTRAT au
/// 99ᵉ centile, et garde le matériel sur une ligne à part (§ 2.9 ; OUT-88 : le « pixel »
/// est la fin du travail du GPU, pas le photon).
#[test]
fn le_panneau_garde_200_gestes_par_source_et_dit_le_budget() {
    let mut d = Diagnostic::new();
    for k in 0..250u64 {
        d.noter(Source::Clavier, trace(1_000_000 + k * 100_000));
    }
    let lent = Trace {
        gpu_fini_us: 1_000_000 + 90_000,
        ..trace(1_000_000)
    };
    for _ in 0..3 {
        d.noter(Source::Telephone(2), lent);
    }
    let c = d.distribution(Source::Clavier, Segment::Total).unwrap();
    assert_eq!(c.n, GESTES_PAR_SOURCE);
    assert!(c.c99 <= BUDGET_CLAVIER_US);
    d.noter_une_image(1);
    d.noter_une_image(7);
    let texte = d.texte("webgpu (essai)", 3);
    assert!(texte.contains("clavier : 200 gestes"), "{texte}");
    assert!(texte.contains("téléphone 2 : 3 gestes"), "{texte}");
    assert!(texte.contains("99 % sous 50.0 ms : tenu"), "{texte}");
    assert!(texte.contains("99 % sous 80.0 ms : DÉPASSÉ"), "{texte}");
    assert!(texte.contains("1 → 1"), "{texte}");
    assert!(texte.contains("4 et plus → 1"), "{texte}");
    assert!(
        texte.contains("moteur de dessin : webgpu (essai)"),
        "{texte}"
    );
    assert!(texte.contains("glyphes absents : 3"), "{texte}");
    let materiel = texte.lines().filter(|l| l.starts_with("matériel")).count();
    assert_eq!(materiel, 1, "{texte}");
}

// ---------------------------------------------------------------------------
// Par la vraie autorité
// ---------------------------------------------------------------------------

fn module_d_exemple(nom: &str) -> Vec<u8> {
    let racine = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cible = racine.join("target/verif-module");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| String::from("cargo"));
    let statut = Command::new(cargo)
        .current_dir(&racine)
        .env("CARGO_TARGET_DIR", &cible)
        .args([
            "build",
            "-p",
            &format!("{nom}-module"),
            "--target",
            "wasm32-unknown-unknown",
            "--profile",
            "module",
        ])
        .status()
        .expect("lancer cargo");
    assert!(
        statut.success(),
        "la compilation du module de {nom} a échoué"
    );
    std::fs::read(cible.join(format!("wasm32-unknown-unknown/module/{nom}_module.wasm")))
        .expect("lire le module")
}

fn devine() -> &'static [u8] {
    static OCTETS: OnceLock<Vec<u8>> = OnceLock::new();
    OCTETS.get_or_init(|| module_d_exemple("devine"))
}

fn ardoise() -> &'static [u8] {
    static OCTETS: OnceLock<Vec<u8>> = OnceLock::new();
    OCTETS.get_or_init(|| module_d_exemple("ardoise"))
}

/// Les instants que le Worker pose sont dans l'ordre du chemin.
fn dans_l_ordre(t: &Trace) {
    assert!(t.recu_us <= t.pas_debut_us, "{t:?}");
    assert!(t.pas_debut_us <= t.pas_fin_us, "{t:?}");
    assert!(t.pas_fin_us <= t.vue_fin_us, "{t:?}");
    assert_eq!(
        (t.page_recue_us, t.image_us, t.soumis_us, t.gpu_fini_us),
        (0, 0, 0, 0),
        "la page ajoute les siens"
    );
}

/// Les gestes des téléphones reviennent en traces : chacun placé une fois (son numéro, son
/// appareil), son pas joué au plus tard au tour qui l'envoie, avec les instants que le
/// téléphone a mis dans son geste.
#[test]
fn chaque_geste_place_revient_en_trace_apres_son_pas() {
    let moteur = Moteur::new(ReglagesMoteur::PHASE_3).expect("le moteur démarre");
    let charge = moteur.charger(devine()).expect("le module de devine");
    let mut s = Soiree::new(|| charge.fabrique(), 13, Oui::Oui, Pannes::AUCUNE);
    let recus = Rc::new(RefCell::new(Vec::<Vec<u8>>::new()));
    let r = Rc::clone(&recus);
    s.observer_la_page(move |o| r.borrow_mut().push(o.to_vec()));
    for _ in 0..3 {
        let _: usize = s.ajouter_telephone();
    }
    s.avancer_jusqua(s.t_us + 30_000_000);
    let places = s.autorite.as_ref().unwrap().comptes().gestes_places;
    assert!(places > 0, "aucun geste placé");
    let mut vues = std::collections::BTreeSet::new();
    let mut definitif_vu: u64 = 0;
    for o in recus.borrow().iter() {
        match VersPage::decoder(o).unwrap() {
            VersPage::Battement { pas, .. } => definitif_vu = pas.numero(),
            VersPage::Traces(v) => {
                assert!(!v.is_empty(), "un message de traces vide");
                for t in v {
                    dans_l_ordre(&t);
                    let IdTrace::Appareil { appareil, numero } = t.id else {
                        panic!("une trace d'écran sans entrée de l'écran : {t:?}");
                    };
                    // L'événement et l'incertitude sont ceux que le téléphone a estimés par
                    // écho ; dans le relais en mémoire, l'aller-retour peut être nul, et le
                    // tout premier geste partir avant tout écho (`evenement_us` à 0, la
                    // trace sans total).
                    assert!(
                        t.evenement_us == 0 || t.evenement_us <= t.recu_us,
                        "l'événement avant sa réception : {t:?}"
                    );
                    assert!(
                        t.place.numero() > definitif_vu,
                        "{t:?} après le pas {definitif_vu}"
                    );
                    assert!(
                        vues.insert((appareil.fil(), numero)),
                        "une trace rendue deux fois : {t:?}"
                    );
                }
            }
            VersPage::Scene { .. }
            | VersPage::Ressource { .. }
            | VersPage::Demande { .. }
            | VersPage::EtatSalle(_)
            | VersPage::Panne(_)
            | VersPage::EnCours(_)
            | VersPage::Chargement(_)
            | VersPage::Jeu { .. }
            | VersPage::Accueil { .. }
            | VersPage::Cle(_)
            | VersPage::Banc(_) => {}
        }
    }
    // Les gestes placés aux tout derniers tours attendent encore leur pas.
    let rendues = u64::try_from(vues.len()).unwrap();
    assert!(
        rendues + 8 >= places && rendues <= places,
        "{rendues} traces pour {places} gestes placés"
    );
}

/// Une entrée de l'écran (un raccourci de la scène, comme une touche du clavier) revient
/// en trace sous son numéro de page, avec l'instant de l'événement et celui de l'envoi que
/// la page a donnés, **juste après la scène** du tour qui a joué son pas : la page y
/// rattache sa réception, son dessin et le travail du GPU (OUT-93 : mesurée sous de vrais
/// gestes, pas sur un salon immobile).
#[test]
fn une_entree_de_l_ecran_revient_en_trace_juste_apres_sa_scene() {
    let moteur = Moteur::new(ReglagesMoteur::PHASE_3).expect("le moteur démarre");
    let charge = moteur.charger(ardoise()).expect("le module d'ardoise");
    let mut s = Soiree::new(|| charge.fabrique(), 14, Oui::Oui, Pannes::AUCUNE);
    let recus = Rc::new(RefCell::new(Vec::<Vec<u8>>::new()));
    let r = Rc::clone(&recus);
    s.observer_la_page(move |o| r.borrow_mut().push(o.to_vec()));
    // Les coups des bots de l'écran ne passent pas par la page : sans eux, la seule
    // entrée est la nôtre.
    s.depuis_la_page(
        &DeLaPage::Commande(salle::ecran::CommandeEcran::Bots(Oui::Non))
            .encoder()
            .unwrap(),
    );
    s.avancer_jusqua(s.t_us + 2_000_000);
    let derniere_scene = || -> (Pas, SceneFil) {
        recus
            .borrow()
            .iter()
            .rev()
            .find_map(|o| match VersPage::decoder(o) {
                Ok(VersPage::Scene { pas, octets, .. }) => {
                    Some((pas, jeu_abi::decoder::<SceneFil>(octets).unwrap()))
                }
                _ => None,
            })
            .expect("une scène")
    };
    let (vu, scene) = derniere_scene();
    let action = scene
        .raccourcis
        .first()
        .expect("un raccourci")
        .action
        .clone();
    let avant = recus.borrow().len();
    let t = s.t_us;
    s.depuis_la_page(
        &DeLaPage::Ecran {
            quoi: EcranFil::Action(&action),
            vu,
            t_us: t - 700,
            numero: 41,
            envoi_us: t - 200,
        }
        .encoder()
        .unwrap(),
    );
    s.avancer_jusqua(s.t_us + 200_000);
    let messages = recus.borrow()[avant..].to_vec();
    let mut precedent_scene = false;
    let mut trouvee = None;
    for o in &messages {
        match VersPage::decoder(o).unwrap() {
            VersPage::Traces(v) => {
                assert!(precedent_scene, "les traces suivent la scène de leur tour");
                trouvee = v.into_iter().find(|t| t.id == IdTrace::Ecran(41));
                break;
            }
            VersPage::Scene { .. } => precedent_scene = true,
            VersPage::Ressource { .. } | VersPage::Demande { .. } => {}
            VersPage::EtatSalle(_)
            | VersPage::Battement { .. }
            | VersPage::Panne(_)
            | VersPage::EnCours(_)
            | VersPage::Chargement(_)
            | VersPage::Jeu { .. }
            | VersPage::Accueil { .. }
            | VersPage::Cle(_)
            | VersPage::Banc(_) => precedent_scene = false,
        }
    }
    let t41 = trouvee.expect("la trace de l'entrée 41");
    assert_eq!((t41.evenement_us, t41.envoi_us), (t - 700, t - 200));
    assert_eq!(t41.recu_us, t);
    assert_eq!(t41.incertitude_us, 0);
    assert!(t41.place.numero() > vu.numero());
    dans_l_ordre(&t41);
}
