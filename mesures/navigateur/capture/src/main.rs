//! La capture du banc de mesure dans les navigateurs (`mesures/navigateur/README.md`).
//!
//! ```text
//! capture-navigateur <sortie> <nom>=<module.wasm>…
//! ```
//!
//! Pour chaque module livré (compilé au profil `module`) :
//!
//! 1. **Le module réécrit**, exactement comme `moteur-wasmtime` le charge
//!    ([`Moteur::charger`] avec [`ReglagesMoteur::PHASE_1`], puis
//!    [`ModuleCharge::reecrit`]) : `<sortie>/<nom>/reecrit.wasm`, avec ce qu'il faut
//!    pour l'appeler comme le moteur (zones, pile, `soiree_memoire_max`) dans
//!    `meta.json`. La durée de la réécriture seule et de la compilation Cranelift
//!    seule y sont notées (mesure 4).
//! 2. **La trace** : chaque appel fait au module, avec ses octets, son budget, le
//!    carburant qu'il a brûlé et une empreinte de son retour, et chaque instantané,
//!    restauration, instance neuve ou lâchée, dans l'ordre :
//!    - l'accueil d'une instance nue (`Bonjour`, `Decrire`), puis deux mille
//!      `Bonjour` : le coût fixe d'un appel ;
//!    - chaque pire cas (`PireCas`), puis sept fois la vue de l'écran, l'état et les
//!      invariants : les appels les plus lourds que le jeu sache construire ;
//!    - une soirée du plan court (graine `5e…5e`), rejouée entrée par entrée par E9
//!      et E14 sur les instances enregistrées, passées à l'épreuve à la place de la
//!      copie native, comme le fait `epreuve/examples/etalonner.rs`.
//!
//!    `trace.bin`, au format décrit par [`ecrire_trace`].
//! 3. **Le rejeu dans wasmtime** : la trace rejouée [`PASSES`] fois sur des
//!    instances neuves du même module chargé, chaque appel chronométré autour de
//!    [`Instance::appeler`] (le fil de l'instance, la remise à zéro des zones, l'appel,
//!    la copie du retour : ce que la partie paie), son carburant et son retour
//!    comparés à ceux de l'enregistrement. `wasmtime.json`.
//!
//! Les pages du banc (`../page/banc.js`) rejouent la même trace, dans le même ordre,
//! avec le même protocole d'appel, dans chaque navigateur : c'est ce qui permet de
//! comparer appel par appel.
//!
//! Les durées ne valent que pour la machine qui lance la capture : c'est à qui la
//! lance d'écrire la date et la machine à côté des chiffres.

#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, Instant};

use epreuve::{IdControle, Plan};
use jeu_abi::{Appel, RegardFil, Retour, SorteAppel, decoder, encoder};
use moteur_wasmtime::{
    FabriqueWasm, InstanceWasm, InstantaneWasm, ModuleCharge, Moteur, ReglagesMoteur,
};
use partie::{Echec, Fabrique, Instance, PanneOutil};
use serde_json::json;

/// Les rejeux de la trace dans wasmtime : le premier compte la mise en route, le
/// second est celui qu'on compare.
const PASSES: usize = 2;
/// Les `Bonjour` de la mesure du coût fixe, comme `etalonner`.
const BONJOURS: usize = 2000;
/// Les répétitions de chaque lecture d'un pire cas, comme `etalonner`.
const REPETITIONS: usize = 7;
/// Le budget des appels hors de la soirée : celui d'`etalonner`.
const BUDGET_HORS_SOIREE: u64 = 1 << 28;
/// Les compilations et réécritures chronométrées par module.
const COMPILATIONS: usize = 5;

/// Le nom de chaque appel, par son indice dans la trace. Le même tableau est dans
/// `../page/trace.js` : l'ordre compte.
const NOMS: [&str; 21] = [
    "Bonjour",
    "Decrire",
    "Creer",
    "Valider",
    "Entree",
    "Regarder",
    "Etat",
    "Ressources",
    "Projeter",
    "Charger",
    "Canonique",
    "DepuisCanonique",
    "Garder",
    "Resumer",
    "Verifier",
    "Transition",
    "Bot",
    "PireCas",
    "Memoire",
    "Proprietes",
    "Hypothese",
];

/// Les sections de la trace. Le même tableau est dans `../page/trace.js`.
const SECTIONS: [&str; 4] = ["accueil", "bonjours", "pires_cas", "soiree"];
const ACCUEIL: u8 = 0;
const SECTION_BONJOURS: u8 = 1;
const PIRES_CAS: u8 = 2;
const SOIREE: u8 = 3;

/// L'issue d'un appel, telle que la trace l'écrit.
const ISSUE_OK: u8 = 0;
const ISSUE_FAUTE: u8 = 1;
const ISSUE_PANNE: u8 = 2;

const fn indice_du_nom(a: &Appel<'_>) -> u8 {
    match a {
        Appel::Bonjour => 0,
        Appel::Decrire => 1,
        Appel::Creer { .. } => 2,
        Appel::Valider { .. } => 3,
        Appel::Entree { .. } => 4,
        Appel::Regarder { .. } => 5,
        Appel::Etat { .. } => 6,
        Appel::Ressources { .. } => 7,
        Appel::Projeter { .. } => 8,
        Appel::Charger { .. } => 9,
        Appel::Canonique { .. } => 10,
        Appel::DepuisCanonique { .. } => 11,
        Appel::Garder => 12,
        Appel::Resumer => 13,
        Appel::Verifier => 14,
        Appel::Transition { .. } => 15,
        Appel::Bot { .. } => 16,
        Appel::PireCas { .. } => 17,
        Appel::Memoire => 18,
        Appel::Proprietes => 19,
        Appel::Hypothese { .. } => 20,
    }
}

/// FNV-1a sur 32 bits : une empreinte courte du retour, que la page recalcule à
/// l'identique. Elle ne sert qu'à voir qu'un rejeu rend la même chose.
fn fnv(octets: &[u8]) -> u32 {
    octets.iter().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(*b)).wrapping_mul(0x0100_0193)
    })
}

/// Un événement de la trace.
#[derive(Debug, Clone)]
enum Evenement {
    Neuve {
        inst: u32,
    },
    Appel {
        inst: u32,
        section: u8,
        nom: u8,
        sorte: SorteAppel,
        budget: u64,
        octets: Vec<u8>,
        carburant: u64,
        issue: u8,
        sortie_longueur: u32,
        sortie_fnv: u32,
    },
    Instantane {
        inst: u32,
        id: u32,
    },
    Restaurer {
        inst: u32,
        id: u32,
    },
    Oublier {
        id: u32,
    },
    Lacher {
        inst: u32,
    },
}

#[derive(Default)]
struct Journal {
    evenements: Vec<Evenement>,
    section: u8,
    instances: u32,
    instantanes: u32,
}

type Partage = Rc<RefCell<Journal>>;

fn issue_de(r: &Result<Vec<u8>, Echec>) -> (u8, u32, u32) {
    match r {
        Ok(o) => (ISSUE_OK, u32::try_from(o.len()).unwrap_or(u32::MAX), fnv(o)),
        Err(Echec::Faute(_)) => (ISSUE_FAUTE, 0, 0),
        Err(Echec::Panne(_)) => (ISSUE_PANNE, 0, 0),
    }
}

/// Une instance du module dont chaque opération s'écrit dans la trace.
struct Enregistree {
    inner: InstanceWasm,
    num: u32,
    journal: Partage,
}

/// Un instantané, avec son numéro dans la trace.
#[derive(Clone)]
struct InstantaneTrace {
    id: u32,
    inner: InstantaneWasm,
}

impl Instance for Enregistree {
    type Instantane = InstantaneTrace;

    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8], budget: u64) -> Result<Vec<u8>, Echec> {
        let nom = decoder::<Appel<'_>>(appel).map_or(u8::MAX, |a| indice_du_nom(&a));
        let r = Instance::appeler(&mut self.inner, sorte, appel, budget);
        let carburant = Instance::carburant(&self.inner).unwrap_or(0);
        let (issue, sortie_longueur, sortie_fnv) = issue_de(&r);
        let mut j = self.journal.borrow_mut();
        let section = j.section;
        j.evenements.push(Evenement::Appel {
            inst: self.num,
            section,
            nom,
            sorte,
            budget,
            octets: appel.to_vec(),
            carburant,
            issue,
            sortie_longueur,
            sortie_fnv,
        });
        r
    }

    fn carburant(&self) -> Option<u64> {
        Instance::carburant(&self.inner)
    }

    fn instantane(&mut self) -> Result<InstantaneTrace, PanneOutil> {
        let inner = Instance::instantane(&mut self.inner)?;
        let mut j = self.journal.borrow_mut();
        let id = j.instantanes;
        j.instantanes = j.instantanes.saturating_add(1);
        j.evenements.push(Evenement::Instantane { inst: self.num, id });
        Ok(InstantaneTrace { id, inner })
    }

    fn restaurer(&mut self, i: &InstantaneTrace) -> Result<(), PanneOutil> {
        self.journal.borrow_mut().evenements.push(Evenement::Restaurer {
            inst: self.num,
            id: i.id,
        });
        Instance::restaurer(&mut self.inner, &i.inner)
    }

    fn empreinte(&mut self) -> Result<[u8; 32], PanneOutil> {
        // Pas un appel du module : la page ne la refait pas, le rejeu non plus.
        Instance::empreinte(&mut self.inner)
    }
}

impl Drop for Enregistree {
    fn drop(&mut self) {
        if let Ok(mut j) = self.journal.try_borrow_mut() {
            j.evenements.push(Evenement::Lacher { inst: self.num });
        }
    }
}

/// Des instances enregistrées.
struct Enregistreuse {
    inner: FabriqueWasm,
    journal: Partage,
}

impl Fabrique for Enregistreuse {
    type I = Enregistree;

    fn neuve(&mut self) -> Result<Enregistree, PanneOutil> {
        let inner = Fabrique::neuve(&mut self.inner)?;
        let mut j = self.journal.borrow_mut();
        let num = j.instances;
        j.instances = j.instances.saturating_add(1);
        j.evenements.push(Evenement::Neuve { inst: num });
        Ok(Enregistree {
            inner,
            num,
            journal: Rc::clone(&self.journal),
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
    reason = "le banc mesure des durées murales : c'est son objet, et rien d'un jeu n'en dépend"
)]
fn maintenant() -> Instant {
    Instant::now()
}

fn ns(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
}

fn section(f: &Enregistreuse, s: u8) {
    f.journal.borrow_mut().section = s;
}

/// Une instance nue, accueillie (`Bonjour`, `Decrire`).
fn nue(f: &mut Enregistreuse) -> Result<Enregistree, String> {
    section(f, ACCUEIL);
    let mut i = Fabrique::neuve(f).map_err(|p| p.to_string())?;
    for a in [Appel::Bonjour, Appel::Decrire] {
        let _: Vec<u8> =
            Instance::appeler(&mut i, SorteAppel::Lecture, &encode(&a), BUDGET_HORS_SOIREE)
                .map_err(|e| e.to_string())?;
    }
    Ok(i)
}

/// Le coût fixe : `Bonjour` sur une instance nue, [`BONJOURS`] fois.
fn enregistrer_bonjours(f: &mut Enregistreuse) -> Result<(), String> {
    let mut i = nue(f)?;
    section(f, SECTION_BONJOURS);
    let octets = encode(&Appel::Bonjour);
    for _ in 0..BONJOURS {
        let _: Vec<u8> =
            Instance::appeler(&mut i, SorteAppel::Lecture, &octets, BUDGET_HORS_SOIREE)
                .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Chaque pire cas, puis ses lectures, [`REPETITIONS`] fois chacune. Rend le nombre
/// de pires cas.
fn enregistrer_pires_cas(f: &mut Enregistreuse) -> Result<u32, String> {
    let lectures = [
        Appel::Regarder {
            regards: vec![RegardFil::Ecran],
        },
        Appel::Etat {
            connues: Vec::new(),
        },
        Appel::Verifier,
    ];
    for indice in 0..=u32::from(u16::MAX) {
        let mut i = nue(f)?;
        section(f, PIRES_CAS);
        let pire = Instance::appeler(
            &mut i,
            SorteAppel::Ecriture,
            &encode(&Appel::PireCas { indice }),
            BUDGET_HORS_SOIREE,
        );
        let accepte = pire.as_deref().map(decoder::<Retour<'_>>);
        if !matches!(accepte, Ok(Ok(Retour::PireCas { .. }))) {
            return Ok(indice);
        }
        for a in &lectures {
            let octets = encode(a);
            for _ in 0..REPETITIONS {
                let _: Result<Vec<u8>, Echec> =
                    Instance::appeler(&mut i, SorteAppel::Lecture, &octets, BUDGET_HORS_SOIREE);
            }
        }
    }
    Ok(u32::from(u16::MAX))
}

/// La soirée du plan court, E9 et E14 seuls, sur des instances enregistrées. Rend le
/// code des contrôles.
fn enregistrer_soiree(module: &ModuleCharge, f: &mut Enregistreuse) -> u8 {
    section(f, SOIREE);
    let mut plan = Plan::court([0x5e; 32]);
    plan.controles = vec![IdControle::E9, IdControle::E14];
    let rapport = epreuve::eprouver(module, Some(f), &plan);
    rapport.code_des_controles()
}

/// Ajoute `Oublier` après le dernier usage de chaque instantané : la page n'en garde
/// pas plus qu'il n'en faut (25 Mio chacun chez Tier list).
fn ajouter_les_oublis(ev: Vec<Evenement>) -> Vec<Evenement> {
    let mut dernier: BTreeMap<u32, usize> = BTreeMap::new();
    for (k, e) in ev.iter().enumerate() {
        if let Evenement::Instantane { id, .. } | Evenement::Restaurer { id, .. } = e {
            let _: Option<usize> = dernier.insert(*id, k);
        }
    }
    let mut apres: BTreeMap<usize, Vec<u32>> = BTreeMap::new();
    for (id, k) in dernier {
        apres.entry(k).or_default().push(id);
    }
    let mut sortie = Vec::with_capacity(ev.len().saturating_add(apres.len()));
    for (k, e) in ev.into_iter().enumerate() {
        sortie.push(e);
        if let Some(ids) = apres.get_mut(&k) {
            ids.sort_unstable();
            sortie.extend(ids.iter().map(|id| Evenement::Oublier { id: *id }));
        }
    }
    sortie
}

/// Le format de la trace, petit-boutiste, que `../page/trace.js` lit :
///
/// ```text
/// "SOIRTRC1"
/// puis des événements, chacun introduit par un octet :
///   1 Neuve       u32 instance
///   2 Appel       u32 instance, u8 section, u8 nom, i32 sorte (code de soiree_boite),
///                 u64 budget, u32 longueur, octets de l'appel,
///                 u64 carburant brûlé, u8 issue (0 rendu, 1 faute, 2 panne),
///                 u32 longueur du retour, u32 FNV-1a du retour
///   3 Instantane  u32 instance, u32 numéro
///   4 Restaurer   u32 instance, u32 numéro
///   5 Oublier     u32 numéro
///   6 Lacher      u32 instance
///   0 Fin
/// ```
fn ecrire_trace(ev: &[Evenement]) -> Vec<u8> {
    let mut o = Vec::new();
    o.extend_from_slice(b"SOIRTRC1");
    for e in ev {
        match e {
            Evenement::Neuve { inst } => {
                o.push(1);
                o.extend_from_slice(&inst.to_le_bytes());
            }
            Evenement::Appel {
                inst,
                section,
                nom,
                sorte,
                budget,
                octets,
                carburant,
                issue,
                sortie_longueur,
                sortie_fnv,
            } => {
                o.push(2);
                o.extend_from_slice(&inst.to_le_bytes());
                o.push(*section);
                o.push(*nom);
                o.extend_from_slice(&sorte.code().to_le_bytes());
                o.extend_from_slice(&budget.to_le_bytes());
                o.extend_from_slice(&u32::try_from(octets.len()).unwrap_or(0).to_le_bytes());
                o.extend_from_slice(octets);
                o.extend_from_slice(&carburant.to_le_bytes());
                o.push(*issue);
                o.extend_from_slice(&sortie_longueur.to_le_bytes());
                o.extend_from_slice(&sortie_fnv.to_le_bytes());
            }
            Evenement::Instantane { inst, id } => {
                o.push(3);
                o.extend_from_slice(&inst.to_le_bytes());
                o.extend_from_slice(&id.to_le_bytes());
            }
            Evenement::Restaurer { inst, id } => {
                o.push(4);
                o.extend_from_slice(&inst.to_le_bytes());
                o.extend_from_slice(&id.to_le_bytes());
            }
            Evenement::Oublier { id } => {
                o.push(5);
                o.extend_from_slice(&id.to_le_bytes());
            }
            Evenement::Lacher { inst } => {
                o.push(6);
                o.extend_from_slice(&inst.to_le_bytes());
            }
        }
    }
    o.push(0);
    o
}

/// Ce qu'un rejeu a constaté.
#[derive(Default)]
struct Rejeu {
    /// La durée de chaque appel, dans l'ordre de la trace, en nanosecondes.
    durees_ns: Vec<u64>,
    /// Les appels dont le carburant, l'issue ou le retour diffèrent de
    /// l'enregistrement.
    ecarts: u64,
    /// Le premier écart, décrit.
    premier_ecart: Option<String>,
    /// Les pannes du rejeu lui-même (instance, instantané).
    pannes: Vec<String>,
}

/// Rejoue la trace sur des instances neuves du module, chaque appel chronométré.
fn rejouer(module: &ModuleCharge, ev: &[Evenement]) -> Rejeu {
    let mut f = module.fabrique();
    let mut instances: BTreeMap<u32, InstanceWasm> = BTreeMap::new();
    let mut instantanes: BTreeMap<u32, InstantaneWasm> = BTreeMap::new();
    let mut r = Rejeu::default();
    for (k, e) in ev.iter().enumerate() {
        match e {
            Evenement::Neuve { inst } => match Fabrique::neuve(&mut f) {
                Ok(i) => {
                    let _: Option<InstanceWasm> = instances.insert(*inst, i);
                }
                Err(p) => r.pannes.push(format!("événement {k} : neuve : {p}")),
            },
            Evenement::Appel {
                inst,
                sorte,
                budget,
                octets,
                carburant,
                issue,
                sortie_longueur,
                sortie_fnv,
                ..
            } => {
                let Some(i) = instances.get_mut(inst) else {
                    r.pannes.push(format!("événement {k} : instance {inst} absente"));
                    r.durees_ns.push(0);
                    continue;
                };
                let debut = maintenant();
                let res = Instance::appeler(i, *sorte, octets, *budget);
                let d = ns(debut.elapsed());
                r.durees_ns.push(d);
                let brule = Instance::carburant(i).unwrap_or(0);
                let (iss, lon, emp) = issue_de(&res);
                let meme_carburant = *issue == ISSUE_PANNE || brule == *carburant;
                if !meme_carburant || iss != *issue || lon != *sortie_longueur || emp != *sortie_fnv {
                    r.ecarts = r.ecarts.saturating_add(1);
                    if r.premier_ecart.is_none() {
                        r.premier_ecart = Some(format!(
                            "événement {k} : carburant {brule} au lieu de {carburant}, issue {iss} au lieu de {issue}, retour {lon} octets ({emp:08x}) au lieu de {sortie_longueur} ({sortie_fnv:08x})"
                        ));
                    }
                }
            }
            Evenement::Instantane { inst, id } => {
                if let Some(i) = instances.get_mut(inst) {
                    match Instance::instantane(i) {
                        Ok(s) => {
                            let _: Option<InstantaneWasm> = instantanes.insert(*id, s);
                        }
                        Err(p) => r.pannes.push(format!("événement {k} : instantané : {p}")),
                    }
                }
            }
            Evenement::Restaurer { inst, id } => {
                if let (Some(i), Some(s)) = (instances.get_mut(inst), instantanes.get(id))
                    && let Err(p) = Instance::restaurer(i, s)
                {
                    r.pannes.push(format!("événement {k} : restaurer : {p}"));
                }
            }
            Evenement::Oublier { id } => {
                let _: Option<InstantaneWasm> = instantanes.remove(id);
            }
            Evenement::Lacher { inst } => {
                let _: Option<InstanceWasm> = instances.remove(inst);
            }
        }
    }
    r
}

/// La durée d'un `fill(0)` de `octets` octets déjà écrits : la médiane de quinze, en
/// nanosecondes, comme `etalonner`.
fn remise_a_zero(octets: u64) -> u64 {
    let n = usize::try_from(octets).unwrap_or(0);
    let mut tampon = vec![1u8; n];
    let mut durees: Vec<u64> = (0..15u8)
        .map(|k| {
            tampon.fill(k.saturating_add(1));
            let debut = maintenant();
            tampon.fill(0);
            let d = ns(debut.elapsed());
            std::hint::black_box(&tampon);
            d
        })
        .collect();
    durees.sort_unstable();
    durees.get(durees.len() / 2).copied().unwrap_or(0)
}

/// Les réglages de wasmtime de `moteur-wasmtime` (`Moteur::new`), recopiés : la
/// compilation seule se chronomètre avec eux.
fn engine_du_moteur() -> Result<wasmtime::Engine, String> {
    let r = ReglagesMoteur::PHASE_1;
    let pile = usize::try_from(r.reecriture.profondeur_max)
        .unwrap_or(usize::MAX)
        .saturating_add(16)
        .saturating_mul(moteur_wasmtime::PILE_PAR_APPEL)
        .max(512 * 1024);
    let mut c = wasmtime::Config::new();
    c.strategy(wasmtime::Strategy::Cranelift)
        .wasm_simd(false)
        .wasm_relaxed_simd(false)
        .wasm_multi_memory(false)
        .wasm_memory64(false)
        .wasm_tail_call(false)
        .wasm_gc(false)
        .wasm_stack_switching(false)
        .wasm_wide_arithmetic(false)
        .wasm_custom_page_sizes(false)
        .wasm_extended_const(false)
        .wasm_bulk_memory(true)
        .wasm_multi_value(true)
        .consume_fuel(false)
        .cranelift_nan_canonicalization(false)
        .epoch_interruption(true)
        .max_wasm_stack(pile)
        .async_stack_size(pile);
    wasmtime::Engine::new(&c).map_err(|e| format!("wasmtime : {e}"))
}

fn hex(o: &[u8]) -> String {
    o.iter().fold(String::new(), |mut s, b| {
        let _: Result<(), std::fmt::Error> = write!(s, "{b:02x}");
        s
    })
}

fn ms(d: u64) -> f64 {
    // Des durées de moins d'une heure : la conversion ne perd rien qui compte ici.
    #[expect(
        clippy::cast_precision_loss,
        reason = "une durée en nanosecondes de moins de 2^53 se convertit exactement"
    )]
    let x = d as f64;
    x / 1e6
}

fn ecrire(chemin: &Path, octets: &[u8]) -> Result<(), String> {
    std::fs::write(chemin, octets).map_err(|e| format!("{} : {e}", chemin.display()))
}

#[expect(
    clippy::too_many_lines,
    reason = "la capture d'un module se lit d'un trait, dans l'ordre de la documentation"
)]
fn capturer(moteur: &Moteur, engine: &wasmtime::Engine, sortie: &Path, nom: &str, chemin: &Path) -> Result<(), String> {
    let original = std::fs::read(chemin).map_err(|e| format!("{} : {e}", chemin.display()))?;
    let dossier = sortie.join(nom);
    std::fs::create_dir_all(&dossier).map_err(|e| format!("{} : {e}", dossier.display()))?;
    println!("{nom} : {}", chemin.display());

    // 1. La réécriture seule, puis la compilation Cranelift seule, puis le chargement.
    let mut reecrire_ns = Vec::new();
    let mut reecrit = None;
    for _ in 0..COMPILATIONS {
        let debut = maintenant();
        let r = reecriture::reecrire(&original, &ReglagesMoteur::PHASE_1.reecriture)
            .map_err(|r| r.to_string())?;
        reecrire_ns.push(ns(debut.elapsed()));
        reecrit = Some(r);
    }
    let reecrit = reecrit.ok_or("aucune réécriture")?;
    let mut compiler_ns = Vec::new();
    for _ in 0..COMPILATIONS {
        let debut = maintenant();
        let m = wasmtime::Module::new(engine, &reecrit.octets).map_err(|e| e.to_string())?;
        compiler_ns.push(ns(debut.elapsed()));
        drop(m);
    }
    let debut = maintenant();
    let module = moteur.charger(&original).map_err(|r| r.to_string())?;
    let charger_ns = ns(debut.elapsed());
    if module.reecrit().octets != reecrit.octets {
        return Err(String::from(
            "la réécriture du moteur diffère de la réécriture seule : la page ne chargerait pas le même module",
        ));
    }
    ecrire(&dossier.join("reecrit.wasm"), &module.reecrit().octets)?;
    let b = module.bonjour();
    let pile = module.reecrit().pile_initiale;
    let zones = [(0, pile), b.boite, b.brouillon, b.panique];
    let somme: u64 = zones.iter().map(|(_, n)| u64::from(*n)).sum();
    if somme != module.octets_remis_a_zero() {
        return Err(format!(
            "zones : {somme} octets comptés ici, {} par le moteur",
            module.octets_remis_a_zero()
        ));
    }
    let zero_ns = remise_a_zero(somme);
    println!(
        "  réécrit : {} octets ; zones remises à zéro : {somme} octets, fill(0) {zero_ns} ns",
        module.reecrit().octets.len()
    );

    // 2. La trace.
    let journal: Partage = Rc::new(RefCell::new(Journal::default()));
    let mut f = Enregistreuse {
        inner: module.fabrique(),
        journal: Rc::clone(&journal),
    };
    enregistrer_bonjours(&mut f)?;
    let pires = enregistrer_pires_cas(&mut f)?;
    let debut = maintenant();
    let code = enregistrer_soiree(&module, &mut f);
    println!(
        "  soirée du plan court (E9, E14) : code des contrôles {code}, {} s",
        debut.elapsed().as_secs()
    );
    drop(f);
    let evenements = ajouter_les_oublis(std::mem::take(&mut journal.borrow_mut().evenements));
    let appels = evenements
        .iter()
        .filter(|e| matches!(e, Evenement::Appel { .. }))
        .count();
    let trace = ecrire_trace(&evenements);
    ecrire(&dossier.join("trace.bin"), &trace)?;
    println!(
        "  trace : {} événements, {appels} appels, {pires} pires cas, {} octets",
        evenements.len(),
        trace.len()
    );

    // 3. Le rejeu dans wasmtime.
    let mut passes = Vec::new();
    for p in 0..PASSES {
        let debut = maintenant();
        let r = rejouer(&module, &evenements);
        println!(
            "  rejeu {} : {} ms, {} écarts, {} pannes{}",
            p.saturating_add(1),
            debut.elapsed().as_millis(),
            r.ecarts,
            r.pannes.len(),
            r.premier_ecart
                .as_deref()
                .map_or(String::new(), |e| format!(" ; premier : {e}"))
        );
        passes.push(json!({
            "durees_ns": r.durees_ns,
            "ecarts": r.ecarts,
            "premier_ecart": r.premier_ecart,
            "pannes": r.pannes,
        }));
    }
    let meta = json!({
        "nom": nom,
        "module_livre": chemin.display().to_string(),
        "octets_livres": original.len(),
        "octets_reecrits": module.reecrit().octets.len(),
        "empreinte_livree": hex(&module.reecrit().empreinte_originale),
        "empreinte_reecrite": hex(&module.reecrit().empreinte),
        "reecriture_version": reecriture::VERSION,
        "pile_initiale": pile,
        "remises_a_zero": zones.iter().map(|(a, n)| [*a, *n]).collect::<Vec<_>>(),
        "boite": [b.boite.0, b.boite.1],
        "octets_remis_a_zero": somme,
        "memoire_max": module.memoire_max(),
        "memoire_pages": [module.constat().memoire.0, module.constat().memoire.1],
        "globales": {
            "carburant": reecriture::CARBURANT,
            "profondeur": reecriture::PROFONDEUR,
            "memoire_max": reecriture::MEMOIRE_MAX,
            "croissance_refusee": reecriture::CROISSANCE_REFUSEE,
            "croissance_bornee": reecriture::CROISSANCE_BORNEE,
        },
        "noms": NOMS,
        "sections": SECTIONS,
        "appels": appels,
        "pires_cas": pires,
        "code_des_controles_soiree": code,
        "reecrire_ms": reecrire_ns.iter().map(|d| ms(*d)).collect::<Vec<_>>(),
        "compiler_wasmtime_ms": compiler_ns.iter().map(|d| ms(*d)).collect::<Vec<_>>(),
        "charger_ms": ms(charger_ns),
        "remise_a_zero_ns": zero_ns,
    });
    ecrire(
        &dossier.join("meta.json"),
        serde_json::to_string_pretty(&meta)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )?;
    let wasmtime_json = json!({
        "moteur": "wasmtime 50.0.1 (Cranelift), moteur-wasmtime, ReglagesMoteur::PHASE_1",
        "passes": passes,
    });
    ecrire(
        &dossier.join("wasmtime.json"),
        serde_json::to_string(&wasmtime_json)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )?;
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((sortie, modules)) = args.split_first() else {
        eprintln!("usage : capture-navigateur <sortie> <nom>=<module.wasm>…");
        return ExitCode::from(64);
    };
    let moteur = match Moteur::new(ReglagesMoteur::PHASE_1) {
        Ok(m) => m,
        Err(p) => {
            eprintln!("moteur : {p}");
            return ExitCode::from(2);
        }
    };
    let engine = match engine_du_moteur() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let sortie = PathBuf::from(sortie);
    let mut code = ExitCode::SUCCESS;
    for m in modules {
        let Some((nom, chemin)) = m.split_once('=') else {
            eprintln!("{m} : attendu <nom>=<module.wasm>");
            return ExitCode::from(64);
        };
        if let Err(e) = capturer(&moteur, &engine, &sortie, nom, Path::new(chemin)) {
            eprintln!("{nom} : {e}");
            code = ExitCode::from(1);
        }
    }
    code
}
