//! L'étalonnage dans un vrai navigateur (PHASE-3, § 8.3 et § 8.6, lot 3c) : M1, M10 et
//! la première moitié de M9.
//!
//! Une bibliothèque WebAssembly : `wasm-bindgen` la relie au Worker de la page de
//! `soiree-banc etalonner` (`crates/banc/pages/etalonner.html`), qui appelle l'une des
//! trois fonctions ci-dessous et envoie ce qu'elle rend à `POST /banc/rapport`. Tout
//! passe par le protocole du commun (`moteur-commun`) et la machine du navigateur
//! (`moteur-navigateur`) : rien n'en est recopié, comme le faisait le harnais de mesure
//! (`mesures/navigateur/page/banc-coeur.js`, retiré au lot 3c).
//!
//! - [`etalonner`] (M1) : le module livré chargé comme l'autorité le chargera
//!   (`moteur_navigateur::charger`, sous `reecriture::Reglages::PHASE_3`), la mise en
//!   route, la réécriture et la compilation chronométrées ; la remise à zéro des zones ;
//!   deux mille `Bonjour` d'un bloc ; puis la trace de la capture
//!   (`moteur_commun::trace`) rejouée appel par appel, chaque appel, instance neuve,
//!   instantané et restauration chronométrés, et le carburant, l'issue et le retour de
//!   chaque appel comparés à ceux de wasmtime.
//! - [`boucle_vide`] (M10) : le module de `Bonjour` qui boucle chargé jusqu'à sa faute
//!   `Budget`, et un témoin qui piège dès l'entrée ; puis la même boucle sur un plus
//!   grand budget, par la machine seule.
//! - [`profondeur_tenue`] (M9) : des récursions sans fin, à cadres de plus en plus
//!   épais, réécrites sans plafond de profondeur utile : la pile native du fil qui
//!   appelle (Worker ou page) cède d'abord, et `soiree_profondeur` dit combien d'étages
//!   elle a tenu ; puis les mêmes récursions chargées sous `PHASE_3` par
//!   `moteur_navigateur::charger`, sur le même fil : chacune doit être la faute
//!   `Profondeur` du jeu avant que la pile native ne cède.
//!
//! Les durées sont celles de `performance.now()`, dont le navigateur choisit la
//! granularité (mesurée ici : `granularite_ms`) ; la page est isolée (COOP et COEP)
//! pour avoir la plus fine.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::rc::Rc;

use js_sys::{Array, Function, Object, Reflect, Uint8Array};
use moteur_commun::trace::{self, Evenement};
use moteur_commun::{BUDGET_DE_CHARGEMENT, Bits, Instantane, Machine, Piege, RefusModule};
use moteur_navigateur::{InstanceNavigateur, MachineNavigateur, ModuleNavigateur, charger};
use partie::{Fabrique, Faute, Instance};
use reecriture::{CARBURANT, MEMOIRE_MAX, PROFONDEUR, Reglages};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};

/// Les réglages du module chargé : ceux de l'autorité.
const REGLAGES: Reglages = Reglages::PHASE_3;
/// Les `Bonjour` d'un bloc, comme le harnais et `epreuve/examples/etalonner.rs`.
const BONJOURS: u32 = 2000;
/// Le budget des `Bonjour` du bloc : celui de la capture hors de la soirée.
const BUDGET_DU_BLOC: u64 = 1 << 28;
/// Les réécritures, compilations et remises à zéro chronométrées.
const REPETITIONS: u32 = 5;
/// Les remises à zéro chronométrées, comme `etalonner` : médiane de quinze.
const REMISES: u8 = 15;
/// Les mises en route chronométrées.
const MISES_EN_ROUTE: u32 = 3;

// L'horloge.

/// `performance.now()` du fil qui appelle, Worker ou page.
struct Horloge {
    performance: JsValue,
    now: Function,
}

impl Horloge {
    fn new() -> Result<Horloge, String> {
        let performance = Reflect::get(&js_sys::global(), &JsValue::from_str("performance"))
            .map_err(|_| String::from("performance absent"))?;
        let now = Reflect::get(&performance, &JsValue::from_str("now"))
            .ok()
            .and_then(|f| f.dyn_into::<Function>().ok())
            .ok_or_else(|| String::from("performance.now absent"))?;
        Ok(Horloge { performance, now })
    }

    /// Maintenant, en millisecondes.
    fn ms(&self) -> f64 {
        self.now
            .call0(&self.performance)
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
    }

    /// La plus petite différence non nulle entre deux lectures, sur deux cents.
    fn granularite(&self) -> f64 {
        let mut min = f64::INFINITY;
        for _ in 0..200 {
            let a = self.ms();
            let mut b = a;
            for _ in 0..10_000_000u32 {
                b = self.ms();
                if b > a {
                    break;
                }
            }
            let d = b - a;
            if d > 0.0 && d < min {
                min = d;
            }
        }
        min
    }
}

// Ce que l'on rend à la page.

fn objet(champs: &[(&str, JsValue)]) -> JsValue {
    let o = Object::new();
    for (cle, v) in champs {
        let _: Result<bool, JsValue> = Reflect::set(&o, &JsValue::from_str(cle), v);
    }
    o.into()
}

fn erreur(quoi: &str) -> JsValue {
    objet(&[("erreur", JsValue::from_str(quoi))])
}

fn nombres(v: &[f64]) -> JsValue {
    let a = Array::new_with_length(u32::try_from(v.len()).unwrap_or(u32::MAX));
    for (k, x) in v.iter().enumerate() {
        a.set(u32::try_from(k).unwrap_or(u32::MAX), JsValue::from_f64(*x));
    }
    a.into()
}

fn entier(n: u64) -> JsValue {
    // Un nombre du navigateur est exact jusqu'à 2⁵³ : au-delà, le texte.
    match u32::try_from(n) {
        Ok(x) => JsValue::from(x),
        Err(_) => JsValue::from_str(&n.to_string()),
    }
}

fn mediane(v: &[f64]) -> f64 {
    let mut t = v.to_vec();
    t.sort_by(f64::total_cmp);
    t.get(t.len() / 2).copied().unwrap_or(f64::NAN)
}

fn hex(o: &[u8]) -> String {
    o.iter().fold(String::new(), |mut s, b| {
        let _: std::fmt::Result = write!(s, "{b:02x}");
        s
    })
}

/// Les octets d'un module suivis d'une section personnalisée unique : le navigateur ne
/// peut pas reprendre une compilation faite sur d'autres octets. Une section
/// personnalisée ne change rien au code.
fn avec_nonce(octets: &[u8], k: u32, horloge: &Horloge) -> Vec<u8> {
    let nom = b"banc-nonce";
    let nonce = format!("{k}-{}", horloge.ms());
    let contenu = nom
        .len()
        .saturating_add(1)
        .saturating_add(nonce.len())
        .min(127);
    let mut o = octets.to_vec();
    o.push(0);
    o.push(u8::try_from(contenu).unwrap_or(127));
    o.push(u8::try_from(nom.len()).unwrap_or(0));
    o.extend_from_slice(nom);
    o.extend(
        nonce
            .bytes()
            .take(contenu.saturating_sub(nom.len()).saturating_sub(1)),
    );
    o
}

// M1 : la trace rejouée.

/// Ce qu'un rejeu de la trace a constaté.
#[derive(Default)]
struct Rejeu {
    /// La durée de chaque appel, dans l'ordre de la trace, en µs.
    appels_us: Vec<f64>,
    /// La durée de chaque instance neuve, instantané et restauration, en µs.
    neuves_us: Vec<f64>,
    instantanes_us: Vec<f64>,
    restaurations_us: Vec<f64>,
    /// Les octets de chaque instantané pris.
    instantanes_octets: Vec<f64>,
    ecarts: u64,
    premier_ecart: Option<String>,
    pannes: Vec<String>,
    total_ms: f64,
}

fn rejouer_la_trace(
    module: &moteur_navigateur::ModuleCharge,
    evenements: &[Evenement],
    horloge: &Horloge,
) -> Rejeu {
    let mut fabrique = module.fabrique();
    let mut instances: BTreeMap<u32, InstanceNavigateur> = BTreeMap::new();
    let mut instantanes: BTreeMap<u32, Instantane> = BTreeMap::new();
    let mut rejeu = Rejeu::default();
    let debut = horloge.ms();
    for (k, e) in evenements.iter().enumerate() {
        match e {
            Evenement::Neuve { inst } => {
                let debut_op = horloge.ms();
                let i = Fabrique::neuve(&mut fabrique);
                rejeu.neuves_us.push((horloge.ms() - debut_op) * 1000.0);
                match i {
                    Ok(i) => {
                        let _: Option<InstanceNavigateur> = instances.insert(*inst, i);
                    }
                    Err(p) => rejeu.pannes.push(format!("événement {k} : neuve : {p}")),
                }
            }
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
                    rejeu
                        .pannes
                        .push(format!("événement {k} : instance {inst} absente"));
                    rejeu.appels_us.push(0.0);
                    continue;
                };
                let debut_op = horloge.ms();
                let res = Instance::appeler(i, *sorte, octets, *budget);
                rejeu.appels_us.push((horloge.ms() - debut_op) * 1000.0);
                let brule = Instance::carburant(i).unwrap_or(0);
                let (iss, lon, emp) = trace::issue_de(&res);
                let meme_carburant = *issue == trace::ISSUE_PANNE || brule == *carburant;
                if !meme_carburant || iss != *issue || lon != *sortie_longueur || emp != *sortie_fnv
                {
                    rejeu.ecarts = rejeu.ecarts.saturating_add(1);
                    if rejeu.premier_ecart.is_none() {
                        rejeu.premier_ecart = Some(format!(
                            "événement {k} : carburant {brule} au lieu de {carburant}, issue {iss} au lieu de {issue}, retour {lon} octets ({emp:08x}) au lieu de {sortie_longueur} ({sortie_fnv:08x})"
                        ));
                    }
                }
            }
            Evenement::Instantane { inst, id } => {
                if let Some(i) = instances.get_mut(inst) {
                    let debut_op = horloge.ms();
                    let s = Instance::instantane(i);
                    rejeu
                        .instantanes_us
                        .push((horloge.ms() - debut_op) * 1000.0);
                    match s {
                        Ok(s) => {
                            let n = u32::try_from(s.octets().len()).unwrap_or(u32::MAX);
                            rejeu.instantanes_octets.push(f64::from(n));
                            let _: Option<Instantane> = instantanes.insert(*id, s);
                        }
                        Err(p) => rejeu
                            .pannes
                            .push(format!("événement {k} : instantané : {p}")),
                    }
                }
            }
            Evenement::Restaurer { inst, id } => {
                if let (Some(i), Some(s)) = (instances.get_mut(inst), instantanes.get(id)) {
                    let debut_op = horloge.ms();
                    let x = Instance::restaurer(i, s);
                    rejeu
                        .restaurations_us
                        .push((horloge.ms() - debut_op) * 1000.0);
                    if let Err(p) = x {
                        rejeu
                            .pannes
                            .push(format!("événement {k} : restaurer : {p}"));
                    }
                }
            }
            Evenement::Oublier { id } => {
                let _: Option<Instantane> = instantanes.remove(id);
            }
            Evenement::Lacher { inst } => {
                let _: Option<InstanceNavigateur> = instances.remove(inst);
            }
        }
    }
    rejeu.total_ms = horloge.ms() - debut;
    rejeu
}

fn rejeu_js(r: &Rejeu) -> JsValue {
    let pannes = Array::new();
    for p in &r.pannes {
        pannes.push(&JsValue::from_str(p));
    }
    objet(&[
        ("total_ms", JsValue::from_f64(r.total_ms)),
        ("appels_us", nombres(&r.appels_us)),
        ("neuves_us", nombres(&r.neuves_us)),
        ("instantanes_us", nombres(&r.instantanes_us)),
        ("restaurations_us", nombres(&r.restaurations_us)),
        ("instantanes_octets", nombres(&r.instantanes_octets)),
        ("ecarts", entier(r.ecarts)),
        (
            "premier_ecart",
            r.premier_ecart
                .as_deref()
                .map_or(JsValue::NULL, JsValue::from_str),
        ),
        ("pannes", pannes.into()),
    ])
}

/// La remise à zéro des zones : par la machine, dans la mémoire d'une instance, comme le
/// protocole la fait avant chaque appel ; puis `fill(0)` seul d'un tampon de leur
/// taille, comme le harnais. Médianes de quinze, en ms.
fn remise_a_zero(
    module: &moteur_navigateur::ModuleCharge,
    horloge: &Horloge,
) -> Result<(u64, f64, f64), String> {
    let zones = module.carte().remises_a_zero();
    let octets = module.carte().octets_remis_a_zero();
    let mut i = module.fabrique().neuve().map_err(|p| p.to_string())?;
    let m = i.machine();
    let mut dans_la_memoire = Vec::new();
    for k in 0..REMISES {
        for (a, n) in zones {
            let sale = vec![k.saturating_add(1); usize::try_from(n).unwrap_or(0)];
            m.ecrire(a, &sale).map_err(|p| p.to_string())?;
        }
        let t = horloge.ms();
        for (a, n) in zones {
            m.remplir_de_zeros(a, n).map_err(|p| p.to_string())?;
        }
        dans_la_memoire.push(horloge.ms() - t);
    }
    let tampon = Uint8Array::new_with_length(u32::try_from(octets).unwrap_or(u32::MAX));
    let mut seul = Vec::new();
    for k in 0..REMISES {
        let _: Uint8Array = tampon.fill(k.saturating_add(1), 0, tampon.length());
        let t = horloge.ms();
        let _: Uint8Array = tampon.fill(0, 0, tampon.length());
        seul.push(horloge.ms() - t);
    }
    Ok((octets, mediane(&dans_la_memoire), mediane(&seul)))
}

/// Deux mille `Bonjour` d'un bloc, par le protocole : une mise en route, puis trois
/// blocs ; la durée moyenne d'un appel de chaque bloc, en ms.
fn bonjours(
    module: &moteur_navigateur::ModuleCharge,
    bonjour: &[u8],
    horloge: &Horloge,
) -> Result<Vec<f64>, String> {
    let mut i = module.fabrique().neuve().map_err(|p| p.to_string())?;
    let bloc = |i: &mut InstanceNavigateur| -> Result<f64, String> {
        let t = horloge.ms();
        for _ in 0..BONJOURS {
            let _: Vec<u8> =
                Instance::appeler(i, jeu_abi::SorteAppel::Lecture, bonjour, BUDGET_DU_BLOC)
                    .map_err(|e| e.to_string())?;
        }
        Ok((horloge.ms() - t) / f64::from(BONJOURS))
    };
    let _: f64 = bloc(&mut i)?;
    let mut v = Vec::new();
    for _ in 0..3 {
        v.push(bloc(&mut i)?);
    }
    Ok(v)
}

fn mesurer(livre: &[u8], octets_trace: &[u8], passes: u32) -> Result<JsValue, String> {
    let horloge = Horloge::new()?;
    let evenements = trace::lire(octets_trace)?;
    let granularite = horloge.granularite();
    // La réécriture seule, puis la compilation seule (sur des octets rendus uniques).
    let mut reecrire_ms = Vec::new();
    let mut reecrit = None;
    for _ in 0..REPETITIONS {
        let t = horloge.ms();
        let r = reecriture::reecrire(livre, &REGLAGES).map_err(|r| r.to_string())?;
        reecrire_ms.push(horloge.ms() - t);
        reecrit = Some(r);
    }
    let reecrit = reecrit.ok_or("aucune réécriture")?;
    let mut compiler_ms = Vec::new();
    for k in 0..=REPETITIONS {
        let o = if k == 0 {
            reecrit.octets.clone()
        } else {
            avec_nonce(&reecrit.octets, k, &horloge)
        };
        let t = horloge.ms();
        let m = ModuleNavigateur::compiler(&o).map_err(|p| p.to_string())?;
        compiler_ms.push(horloge.ms() - t);
        drop(m);
    }
    // La mise en route : réécrire, compiler, instancier, `Bonjour`, `Decrire`.
    let mut mise_en_route_ms = Vec::new();
    let mut charge = None;
    for _ in 0..MISES_EN_ROUTE {
        let t = horloge.ms();
        let m = charger(livre, &REGLAGES).map_err(|r| format!("refusé : {r}"))?;
        mise_en_route_ms.push(horloge.ms() - t);
        charge = Some(m);
    }
    let module = charge.ok_or("aucun chargement")?;
    if module.reecrit().octets != reecrit.octets {
        return Err(String::from(
            "la réécriture du chargement diffère de la réécriture seule",
        ));
    }
    let (octets_zones, zero_memoire_ms, zero_seul_ms) = remise_a_zero(&module, &horloge)?;
    let bonjour = evenements
        .iter()
        .find_map(|e| match e {
            Evenement::Appel { nom: 0, octets, .. } => Some(octets.clone()),
            Evenement::Appel { .. }
            | Evenement::Neuve { .. }
            | Evenement::Instantane { .. }
            | Evenement::Restaurer { .. }
            | Evenement::Oublier { .. }
            | Evenement::Lacher { .. } => None,
        })
        .ok_or("la trace n'a pas de Bonjour")?;
    let blocs = bonjours(&module, &bonjour, &horloge)?;
    let liste = Array::new();
    for _ in 0..passes {
        liste.push(&rejeu_js(&rejouer_la_trace(&module, &evenements, &horloge)));
    }
    let (b, d) = module.carburant_du_chargement();
    Ok(objet(&[
        ("granularite_ms", JsValue::from_f64(granularite)),
        (
            "empreinte_reecrite",
            JsValue::from_str(&hex(&module.reecrit().empreinte)),
        ),
        (
            "octets_reecrits",
            entier(u64::try_from(module.reecrit().octets.len()).unwrap_or(u64::MAX)),
        ),
        ("profondeur_max", JsValue::from(REGLAGES.profondeur_max)),
        ("carburant_bonjour", entier(b)),
        ("carburant_decrire", entier(d)),
        ("reecrire_ms", nombres(&reecrire_ms)),
        ("compiler_ms", nombres(&compiler_ms)),
        ("mise_en_route_ms", nombres(&mise_en_route_ms)),
        ("octets_remis_a_zero", entier(octets_zones)),
        (
            "remise_a_zero_memoire_ms",
            JsValue::from_f64(zero_memoire_ms),
        ),
        ("remise_a_zero_seul_ms", JsValue::from_f64(zero_seul_ms)),
        ("bonjours", JsValue::from(BONJOURS)),
        ("bloc_avec_ms", nombres(&blocs)),
        ("passes", liste.into()),
    ]))
}

/// M1 : charge `livre` comme l'autorité, chronomètre la mise en route, la remise à zéro
/// et deux mille `Bonjour`, puis rejoue `octets_trace` `passes` fois, chaque appel
/// chronométré et comparé à wasmtime. Rend `{ granularite_ms, empreinte_reecrite, …,
/// passes: [{ total_ms, appels_us, …, ecarts, premier_ecart, pannes }] }`, ou
/// `{ erreur }` si l'étalonnage n'a pas pu tourner.
#[wasm_bindgen]
pub fn etalonner(livre: &[u8], octets_trace: &[u8], passes: u32) -> JsValue {
    mesurer(livre, octets_trace, passes).unwrap_or_else(|e| erreur(&e))
}

// M10 : la boucle vide.

/// Le module de `Bonjour` qui boucle, et son témoin qui piège dès l'entrée.
fn boucle_et_temoin() -> Result<(Vec<u8>, Vec<u8>), String> {
    let source = moteur_commun::essais::BONJOUR_QUI_BOUCLE;
    let temoin = source.replace("(loop $l (br $l))", "");
    if temoin == source {
        return Err(String::from("la boucle du module d'essai a changé"));
    }
    Ok((
        wat::parse_str(source).map_err(|e| e.to_string())?,
        wat::parse_str(&temoin).map_err(|e| e.to_string())?,
    ))
}

/// Un chargement par le protocole du commun, chronométré ; son issue doit être celle
/// qu'on attend.
fn charger_et_chronometrer(
    octets: &[u8],
    horloge: &Horloge,
    attendu: fn(&RefusModule) -> bool,
) -> Result<f64, String> {
    let t = horloge.ms();
    let r = charger(octets, &REGLAGES);
    let d = horloge.ms() - t;
    match r {
        Err(e) if attendu(&e) => Ok(d),
        Err(e) => Err(format!("issue inattendue : {e}")),
        Ok(_) => Err(String::from("le module s'est chargé")),
    }
}

/// Un appel brut à la machine, sous `budget` : la boucle, jusqu'à son carburant
/// épuisé. Rend la durée, en ms, et vérifie que c'est bien le carburant qui l'a arrêtée.
fn boucle_brute(
    machine: &mut MachineNavigateur,
    budget: u64,
    horloge: &Horloge,
) -> Result<f64, String> {
    let budget = i64::try_from(budget).map_err(|_| String::from("budget"))?;
    machine
        .poser_globale(CARBURANT, Bits::I64(budget))
        .and_then(|()| machine.poser_globale(PROFONDEUR, Bits::I32(0)))
        .map_err(|p| p.to_string())?;
    let debut = horloge.ms();
    let a = machine.boite(0, 0);
    let r = a.and_then(|_| machine.appel(0));
    let duree = horloge.ms() - debut;
    let reste = machine.globale(CARBURANT).map_err(|p| p.to_string())?;
    match (r, reste) {
        (Err(_), Bits::I64(x)) if x < 0 => Ok(duree),
        (r, x) => Err(format!(
            "la boucle ne s'est pas arrêtée sur son carburant : {r:?}, reste {x:?}"
        )),
    }
}

fn mesurer_la_boucle(fois: u32, grand_budget: u64) -> Result<JsValue, String> {
    let horloge = Horloge::new()?;
    let (boucle, temoin) = boucle_et_temoin()?;
    let budget = |e: &RefusModule| {
        *e == RefusModule::Faute {
            appel: "Bonjour",
            faute: Faute::Budget,
        }
    };
    let piege = |e: &RefusModule| {
        matches!(
            e,
            RefusModule::Faute {
                appel: "Bonjour",
                faute: Faute::Piege { .. }
            }
        )
    };
    let mut t = Vec::new();
    let mut b = Vec::new();
    for _ in 0..fois {
        t.push(charger_et_chronometrer(&temoin, &horloge, piege)?);
        b.push(charger_et_chronometrer(&boucle, &horloge, budget)?);
    }
    // La même boucle, par la machine seule, sous un plus grand budget : la mesure y
    // dépend moins de la granularité de l'horloge et du coût fixe.
    let r = reecriture::reecrire(&boucle, &REGLAGES).map_err(|r| r.to_string())?;
    let module = ModuleNavigateur::compiler(&r.octets).map_err(|p| p.to_string())?;
    let mut m = MachineNavigateur::new(&module).map_err(|p| p.to_string())?;
    m.poser_globale(MEMOIRE_MAX, Bits::I32(2))
        .map_err(|p| p.to_string())?;
    let mut brute = Vec::new();
    let mut petite = Vec::new();
    for _ in 0..fois.min(5) {
        petite.push(boucle_brute(&mut m, 1 << 10, &horloge)?);
        brute.push(boucle_brute(&mut m, grand_budget, &horloge)?);
    }
    Ok(objet(&[
        ("granularite_ms", JsValue::from_f64(horloge.granularite())),
        ("budget_de_chargement", entier(BUDGET_DE_CHARGEMENT)),
        ("temoin_ms", nombres(&t)),
        ("boucle_ms", nombres(&b)),
        ("grand_budget", entier(grand_budget)),
        ("brute_petite_ms", nombres(&petite)),
        ("brute_ms", nombres(&brute)),
    ]))
}

/// M10 : charge `fois` fois le témoin puis le module de `Bonjour` qui boucle, chacun
/// chronométré ; puis la même boucle par la machine seule sous `grand_budget` (et sous
/// 2¹⁰ unités, son témoin). Rend `{ temoin_ms, boucle_ms, brute_ms, … }`, ou
/// `{ erreur }`.
#[wasm_bindgen]
pub fn boucle_vide(fois: u32, grand_budget: f64) -> JsValue {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "un budget entier positif donné par la page, borné ici à 2⁴⁰"
    )]
    let g = grand_budget.clamp(1.0, 1_099_511_627_776.0) as u64;
    mesurer_la_boucle(fois, g).unwrap_or_else(|e| erreur(&e))
}

// M9 : la profondeur que tient le fil qui appelle.

/// Les cadres essayés : le nombre de valeurs `i64` que chaque étage de la récursion lit
/// en mémoire et garde vivantes à travers son appel. 250 est le plus épais qui compte
/// encore pour un seul étage (`reecriture::passes::etages` : 16 octets par valeur, 4 096
/// par étage) ; au-delà, un appel compte plusieurs étages.
pub const CADRES: [u32; 8] = [0, 16, 64, 128, 250, 500, 1000, 4000];

/// Les essais de chaque cadre, sur la même instance : le premier dans le code de base
/// du moteur, les suivants peut-être dans un code plus optimisé.
const ESSAIS: u32 = 3;

/// L'adresse du compteur d'appels, et celle des valeurs lues.
const COMPTEUR: u32 = 70_000;
const VALEURS: u32 = 72_000;

/// Une récursion sans fin dont chaque étage garde `locaux` valeurs vivantes, et compte
/// ses appels en mémoire (le module n'a droit qu'aux exports de § 3.3).
fn recursion(locaux: u32) -> String {
    let mut decl = String::new();
    let mut poser = String::new();
    let mut somme = String::from("(i64.const 0)");
    for k in 0..locaux {
        let _: std::fmt::Result = write!(decl, " (local $l{k} i64)");
        let _: std::fmt::Result = write!(
            poser,
            " (local.set $l{k} (i64.load (i32.const {})))",
            VALEURS.saturating_add(k.saturating_mul(8))
        );
        somme = format!("(i64.add {somme} (local.get $l{k}))");
    }
    format!(
        r#"(module
  (memory (export "memory") 3 3)
  (global $__stack_pointer (mut i32) (i32.const 65536))
  (func (export "soiree_boite") (param $n i32) (param $sorte i32) (result i32)
    (i32.const 65536))
  (func (export "soiree_appel") (param $n i32) (result i64)
    (i32.store (i32.const {COMPTEUR}) (i32.const 0))
    (call $r (i64.const 1)))
  (func $r (param $x i64) (result i64){decl}
    (i32.store (i32.const {COMPTEUR}) (i32.add (i32.load (i32.const {COMPTEUR})) (i32.const 1))){poser}
    (i64.add (call $r (i64.add (local.get $x) (i64.const 1))) {somme})))"#
    )
}

/// Un essai d'un cadre : la récursion jusqu'à ce que la pile native cède. Rend le piège,
/// les étages comptés par `soiree_profondeur` et les appels faits.
fn essayer_un_cadre(m: &mut MachineNavigateur) -> Result<(String, i64, u64), String> {
    m.poser_globale(CARBURANT, Bits::I64(i64::MAX / 4))
        .and_then(|()| m.poser_globale(PROFONDEUR, Bits::I32(0)))
        .map_err(|p| p.to_string())?;
    let r = m.boite(0, 0).and_then(|_| m.appel(0));
    let piege = match r {
        Err(Piege::PileNative) => String::from("PileNative"),
        Err(p) => format!("{p:?}"),
        Ok(v) => format!("rendu {v}"),
    };
    let etages = match m.globale(PROFONDEUR).map_err(|p| p.to_string())? {
        Bits::I32(x) => i64::from(x),
        autre @ (Bits::I64(_) | Bits::F32(_) | Bits::F64(_)) => {
            return Err(format!("soiree_profondeur n'est pas un i32 : {autre:?}"));
        }
    };
    let mut compteur = [0u8; 4];
    m.lire(COMPTEUR, &mut compteur).map_err(|p| p.to_string())?;
    Ok((piege, etages, u64::from(u32::from_le_bytes(compteur))))
}

fn mesurer_la_profondeur() -> Result<JsValue, String> {
    // Un plafond que rien n'atteint : la pile native cède d'abord.
    let sans_plafond = Reglages {
        profondeur_max: 0x7fff_0000,
        ..REGLAGES
    };
    let cadres = Array::new();
    for locaux in CADRES {
        let octets = wat::parse_str(recursion(locaux)).map_err(|e| e.to_string())?;
        let r = reecriture::reecrire(&octets, &sans_plafond).map_err(|r| r.to_string())?;
        let module: Rc<ModuleNavigateur> =
            ModuleNavigateur::compiler(&r.octets).map_err(|p| p.to_string())?;
        let mut m = MachineNavigateur::new(&module).map_err(|p| p.to_string())?;
        m.poser_globale(MEMOIRE_MAX, Bits::I32(3))
            .map_err(|p| p.to_string())?;
        let essais = Array::new();
        for _ in 0..ESSAIS {
            let (piege, etages, appels) = essayer_un_cadre(&mut m)?;
            essais.push(&objet(&[
                ("piege", JsValue::from_str(&piege)),
                (
                    "etages",
                    JsValue::from_f64(f64::from(i32::try_from(etages).unwrap_or(i32::MAX))),
                ),
                ("appels", entier(appels)),
            ]));
        }
        cadres.push(&objet(&[
            ("locaux", JsValue::from(locaux)),
            ("essais", essais.into()),
        ]));
    }
    // Puis chaque récursion telle que l'autorité la chargerait, sous `PHASE_3` : la
    // profondeur doit piquer avant la pile native de ce fil (§ 8.7). C'est ce qui tient
    // `PHASE_3` dans le navigateur ; `cadres_epais_jamais_la_pile_native` ne le prouve
    // que dans wasmtime.
    let sous_phase_3 = Array::new();
    for locaux in CADRES {
        let octets = wat::parse_str(recursion(locaux)).map_err(|e| e.to_string())?;
        let issue = match charger(&octets, &REGLAGES) {
            Err(RefusModule::Faute {
                faute: Faute::Profondeur,
                ..
            }) => String::from("Profondeur"),
            Err(e) => e.to_string(),
            Ok(_) => String::from("chargé"),
        };
        sous_phase_3.push(&objet(&[
            ("locaux", JsValue::from(locaux)),
            ("issue", JsValue::from_str(&issue)),
        ]));
    }
    Ok(objet(&[
        ("cadres", cadres.into()),
        ("sous_phase_3", sous_phase_3.into()),
        (
            "profondeur_max_phase_3",
            JsValue::from(REGLAGES.profondeur_max),
        ),
    ]))
}

/// M9 : pour chaque cadre de [`CADRES`], une récursion sans fin sur le fil qui appelle
/// (le Worker, ou la page) : le piège qui l'arrête (la pile native, `PileNative`), les
/// étages de `soiree_profondeur` atteints et les appels faits, [`ESSAIS`] fois ; puis,
/// pour chaque cadre, l'issue du chargement de la même récursion sous `PHASE_3`
/// (`Profondeur` attendu). Rend `{ cadres: [{ locaux, essais: [{ piege, etages, appels
/// }] }], sous_phase_3: [{ locaux, issue }], profondeur_max_phase_3 }`, ou `{ erreur }`.
#[wasm_bindgen]
pub fn profondeur_tenue() -> JsValue {
    mesurer_la_profondeur().unwrap_or_else(|e| erreur(&e))
}
