//! L'étalonnage des budgets de E19 : ce que coûte, en temps, une unité de carburant et
//! un appel, dans wasmtime, sur la machine qui le lance (`budgets.rs`, « D'où viennent
//! les budgets »).
//!
//! ```text
//! cargo run --release -p epreuve --example etalonner -- <module.wasm>…
//! ```
//!
//! Pour chaque module, quatre mesures :
//!
//! 1. **La remise à zéro des zones.** Le moteur remet à zéro, avant chaque appel, la
//!    pile, la boîte, le brouillon et la zone de panique (`preparer`) : un
//!    `fill(0)` de [`moteur_wasmtime::ModuleCharge::octets_remis_a_zero`] octets, que
//!    le carburant ne compte pas. Mesurée ici sur un tampon de la même taille, déjà
//!    touché, comme le sont les zones après le premier appel.
//! 2. **Le coût fixe d'un appel.** `Bonjour`, deux mille fois sur une instance nue :
//!    son carburant est de quelques milliers d'unités ; sa durée est presque tout
//!    entière le passage de l'appel au fil de l'instance, l'entrée dans wasmtime, la
//!    remise à zéro et la copie du retour.
//! 3. **La durée d'une unité de carburant.** Chaque appel d'une soirée du plan court
//!    (E9 et E14 rejouent la soirée entrée par entrée sur une instance chronométrée,
//!    passée à l'épreuve à la place de la copie native), et la vue de l'écran, l'état
//!    et les invariants de chaque pire cas : pour chaque appel d'au moins
//!    [`LOURD`] unités, `(durée − coût fixe) / carburant`.
//! 4. **Le chargement**, pour mémoire : sa durée, et le carburant de `Bonjour` et de
//!    `Decrire` sous `BUDGET_DE_CHARGEMENT` (celui de `Creer` est dans la soirée, par
//!    sorte d'appel).
//!
//! Les durées ne valent que pour cette machine, et pour wasmtime : ni pour un
//! navigateur, ni pour l'ordinateur de la soirée. Une mesure porte la date et la
//! machine où elle a été prise ; ce programme ne les connaît pas, c'est à qui le lance
//! de les écrire à côté des chiffres.

#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, Instant};

use epreuve::{IdControle, Plan};
use jeu_abi::{Appel, RegardFil, SorteAppel, decoder, encoder};
use moteur_wasmtime::{
    FabriqueWasm, InstanceWasm, InstantaneWasm, ModuleCharge, Moteur, ReglagesMoteur,
};
use partie::{Echec, Fabrique, Instance, PanneOutil};

/// Le carburant à partir duquel un appel compte pour la durée d'une unité : en
/// dessous, le coût fixe et le bruit de la mesure l'emportent.
const LOURD: u64 = 100_000;
/// … et un seuil plus haut, où le coût fixe ne pèse presque plus.
const TRES_LOURD: u64 = 1_000_000;
/// Les `Bonjour` de la mesure du coût fixe.
const BONJOURS: usize = 2000;
/// Les répétitions de chaque lecture d'un pire cas.
const REPETITIONS: usize = 7;

/// Un appel chronométré.
#[derive(Debug, Clone, Copy)]
struct Mesure {
    nom: &'static str,
    carburant: u64,
    duree: Duration,
}

type Mesures = Rc<RefCell<Vec<Mesure>>>;

const fn nom(a: &Appel<'_>) -> &'static str {
    match a {
        Appel::Bonjour => "Bonjour",
        Appel::Decrire => "Decrire",
        Appel::Creer { .. } => "Creer",
        Appel::Valider { .. } => "Valider",
        Appel::Entree { .. } => "Entree",
        Appel::Regarder { .. } => "Regarder",
        Appel::Etat { .. } => "Etat",
        Appel::Ressources { .. } => "Ressources",
        Appel::Projeter { .. } => "Projeter",
        Appel::Charger { .. } => "Charger",
        Appel::Canonique { .. } => "Canonique",
        Appel::DepuisCanonique { .. } => "DepuisCanonique",
        Appel::Garder => "Garder",
        Appel::Resumer => "Resumer",
        Appel::Verifier => "Verifier",
        Appel::Transition { .. } => "Transition",
        Appel::Bot { .. } => "Bot",
        Appel::PireCas { .. } => "PireCas",
        Appel::Memoire => "Memoire",
        Appel::Proprietes => "Proprietes",
        Appel::Hypothese { .. } => "Hypothese",
        Appel::Page { .. } => "Page",
    }
}

/// Une instance du module, dont chaque appel est chronométré.
struct Chronometree {
    inner: InstanceWasm,
    mesures: Mesures,
}

impl Instance for Chronometree {
    type Instantane = InstantaneWasm;

    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8], budget: u64) -> Result<Vec<u8>, Echec> {
        let n = decoder::<Appel<'_>>(appel).map_or("illisible", |a| nom(&a));
        let debut = maintenant();
        let r = Instance::appeler(&mut self.inner, sorte, appel, budget);
        let duree = debut.elapsed();
        if r.is_ok()
            && let Some(carburant) = Instance::carburant(&self.inner)
        {
            self.mesures.borrow_mut().push(Mesure {
                nom: n,
                carburant,
                duree,
            });
        }
        r
    }

    fn carburant(&self) -> Option<u64> {
        Instance::carburant(&self.inner)
    }

    fn instantane(&mut self) -> Result<InstantaneWasm, PanneOutil> {
        Instance::instantane(&mut self.inner)
    }

    fn restaurer(&mut self, i: &InstantaneWasm) -> Result<(), PanneOutil> {
        Instance::restaurer(&mut self.inner, i)
    }

    fn empreinte(&mut self) -> Result<[u8; 32], PanneOutil> {
        Instance::empreinte(&mut self.inner)
    }
}

/// Des instances chronométrées.
struct Chrono {
    inner: FabriqueWasm,
    mesures: Mesures,
}

impl Fabrique for Chrono {
    type I = Chronometree;

    fn neuve(&mut self) -> Result<Chronometree, PanneOutil> {
        Ok(Chronometree {
            inner: Fabrique::neuve(&mut self.inner)?,
            mesures: Rc::clone(&self.mesures),
        })
    }
}

fn encode(a: &Appel<'_>) -> Vec<u8> {
    let mut o = Vec::new();
    match encoder(a, &mut o) {
        Ok(()) => o,
        Err(_) => Vec::new(),
    }
}

/// L'heure de la machine : ce programme mesure des durées, c'est son objet, et aucune
/// règle de jeu n'en dépend.
#[expect(
    clippy::disallowed_methods,
    reason = "l'étalonnage mesure des durées murales : c'est son objet, et rien d'un jeu n'en dépend"
)]
fn maintenant() -> Instant {
    Instant::now()
}

fn ns(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
}

/// La médiane d'une suite.
fn mediane(v: &mut [u64]) -> u64 {
    v.sort_unstable();
    v.get(v.len() / 2).copied().unwrap_or(0)
}

/// Un centile d'une suite triée.
fn centile(triees: &[u64], pour_cent: usize) -> u64 {
    let rang = triees.len().saturating_mul(pour_cent).div_ceil(100).max(1);
    triees.get(rang.saturating_sub(1)).copied().unwrap_or(0)
}

/// 1. La durée d'un `fill(0)` de `octets` octets, déjà touchés : la médiane de
///    quinze, en nanosecondes.
fn remise_a_zero(octets: u64) -> u64 {
    let n = usize::try_from(octets).unwrap_or(0);
    let mut tampon = vec![1u8; n];
    let mut durees: Vec<u64> = (0..15)
        .map(|k| {
            // Le tampon est réécrit entre deux mesures, comme une zone qu'un appel
            // a remplie : `fill(0)` ne trouve pas des pages déjà nulles.
            tampon.fill(u8::try_from(k % 200).unwrap_or(1).saturating_add(1));
            let debut = maintenant();
            tampon.fill(0);
            let d = ns(debut.elapsed());
            std::hint::black_box(&tampon);
            d
        })
        .collect();
    mediane(&mut durees)
}

/// Une instance nue, accueillie.
fn nue(f: &mut Chrono) -> Result<Chronometree, String> {
    let mut i = Fabrique::neuve(f).map_err(|p| p.to_string())?;
    for a in [Appel::Bonjour, Appel::Decrire] {
        let _: Vec<u8> = Instance::appeler(&mut i, SorteAppel::Lecture, &encode(&a), 1 << 28)
            .map_err(|e| e.to_string())?;
    }
    Ok(i)
}

/// 2. Le coût fixe : `Bonjour` sur une instance nue, [`BONJOURS`] fois.
fn bonjours(f: &mut Chrono) -> Result<Vec<Mesure>, String> {
    let mut i = nue(f)?;
    f.mesures.borrow_mut().clear();
    let octets = encode(&Appel::Bonjour);
    for _ in 0..BONJOURS {
        let _: Vec<u8> = Instance::appeler(&mut i, SorteAppel::Lecture, &octets, 1 << 28)
            .map_err(|e| e.to_string())?;
    }
    Ok(f.mesures.borrow_mut().drain(..).collect())
}

/// 3. Les lectures de chaque pire cas, [`REPETITIONS`] fois chacune.
fn pires_cas(f: &mut Chrono) -> Result<Vec<Mesure>, String> {
    let lectures = [
        Appel::Regarder {
            regards: vec![RegardFil::Ecran],
        },
        Appel::Etat {
            connues: Vec::new(),
        },
        Appel::Verifier,
    ];
    let mut toutes = Vec::new();
    for indice in 0..=u32::from(u16::MAX) {
        let mut i = nue(f)?;
        let pire = Instance::appeler(
            &mut i,
            SorteAppel::Ecriture,
            &encode(&Appel::PireCas { indice }),
            1 << 28,
        );
        let accepte = pire.as_deref().map(decoder::<jeu_abi::Retour<'_>>);
        if !matches!(accepte, Ok(Ok(jeu_abi::Retour::PireCas { .. }))) {
            break;
        }
        for a in &lectures {
            let octets = encode(a);
            for _ in 0..REPETITIONS {
                let _: Result<Vec<u8>, Echec> =
                    Instance::appeler(&mut i, SorteAppel::Lecture, &octets, 1 << 28);
            }
        }
        toutes.extend(f.mesures.borrow_mut().drain(..));
    }
    Ok(toutes)
}

/// 3. Les appels d'une soirée du plan court, rejoués par E9 et E14 sur des instances
///    chronométrées.
fn soiree(module: &ModuleCharge, f: &mut Chrono) -> Vec<Mesure> {
    f.mesures.borrow_mut().clear();
    let mut plan = Plan::court([0x5e; 32]);
    plan.controles = vec![IdControle::E9, IdControle::E14];
    let rapport = epreuve::eprouver(module, Some(f), &plan);
    println!(
        "  soirée du plan court (E9, E14) : code des contrôles {}",
        rapport.code_des_controles()
    );
    f.mesures.borrow_mut().drain(..).collect()
}

/// Les picosecondes par unité de chaque appel lourd, au-delà du coût fixe.
fn par_unite(mesures: &[Mesure], fixe_ns: u64, seuil: u64) -> Vec<u64> {
    let mut v: Vec<u64> = mesures
        .iter()
        .filter(|m| m.carburant >= seuil)
        .map(|m| {
            ns(m.duree)
                .saturating_sub(fixe_ns)
                .saturating_mul(1000)
                .checked_div(m.carburant)
                .unwrap_or(0)
        })
        .collect();
    v.sort_unstable();
    v
}

fn decrire_par_unite(quoi: &str, mesures: &[Mesure], fixe_ns: u64) {
    for seuil in [LOURD, TRES_LOURD] {
        decrire_seuil(quoi, &par_unite(mesures, fixe_ns, seuil), seuil);
    }
}

fn decrire_seuil(quoi: &str, v: &[u64], seuil: u64) {
    if v.is_empty() {
        println!("  {quoi} : aucun appel d'au moins {seuil} unités");
    } else {
        println!(
            "  {quoi} : {} appels d'au moins {seuil} unités ; ps par unité : médiane {}, 90ᵉ centile {}, maximum {}",
            v.len(),
            centile(v, 50),
            centile(v, 90),
            v.last().copied().unwrap_or(0)
        );
    }
}

fn par_sorte(mesures: &[Mesure]) {
    let mut noms: Vec<&'static str> = mesures.iter().map(|m| m.nom).collect();
    noms.sort_unstable();
    noms.dedup();
    for n in noms {
        let mut durees: Vec<u64> = mesures
            .iter()
            .filter(|m| m.nom == n)
            .map(|m| ns(m.duree))
            .collect();
        let mut carburants: Vec<u64> = mesures
            .iter()
            .filter(|m| m.nom == n)
            .map(|m| m.carburant)
            .collect();
        durees.sort_unstable();
        carburants.sort_unstable();
        println!(
            "    {n} : {} appels ; durée médiane {} ns, maximum {} ns ; carburant médian {}, maximum {}",
            durees.len(),
            centile(&durees, 50),
            durees.last().copied().unwrap_or(0),
            centile(&carburants, 50),
            carburants.last().copied().unwrap_or(0)
        );
    }
}

fn etalonner(moteur: &Moteur, chemin: &str) -> Result<(), String> {
    let octets = std::fs::read(chemin).map_err(|e| format!("{chemin} : {e}"))?;
    let debut = maintenant();
    let module = moteur.charger(&octets).map_err(|r| r.to_string())?;
    let chargement = debut.elapsed();
    let zones = module.octets_remis_a_zero();
    println!("{chemin}");
    println!("  chargement : {} ms", chargement.as_millis());
    let (bonjour, decrire) = module.carburant_du_chargement();
    println!(
        "  carburant du chargement : Bonjour {bonjour}, Decrire {decrire} (BUDGET_DE_CHARGEMENT {})",
        moteur_wasmtime::BUDGET_DE_CHARGEMENT
    );

    let zero_ns = remise_a_zero(zones);
    println!(
        "  1. remise à zéro : {zones} octets ({} Kio), {zero_ns} ns, soit {} ps par Kio",
        zones / 1024,
        zero_ns
            .saturating_mul(1000)
            .checked_div(zones / 1024)
            .unwrap_or(0)
    );

    let mut f = Chrono {
        inner: module.fabrique(),
        mesures: Rc::new(RefCell::new(Vec::new())),
    };
    let b = bonjours(&mut f)?;
    let mut durees: Vec<u64> = b.iter().map(|m| ns(m.duree)).collect();
    let mut carburants: Vec<u64> = b.iter().map(|m| m.carburant).collect();
    let fixe = mediane(&mut durees);
    let min = durees.first().copied().unwrap_or(0);
    let p99 = centile(&durees, 99);
    let carb = mediane(&mut carburants);
    println!(
        "  2. coût fixe : {} Bonjour, durée médiane {fixe} ns (minimum {min}, 99ᵉ centile {p99}), carburant médian {carb} ; hors remise à zéro : {} ns",
        b.len(),
        fixe.saturating_sub(zero_ns)
    );

    let pc = pires_cas(&mut f)?;
    let s = soiree(&module, &mut f);
    println!(
        "  3. appels chronométrés : {} des pires cas, {} de la soirée",
        pc.len(),
        s.len()
    );
    par_sorte(&pc);
    par_sorte(&s);
    decrire_par_unite("pires cas", &pc, fixe);
    decrire_par_unite("soirée", &s, fixe);
    let mut tout = pc;
    tout.extend(s);
    decrire_par_unite("ensemble", &tout, fixe);
    Ok(())
}

fn main() -> ExitCode {
    let chemins: Vec<String> = std::env::args().skip(1).collect();
    if chemins.is_empty() {
        eprintln!("usage : etalonner <module.wasm>…");
        return ExitCode::from(64);
    }
    let moteur = match Moteur::new(ReglagesMoteur::PHASE_3) {
        Ok(m) => m,
        Err(p) => {
            eprintln!("moteur : {p}");
            return ExitCode::from(70);
        }
    };
    let mut code = ExitCode::SUCCESS;
    println!("taille des tampons de remise à zéro, pour comparaison :");
    for kio in [64u64, 256, 1024, 4096, 16_384, 32_768] {
        let d = remise_a_zero(kio.saturating_mul(1024));
        println!(
            "  {kio} Kio : {d} ns, {} ps par Kio",
            d.saturating_mul(1000).checked_div(kio).unwrap_or(0)
        );
    }
    for c in &chemins {
        if let Err(e) = etalonner(&moteur, c) {
            eprintln!("{c} : {e}");
            code = ExitCode::from(1);
        }
    }
    code
}
