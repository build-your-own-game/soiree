//! Des soirées en mémoire sur le module de `devine`, réécrit et chargé dans wasmtime
//! (PHASE-3, § 8.1, lot 4b) : la vraie autorité sur le vrai moteur, de vrais appareils
//! qui jouent les coups du bot du jeu par le vrai chemin, le relais en mémoire, et les
//! pannes de la graine, dont l'autorité tuée et relancée depuis la garde. Les trois jeux
//! se jouent de même par l'exemple `soiree_en_memoire`.
//!
//! `cfg(test)` marque tout ce binaire comme du code de test : les fonctions d'aide y
//! ont droit à `unwrap`, `expect`, `panic!` et aux index (CONTRAT, « Les règles
//! d'écriture »).

#![cfg(test)]

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::sync::OnceLock;

use jeu_abi::{IdGeste, Oui, SorteAppel};
use moteur_wasmtime::{InstantaneWasm, Moteur, ReglagesMoteur};
use partie::{CausePanne, Echec, Fabrique, Instance, PanneOutil};
use salle::chien::{Chien, Decision, TIC_MS};
use salle::ecran::{CommandeEcran, DeLaPage, VersPage};
use salle::en_cours::{Dire, FabriqueQuiDit, geste_de_l_appel};
use salle::simulation::{Pannes, Soiree, verifier_l_historique};

/// Le module de `devine`, compilé une fois par binaire de test, dans un dossier à part
/// (`target/verif-module`) : `cargo test` tient le verrou de `target/`.
fn octets() -> &'static [u8] {
    static OCTETS: OnceLock<Vec<u8>> = OnceLock::new();
    OCTETS.get_or_init(|| {
        let racine = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cible = racine.join("target/verif-module");
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| String::from("cargo"));
        let statut = Command::new(cargo)
            .current_dir(&racine)
            .env("CARGO_TARGET_DIR", &cible)
            .args([
                "build",
                "-p",
                "devine-module",
                "--target",
                "wasm32-unknown-unknown",
                "--profile",
                "module",
            ])
            .status()
            .expect("lancer cargo");
        assert!(
            statut.success(),
            "la compilation du module de devine a échoué"
        );
        std::fs::read(cible.join("wasm32-unknown-unknown/module/devine_module.wasm"))
            .expect("lire le module de devine")
    })
}

/// Une soirée de `devine` : la chaîne entière écrite, chaque geste au plus une fois, tout
/// geste accusé écrit, tout ce qui attend accusé à la fin, et la garde finale qui
/// reprend la même partie. Rend l'empreinte de la chaîne.
fn soiree_de_devine(graine: u64) -> [u8; 32] {
    let moteur = Moteur::new(ReglagesMoteur::PHASE_3).expect("le moteur démarre");
    let charge = moteur.charger(octets()).expect("le module de devine");
    let mut s = Soiree::new(|| charge.fabrique(), graine, Oui::Oui, Pannes::SOIREE);
    let b = s.jouer(4, 60_000_000);
    eprintln!("devine, graine {graine} : {b:?}");
    assert!(b.arret.is_none(), "{b:?}");
    assert!(b.redemarrages > 0, "{b:?}");
    // Devine va lentement : une manche toutes les quinze secondes environ, un essai par
    // joueur et par manche.
    assert!(b.autorite.gestes_places >= 10, "{b:?}");
    let historique = s.relais.historique();
    let h = verifier_l_historique::<InstantaneWasm>(&s.cle_de_garde(), s.salle(), &historique)
        .expect("la chaîne de la soirée");
    assert_eq!(h.en_double, Vec::<u64>::new());
    // SAL-56 : tout geste accusé est dans la chaîne écrite.
    assert_eq!(s.accuses_non_ecrits(&h), Vec::<String>::new());
    for (k, t) in s.telephones.iter().enumerate() {
        let jumeau = s.telephones[..k].iter().any(|u| u.jeton == t.jeton);
        if !jumeau {
            assert_eq!(t.appareil.en_file(), 0, "téléphone {k}");
        }
    }
    s.comparer_a_la_garde()
        .expect("la garde reprend la même partie");
    let mut e = blake3::Hasher::new();
    for (c, o) in &historique {
        e.update(format!("{c:?}").as_bytes());
        e.update(o);
    }
    *e.finalize().as_bytes()
}

#[test]
fn une_soiree_de_devine_en_memoire_survit_a_ses_pannes() {
    let _: [u8; 32] = soiree_de_devine(11);
}

/// SAL-09 : la même graine refait la même soirée, case pour case.
#[test]
fn une_soiree_en_memoire_se_refait_a_l_identique_sal09() {
    assert_eq!(soiree_de_devine(12), soiree_de_devine(12));
}

// ---------------------------------------------------------------------------
// Par la page (lot 12)
// ---------------------------------------------------------------------------

/// PHASE-3, § 8.1 (lot 12) : les téléphones passent **par la page**. L'autorité leur livre
/// le module par la connexion chiffrée, par fenêtre (P18, § 2.5), à chaque arrivée et
/// après chaque rechargement ; chacun le charge, appelle `Page` dans wasmtime, et les coups
/// du bot partent du bouton (ou de l'Entrée, du choix) qui les porte ; un singe joue en
/// plus au hasard. Sous les pannes de la graine, la chaîne écrite tient comme sans la page.
#[test]
fn une_soiree_de_devine_par_la_page_survit_a_ses_pannes() {
    let moteur = Moteur::new(ReglagesMoteur::PHASE_3).expect("le moteur démarre");
    let charge = moteur.charger(octets()).expect("le module de devine");
    let mut s = Soiree::new(|| charge.fabrique(), 21, Oui::Oui, Pannes::SOIREE);
    s.livrer_le_module(octets().to_vec());
    s.par_la_page(1);
    let b = s.jouer(4, 60_000_000);
    eprintln!("devine par la page : {b:?}");
    assert!(b.arret.is_none(), "{b:?}");
    assert!(b.autorite.morceaux_livres > 0, "{b:?}");
    assert!(b.autorite.gestes_places >= 10, "{b:?}");
    let mut somme = salle::page::ComptesJoueur::default();
    for (k, j) in &s.joueurs {
        assert_eq!(j.panne, None, "téléphone {k}");
        eprintln!("téléphone {k} : {:?}", j.comptes);
        somme.gestes_par_la_page += j.comptes.gestes_par_la_page;
        somme.tenus_par_la_page += j.comptes.tenus_par_la_page;
        somme.gestes_du_singe += j.comptes.gestes_du_singe;
    }
    assert!(somme.gestes_par_la_page > 0, "{somme:?}");
    assert!(somme.tenus_par_la_page > 0, "{somme:?}");
    assert!(somme.gestes_du_singe > 0, "{somme:?}");
    for t in &s.telephones {
        assert_eq!(t.appareil.modules_faux(), 0);
    }
    let historique = s.relais.historique();
    let h = verifier_l_historique::<InstantaneWasm>(&s.cle_de_garde(), s.salle(), &historique)
        .expect("la chaîne de la soirée");
    assert_eq!(h.en_double, Vec::<u64>::new());
    assert_eq!(s.accuses_non_ecrits(&h), Vec::<String>::new());
    s.comparer_a_la_garde()
        .expect("la garde reprend la même partie");
}

/// P18 et § 2.5 (lot 12) : un téléphone qui revient sans avoir rechargé dit le module
/// connu dans son bonjour, et ne le reçoit pas deux fois ; une page rechargée l'a perdu,
/// et le reçoit de nouveau. Le module part par fenêtre : jamais plus de
/// `FENETRE_MODULE` morceaux non accusés.
#[test]
fn le_module_ne_repart_qu_a_une_page_rechargee_p18() {
    let moteur = Moteur::new(ReglagesMoteur::PHASE_3).expect("le moteur démarre");
    let charge = moteur.charger(octets()).expect("le module de devine");
    let mut s = Soiree::new(|| charge.fabrique(), 31, Oui::Oui, Pannes::AUCUNE);
    s.livrer_le_module(octets().to_vec());
    let _: usize = s.ajouter_telephone();
    let _: usize = s.ajouter_telephone();
    let t0 = s.t_us;
    s.avancer_jusqua(t0 + 10_000_000);
    let livres = |s: &Soiree<_, _>| {
        s.autorite
            .as_ref()
            .map_or(0, |a| a.comptes().morceaux_livres)
    };
    let apres_arrivee = livres(&s);
    let par_telephone = s.telephones[0].appareil.progres_du_module().1 as u64;
    assert!(par_telephone > 0);
    assert!(s.telephones.iter().all(|t| t.appareil.module().is_some()));
    assert_eq!(
        apres_arrivee,
        2 * par_telephone,
        "un module entier par téléphone"
    );
    // Coupé, puis revenu sans recharger : rien ne repart.
    s.couper(0, Some(1_000_000));
    let t = s.t_us;
    s.avancer_jusqua(t + 6_000_000);
    assert!(s.telephones[0].appareil.accueilli());
    assert_eq!(livres(&s), apres_arrivee, "le module connu ne repart pas");
    // Rechargé : la page a perdu le module, il repart entier.
    s.recharger(0, 1_000_000);
    let t = s.t_us;
    s.avancer_jusqua(t + 6_000_000);
    assert!(s.telephones[0].appareil.module().is_some());
    assert_eq!(livres(&s), apres_arrivee + par_telephone);
}

// ---------------------------------------------------------------------------
// L'entrée qui bloque (P39, lot 10a)
// ---------------------------------------------------------------------------

/// Une fabrique dont les instances « bloquent » sur un geste : le premier qu'elles voient
/// entrer une fois armées, puis le même à chaque fois. On l'arme après quelques secondes
/// de soirée : l'identité d'un geste tient à son appareil, qu'une reprise ne retrouve
/// que si le registre qui l'inscrit a été écrit (une case suit l'arrivée de 100 ms). Bloquer, ici, c'est rendre la panne
/// `GardeFouMural`, ce que rend wasmtime quand son garde-fou coupe un appel : l'autorité
/// s'arrête et ne bat plus, comme un Worker figé que la page doit tuer.
struct Bloquante {
    fabrique: moteur_wasmtime::FabriqueWasm,
    poison: Rc<Cell<Option<u64>>>,
    arme: Rc<Cell<bool>>,
}

struct InstanceBloquante {
    instance: <moteur_wasmtime::FabriqueWasm as Fabrique>::I,
    poison: Rc<Cell<Option<u64>>>,
    arme: Rc<Cell<bool>>,
}

impl Fabrique for Bloquante {
    type I = InstanceBloquante;
    fn neuve(&mut self) -> Result<InstanceBloquante, PanneOutil> {
        Ok(InstanceBloquante {
            instance: self.fabrique.neuve()?,
            poison: Rc::clone(&self.poison),
            arme: Rc::clone(&self.arme),
        })
    }
}

impl Instance for InstanceBloquante {
    type Instantane = InstantaneWasm;
    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8], budget: u64) -> Result<Vec<u8>, Echec> {
        if let Some(id) = geste_de_l_appel(appel)
            && self.arme.get()
        {
            let poison = self.poison.get().unwrap_or(id.fil());
            self.poison.set(Some(poison));
            if id.fil() == poison {
                return Err(Echec::Panne(PanneOutil::new(
                    CausePanne::GardeFouMural,
                    "le geste empoisonné ne rend pas la main",
                )));
            }
        }
        self.instance.appeler(sorte, appel, budget)
    }
    fn carburant(&self) -> Option<u64> {
        self.instance.carburant()
    }
    fn instantane(&mut self) -> Result<InstantaneWasm, PanneOutil> {
        self.instance.instantane()
    }
    fn restaurer(&mut self, i: &InstantaneWasm) -> Result<(), PanneOutil> {
        self.instance.restaurer(i)
    }
    fn empreinte(&mut self) -> Result<[u8; 32], PanneOutil> {
        self.instance.empreinte()
    }
}

/// P39 (CONTRAT, « La forme d'un jeu » : une entrée qui fait échouer le jeu est isolée et
/// écartée ; HIS-06, OUT-11 : un arrêt mural est une panne de l'outil). Un geste fait
/// s'arrêter l'autorité pendant son appel : le chien de garde de la page la relance ; le
/// téléphone, sans accusé, le renvoie, et elle s'arrête de nouveau ; au second arrêt sur
/// le même geste, le Worker neuf reçoit `Reprendre { ecarter }`, la salle refuse le geste
/// au téléphone (`Ecarte`), la page le dit en panne de l'outil en le nommant, et la soirée
/// continue : les gestes suivants comptent, rien d'autre n'est perdu, le geste écarté
/// n'est pas dans la chaîne écrite, et la file du téléphone se vide.
#[test]
#[expect(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "un seul scénario, de bout en bout, qui se lit d'une traite"
)]
fn une_entree_qui_bloque_deux_fois_est_ecartee() {
    let moteur = Moteur::new(ReglagesMoteur::PHASE_3).expect("le moteur démarre");
    let charge = moteur.charger(octets()).expect("le module de devine");
    let poison: Rc<Cell<Option<u64>>> = Rc::new(Cell::new(None));
    let arme = Rc::new(Cell::new(false));
    let page: Rc<RefCell<Vec<Vec<u8>>>> = Rc::new(RefCell::new(Vec::new()));
    let dire: Dire = {
        let page = Rc::clone(&page);
        Rc::new(move |id| {
            page.borrow_mut()
                .push(VersPage::EnCours(id).encoder().unwrap());
        })
    };
    let p = Rc::clone(&poison);
    let a = Rc::clone(&arme);
    let mut s = Soiree::new(
        move || {
            FabriqueQuiDit::new(
                Bloquante {
                    fabrique: charge.fabrique(),
                    poison: Rc::clone(&p),
                    arme: Rc::clone(&a),
                },
                Rc::clone(&dire),
            )
        },
        21,
        Oui::Oui,
        Pannes::AUCUNE,
    );
    {
        let page = Rc::clone(&page);
        s.observer_la_page(move |o| page.borrow_mut().push(o.to_vec()));
    }
    for _ in 0..3 {
        let _: usize = s.ajouter_telephone();
    }
    let mut chien = Chien::new();
    let ms = |s: &Soiree<_, _>| s.t_us / 1000;
    chien.demarre(ms(&s));
    let mut relances = Vec::new();
    let mut pannes = Vec::new();
    let fin = s.t_us + 90_000_000;
    let armer = s.t_us + 10_000_000;
    while s.t_us < fin {
        if s.t_us >= armer {
            arme.set(true);
        }
        s.avancer_jusqua(s.t_us + TIC_MS * 1000);
        let t = ms(&s);
        for o in page.borrow_mut().drain(..) {
            let m = VersPage::decoder(&o).unwrap();
            if let VersPage::Panne(r) = &m {
                pannes.push(r.detail.to_owned());
            }
            let _: bool = chien.message(t, &m);
        }
        match chien.tic(t) {
            Decision::Rien => {}
            Decision::Relancer { ecarter, .. } => {
                relances.push(ecarter);
                if let Some(e) = ecarter {
                    s.a_la_relance(
                        DeLaPage::Commande(CommandeEcran::Reprendre { ecarter: Some(e) })
                            .encoder()
                            .unwrap(),
                    );
                }
                // La page tue le Worker figé et en lance un neuf.
                s.tuer_l_autorite(200_000);
                chien.demarre(t);
            }
            Decision::Panne { .. } => panic!("le commun en panne : {relances:?} {pannes:?}"),
        }
    }
    let poison = poison.get().expect("un geste a bloqué");
    assert_eq!(
        relances,
        vec![None, Some(IdGeste::du_fil(poison))],
        "deux arrêts sur le même geste, puis plus aucun"
    );
    let a = s.autorite.as_ref().expect("l'autorité vit");
    assert!(a.arret().is_none(), "{:?}", a.arret());
    assert_eq!(a.comptes().gestes_ecartes, 1);
    assert!(
        a.comptes().gestes_places > 0,
        "la soirée continue après l'écart : {:?}",
        a.comptes()
    );
    let nomme = format!("{poison:016x}");
    assert!(
        pannes
            .iter()
            .any(|d| d.contains("deux arrêts muraux") && d.contains(&nomme)),
        "la page dit l'entrée écartée, nommée : {pannes:?}"
    );
    s.calmer();
    let historique = s.relais.historique();
    let h = verifier_l_historique::<InstantaneWasm>(&s.cle_de_garde(), s.salle(), &historique)
        .expect("la chaîne de la soirée");
    assert!(
        !h.gestes.contains(&poison),
        "le geste écarté n'est pas écrit"
    );
    assert_eq!(h.en_double, Vec::<u64>::new());
    assert_eq!(s.accuses_non_ecrits(&h), Vec::<String>::new());
    for (k, t) in s.telephones.iter().enumerate() {
        assert_eq!(t.appareil.en_file(), 0, "téléphone {k}");
    }
    s.comparer_a_la_garde()
        .expect("la garde reprend la même partie");
}

/// SAL-162 : F8 pose une marque au journal. Elle part à la garde avec la case suivante,
/// même sans ligne nouvelle, dans le registre que chaque case porte ; une autorité reprise
/// de la garde la retrouve, et la garde relue la donne à son indice, au bord d'un pas.
#[test]
fn une_marque_f8_va_a_la_garde_et_survit_a_la_reprise_sal162() {
    let moteur = Moteur::new(ReglagesMoteur::PHASE_3).expect("le moteur démarre");
    let charge = moteur.charger(octets()).expect("le module de devine");
    let mut s = Soiree::new(|| charge.fabrique(), 22, Oui::Oui, Pannes::AUCUNE);
    for _ in 0..2 {
        let _: usize = s.ajouter_telephone();
    }
    let marque = DeLaPage::Commande(CommandeEcran::Marque).encoder().unwrap();
    s.avancer_jusqua(s.t_us + 5_000_000);
    s.depuis_la_page(&marque);
    s.avancer_jusqua(s.t_us + 3_000_000);
    s.depuis_la_page(&marque);
    s.avancer_jusqua(s.t_us + 1_000_000);
    let avant = s.autorite.as_ref().unwrap().registre().unwrap().marques;
    assert_eq!(avant.len(), 2, "{avant:?}");
    assert!(avant[0].indice < avant[1].indice && avant[0].pas < avant[1].pas);
    // La garde relue les porte, à des bords de pas du journal écrit.
    let cases = s.relais.cases();
    let lue = salle::garde::relire::<InstantaneWasm>(&s.cle_de_garde(), s.salle(), &cases, None)
        .expect("la garde se relit");
    assert_eq!(lue.registre.marques, avant);
    // Une autorité reprise les retrouve, et en ajoute.
    s.tuer_l_autorite(300_000);
    s.avancer_jusqua(s.t_us + 2_000_000);
    s.depuis_la_page(&marque);
    s.avancer_jusqua(s.t_us + 1_000_000);
    let apres = s.autorite.as_ref().unwrap().registre().unwrap().marques;
    assert_eq!(apres.len(), 3, "{apres:?}");
    assert_eq!(&apres[..2], &avant[..]);
    assert_eq!(s.autorite.as_ref().unwrap().comptes().marques, 1);
}

/// Lot 10a, correction après la vérification : un point de reprise se scelle une tranche
/// par réveil, et non d'un coup dans le tour qui le prend (un point de Punchline, scellé
/// d'un coup, tenait le Worker de Firefox jusqu'à 24 ms, et le jeu sautait un pas). Le
/// tour où il commence ne l'écrit pas ; il part plusieurs réveils plus tard ; aucune case
/// de journal ne passe devant lui ; une autorité tuée pendant qu'il se scelle reprend du
/// point d'avant ; la chaîne entière se vérifie, et la garde reprend la même partie.
#[test]
fn un_point_se_scelle_par_tranches_sur_plusieurs_reveils() {
    let moteur = Moteur::new(ReglagesMoteur::PHASE_3).expect("le moteur démarre");
    let charge = moteur.charger(octets()).expect("le module de devine");
    let mut s = Soiree::new(|| charge.fabrique(), 23, Oui::Oui, Pannes::AUCUNE);
    s.regler_la_tranche_du_point(1024);
    for _ in 0..2 {
        let _: usize = s.ajouter_telephone();
    }
    // Pas après pas, jusqu'à deux points commencés : le point compte quand il part.
    let mut commences = 0;
    let mut reveils_par_point = Vec::new();
    let mut en_scellement: Option<u64> = None;
    let mut journal_au_debut = 0;
    let mut tue = false;
    let mut depuis = s.autorite.as_ref().unwrap().depuis_le_point();
    let fin = s.t_us + 60_000_000;
    while s.t_us < fin && reveils_par_point.len() < 2 {
        let points = s.autorite.as_ref().map_or(0, |a| a.comptes().points);
        s.avancer_jusqua(s.t_us + 16_667);
        let Some(a) = s.autorite.as_ref() else {
            continue;
        };
        let d = a.depuis_le_point();
        if d < depuis && depuis.is_some_and(|x| x > 100) {
            // Un point vient de commencer : il n'est pas écrit dans ce tour.
            commences += 1;
            assert_eq!(
                a.comptes().points,
                points,
                "le point s'écrit dans le tour qui le prend"
            );
            en_scellement = Some(0);
            journal_au_debut = a.comptes().cases_journal;
        }
        depuis = d;
        if let Some(n) = en_scellement.as_mut() {
            *n += 1;
            if a.comptes().points > points {
                reveils_par_point.push(*n);
                en_scellement = None;
            } else {
                // Aucune case de journal ne se scelle devant le point.
                assert_eq!(
                    a.comptes().cases_journal,
                    journal_au_debut,
                    "une case de journal passe devant le point"
                );
                if !tue && *n == 3 && commences == 1 {
                    // Tuée pendant le scellement : la reprise part du point d'avant.
                    tue = true;
                    en_scellement = None;
                    s.tuer_l_autorite(300_000);
                    s.avancer_jusqua(s.t_us + 1_000_000);
                    s.comparer_a_la_garde()
                        .expect("la garde reprend la même partie après un point perdu");
                    depuis = s.autorite.as_ref().unwrap().depuis_le_point();
                }
            }
        }
    }
    assert!(tue, "l'autorité n'a pas été tuée pendant un scellement");
    assert_eq!(reveils_par_point.len(), 2, "{reveils_par_point:?}");
    assert!(
        reveils_par_point.iter().all(|&n| n > 3),
        "le point s'est scellé en trop peu de réveils : {reveils_par_point:?}"
    );
    s.avancer_jusqua(s.t_us + 2_000_000);
    let historique = s.relais.historique();
    let h = verifier_l_historique::<InstantaneWasm>(&s.cle_de_garde(), s.salle(), &historique)
        .expect("la chaîne de la soirée");
    assert_eq!(h.en_double, Vec::<u64>::new());
    s.comparer_a_la_garde()
        .expect("la garde reprend la même partie");
    // La minuterie de secours (P6) : juste après une image, rien ne presse ; attendre
    // deux pas de plus ferait sauter un jeu qui ne rattrape qu'un pas.
    s.avancer_jusqua(s.t_us + 100_000);
    let a = s.autorite.as_ref().unwrap();
    assert!(!a.secours_presse(s.t_us, 0));
    assert!(a.secours_presse(s.t_us, 2 * 16_667 + 1));
}
