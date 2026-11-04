//! Les deux modes de l'allocateur, simulés en natif (INTERFACE.md, § 3.2 et § 8.3) :
//! `lecture_n_alloue_pas_dans_le_tas` et `ecriture_decode_en_mode_tas`.
//!
//! Un allocateur global de test sert une arène en mode brouillon et le système en mode
//! tas, comme `jeu::colle::wasm::Allocateur` dans le module. L'hôte de test fait ce que
//! font les exports : il vide l'arène au début de chaque appel, en la remplissant de
//! `0xAA` pour qu'une valeur restée dans le brouillon se voie, et choisit le mode selon
//! la sorte de l'appel. Sans le harnais de `cargo test` : un seul fil.

#![expect(
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::unwrap_used,
    clippy::wildcard_enum_match_arm,
    clippy::let_underscore_untyped,
    reason = "code de test : un échec s'y lit comme l'échec du test qui l'appelle"
)]
#![expect(unsafe_code, reason = "un allocateur global de test, à deux modes")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use devine::{Devine, GesteTel, Prenom};
use jeu::colle::{Cadre, Enveloppe, Mode, Modes, Sortie, Zones, appeler, reserver};
use jeu::prelude::*;
use jeu_abi::{Appel, EntreeFil, QuandFil, RegardFil, Retour, StyleFil, decoder, encoder};

const TAILLE: usize = 16 << 20;
static mut ARENE: [u8; TAILLE] = [0; TAILLE];
static HAUT: AtomicUsize = AtomicUsize::new(0);
/// Les allocations du système faites pendant une lecture.
static TAS_EN_LECTURE: AtomicUsize = AtomicUsize::new(0);
/// Les bascules en mode tas demandées par la colle pendant une lecture.
static BASCULES_EN_LECTURE: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static BROUILLON: Cell<bool> = const { Cell::new(false) };
    static LECTURE: Cell<bool> = const { Cell::new(false) };
}

fn debut() -> usize {
    (&raw mut ARENE).cast::<u8>().addr()
}

struct DeuxModes;

// SAFETY : l'arène rend des blocs alignés, disjoints, jamais rendus avant la remise à zéro
// qu'ordonne l'hôte de test entre deux appels ; le reste va au système.
unsafe impl GlobalAlloc for DeuxModes {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if BROUILLON.with(Cell::get) {
            let d = debut();
            let haut = d + HAUT.load(Ordering::Relaxed);
            return match reserver(d, d + TAILLE, haut, l.size(), l.align()) {
                Some((a, b)) => {
                    HAUT.store(b - d, Ordering::Relaxed);
                    (&raw mut ARENE).cast::<u8>().wrapping_add(a - d)
                }
                None => std::ptr::null_mut(),
            };
        }
        if LECTURE.with(Cell::get) {
            TAS_EN_LECTURE.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY : la disposition vient de l'appelant.
        unsafe { System.alloc(l) }
    }

    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        let d = debut();
        if (d..d + TAILLE).contains(&p.addr()) {
            return;
        }
        // SAFETY : `p` vient du système, avec cette disposition.
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static ALLOCATEUR: DeuxModes = DeuxModes;

struct ModesDeTest;

impl Modes for ModesDeTest {
    fn bascule(&self, m: Mode) {
        if m == Mode::Tas && LECTURE.with(Cell::get) {
            BASCULES_EN_LECTURE.fetch_add(1, Ordering::Relaxed);
        }
        BROUILLON.with(|b| b.set(m == Mode::Brouillon));
    }
    fn memoire(&self) -> (u64, u64) {
        (0, 0)
    }
}

/// Vide l'arène, et la remplit de `0xAA`.
fn remettre_a_zero() {
    let haut = HAUT.swap(0, Ordering::Relaxed);
    // SAFETY : un seul fil ; rien de vivant n'est dans l'arène entre deux appels, par la
    // règle même qu'on éprouve ; `haut` ne dépasse pas sa taille.
    unsafe { (&raw mut ARENE).cast::<u8>().write_bytes(0xAA, haut) };
}

/// Un appel, comme le font les exports : remise à zéro, mode selon la sorte, puis copie
/// de la sortie en mode tas. `mode_ecriture` est le mode d'une écriture (`Tas` d'habitude).
fn appel(env: &mut Option<Enveloppe<Devine>>, a: &Appel<'_>, mode_ecriture: Mode) -> Vec<u8> {
    let mut octets = Vec::new();
    encoder(a, &mut octets).unwrap();
    remettre_a_zero();
    let sorte = a.sorte();
    let lecture = sorte == jeu_abi::SorteAppel::Lecture;
    LECTURE.with(|l| l.set(lecture));
    let modes = ModesDeTest;
    modes.bascule(if lecture {
        Mode::Brouillon
    } else {
        mode_ecriture
    });
    let cadre = Cadre {
        modes: &modes,
        zones: Zones::default(),
    };
    let mut s = Sortie::new(cadre, sorte);
    let _ = appeler(env, &octets, &mut s);
    LECTURE.with(|l| l.set(false));
    modes.bascule(Mode::Tas);
    s.octets.clone()
}

fn entree(env: &mut Option<Enveloppe<Devine>>, place: u64, e: &EntreeFil<'_>) {
    let mut o = Vec::new();
    encoder(e, &mut o).unwrap();
    let r = appel(
        env,
        &Appel::Entree {
            ordre: place,
            place,
            entree: &o,
        },
        Mode::Tas,
    );
    assert!(matches!(
        decoder::<Retour<'_>>(&r).unwrap(),
        Retour::Issue(_)
    ));
}

fn geste(env: &mut Option<Enveloppe<Devine>>, de: u64, g: &GesteTel, place: u64) {
    let mut o = Vec::new();
    encoder(g, &mut o).unwrap();
    let quand = QuandFil {
        fait: place,
        fraction: 0,
        recu: place,
        vu: place,
        id: place,
    };
    entree(
        env,
        place,
        &EntreeFil::Geste {
            de,
            octets: &o,
            quand,
        },
    );
}

fn creer(env: &mut Option<Enveloppe<Devine>>) {
    let mut presents = Vec::new();
    encoder(&Vec::<Presence>::new(), &mut presents).unwrap();
    let a = Appel::Creer {
        graine: [1; 32],
        pas: 0,
        heure: 0,
        salle: TexteCourt::new("S"),
        presents: &presents,
        garde: None,
    };
    let _ = appel(env, &a, Mode::Tas);
}

fn canonique(env: &mut Option<Enveloppe<Devine>>) -> Vec<u8> {
    let r = appel(env, &Appel::Canonique { morceau: 0 }, Mode::Tas);
    match decoder::<Retour<'_>>(&r).unwrap() {
        Retour::Morceau {
            octets,
            suivant: None,
        } => octets.to_vec(),
        autre => panic!("{autre:?}"),
    }
}

fn contient(foin: &[u8], aiguille: &[u8]) -> bool {
    foin.windows(aiguille.len()).any(|w| w == aiguille)
}

/// Une entrée dont le texte entre dans l'état reste lisible après la remise à zéro du
/// brouillon : la colle décode en mode tas ce qui peut entrer dans l'état (§ 3.2).
fn ecriture_decode_en_mode_tas() {
    let mut env = None;
    creer(&mut env);
    geste(
        &mut env,
        7,
        &GesteTel::Rejoindre(Texte::<Prenom>::nettoyer("Zoé")),
        1,
    );
    // L'écriture suivante, puis une lecture : l'arène a été remplie de 0xAA deux fois.
    let _ = appel(
        &mut env,
        &Appel::Regarder {
            regards: vec![RegardFil::Ecran],
        },
        Mode::Tas,
    );
    let c = canonique(&mut env);
    assert!(
        contient(&c, "Zoé".as_bytes()),
        "le prénom a été écrasé : il vivait dans le brouillon"
    );
}

/// Une lecture n'alloue rien dans le tas, et la colle n'y repasse jamais (§ 3.2, D4).
fn lecture_n_alloue_pas_dans_le_tas() {
    let mut env = None;
    creer(&mut env);
    geste(
        &mut env,
        7,
        &GesteTel::Rejoindre(Texte::<Prenom>::nettoyer("Zoé")),
        1,
    );
    let mut o = Vec::new();
    encoder(&devine::ActionEcran::Lancer, &mut o).unwrap();
    let quand = QuandFil {
        fait: 2,
        fraction: 0,
        recu: 2,
        vu: 2,
        id: 2,
    };
    entree(
        &mut env,
        2,
        &EntreeFil::Ecran {
            quoi: jeu_abi::EcranFil::Action(&o),
            quand,
        },
    );
    let avant = canonique(&mut env);
    TAS_EN_LECTURE.store(0, Ordering::Relaxed);
    BASCULES_EN_LECTURE.store(0, Ordering::Relaxed);
    let lectures = [
        Appel::Bonjour,
        Appel::Decrire,
        Appel::Regarder {
            regards: vec![
                RegardFil::Ecran,
                RegardFil::Appareil(7),
                RegardFil::Appareil(8),
            ],
        },
        Appel::Etat { connues: vec![] },
        Appel::Ressources { cles: vec![1, 2] },
        Appel::Projeter {
            regard: RegardFil::Appareil(7),
            morceau: 0,
        },
        Appel::Canonique { morceau: 0 },
        Appel::Garder,
        Appel::Resumer,
        Appel::Verifier,
        Appel::Bot {
            regard: RegardFil::Appareil(8),
            style: StyleFil::Hostile,
            graine: 3,
            depuis_vue: 0,
        },
        Appel::Bot {
            regard: RegardFil::Ecran,
            style: StyleFil::Sage,
            graine: 3,
            depuis_vue: 500,
        },
        Appel::Memoire,
    ];
    for a in &lectures {
        let r = appel(&mut env, a, Mode::Tas);
        assert!(
            !matches!(decoder::<Retour<'_>>(&r).unwrap(), Retour::Illisible(_)),
            "{a:?}"
        );
    }
    assert_eq!(
        TAS_EN_LECTURE.load(Ordering::Relaxed),
        0,
        "une lecture a alloué dans le tas"
    );
    assert_eq!(
        BASCULES_EN_LECTURE.load(Ordering::Relaxed),
        0,
        "la colle est repassée en mode tas"
    );
    assert_eq!(canonique(&mut env), avant, "une lecture a changé l'état");
}

fn main() {
    for (nom, f) in [
        (
            "ecriture_decode_en_mode_tas",
            ecriture_decode_en_mode_tas as fn(),
        ),
        (
            "lecture_n_alloue_pas_dans_le_tas",
            lecture_n_alloue_pas_dans_le_tas,
        ),
    ] {
        f();
        println!("test {nom} ... ok");
    }
    println!("test result: ok. 2 passed; 0 failed");
}
