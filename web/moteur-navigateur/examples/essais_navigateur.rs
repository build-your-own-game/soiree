//! Les essais de [`moteur_commun::Machine`] dans un vrai navigateur (PHASE-3, lot 3a).
//!
//! Une bibliothèque WebAssembly : `wasm-bindgen` la relie à la page de
//! `soiree-banc machine`, qui appelle [`essayer`] et envoie ce qu'elle rend à
//! `POST /banc/rapport`. Les essais jouent sur `MachineNavigateur` (et, pour le contrôle
//! négatif, sur une machine fautive) le module `essais/machine.wat`, que la page assemble
//! elle-même : lire, écrire, remettre à zéro, faire grandir la mémoire, les globales
//! `i64` et des autres types, `soiree_boite` et `soiree_appel`, une instance neuve, les
//! pièges classés (d'après `essais/pieges.txt`, la référence de wasmtime) ; puis un
//! chargement de bout en bout, la réécriture et le protocole du commun compris.
//!
//! Le nom de chaque essai commence par sa famille (`lire`, `ecrire`, `remplir`,
//! `grandir`, `globales_i64`, `pieges`…) : le banc vérifie que chaque famille que le lot
//! demande a tourné et réussi.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::rc::Rc;

use js_sys::{Array, ArrayBuffer, Function, Object, Reflect, Uint8Array, WebAssembly};
use moteur_commun::{Bits, Machine, Piege, RefusModule};
use moteur_navigateur::{
    AMBIGUS, MESSAGES, MachineNavigateur, ModuleNavigateur, MoteurJs, PREFIXE_NON_RAMENE, Ramene,
    charger,
};
use partie::{CausePanne, Faute, PanneOutil};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};

/// Le module des essais.
const MACHINE: &str = include_str!("../essais/machine.wat");
/// Ses pièges, et ce que wasmtime en rend.
const PIEGES: &str = include_str!("../essais/pieges.txt");

/// La taille de la mémoire du module des essais, au départ : deux pages.
const TAILLE: u32 = 131_072;

/// Ce qu'un essai a constaté, ou pourquoi il échoue.
type Issue = Result<String, String>;

/// Une machine des essais : elle donne aussi sa mémoire, pour voir un tampon détaché.
trait Essayable: Machine {
    fn memoire(&self) -> &WebAssembly::Memory;
}

impl Essayable for MachineNavigateur {
    fn memoire(&self) -> &WebAssembly::Memory {
        MachineNavigateur::memoire(self)
    }
}

/// Le contrôle négatif des essais (`?fautif=i64-par-nombre`) : une machine qui lit et
/// pose les globales `i64` par un nombre du navigateur, comme le ferait une colle
/// écrite sans `BigInt`. Les essais `globales_i64` doivent échouer, et eux seuls.
struct I64ParNombre(MachineNavigateur);

fn par_un_nombre(b: Bits) -> Bits {
    match b {
        #[expect(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            reason = "c'est le défaut injecté : un i64 qui passe par un f64"
        )]
        Bits::I64(x) => Bits::I64(x as f64 as i64),
        autre @ (Bits::I32(_) | Bits::F32(_) | Bits::F64(_)) => autre,
    }
}

impl Machine for I64ParNombre {
    fn neuve(&mut self) -> Result<I64ParNombre, PanneOutil> {
        Ok(I64ParNombre(self.0.neuve()?))
    }
    fn pages(&mut self) -> u64 {
        self.0.pages()
    }
    fn lire(&mut self, adresse: u32, dans: &mut [u8]) -> Result<(), PanneOutil> {
        self.0.lire(adresse, dans)
    }
    fn ecrire(&mut self, adresse: u32, octets: &[u8]) -> Result<(), PanneOutil> {
        self.0.ecrire(adresse, octets)
    }
    fn remplir_de_zeros(&mut self, adresse: u32, longueur: u32) -> Result<(), PanneOutil> {
        self.0.remplir_de_zeros(adresse, longueur)
    }
    fn grandir_jusqu_a(&mut self, pages: u64) -> Result<(), PanneOutil> {
        self.0.grandir_jusqu_a(pages)
    }
    fn globale(&mut self, nom: &str) -> Result<Bits, PanneOutil> {
        self.0.globale(nom).map(par_un_nombre)
    }
    fn poser_globale(&mut self, nom: &str, b: Bits) -> Result<(), PanneOutil> {
        self.0.poser_globale(nom, par_un_nombre(b))
    }
    fn boite(&mut self, taille: i32, sorte: i32) -> Result<i32, Piege> {
        self.0.boite(taille, sorte)
    }
    fn appel(&mut self, taille: i32) -> Result<i64, Piege> {
        self.0.appel(taille)
    }
}

impl Essayable for I64ParNombre {
    fn memoire(&self) -> &WebAssembly::Memory {
        self.0.memoire()
    }
}

fn panne(p: &PanneOutil) -> String {
    format!("panne inattendue : {p}")
}

fn piege(p: &Piege) -> String {
    format!("piège inattendu : {p:?}")
}

fn lire<M: Machine>(m: &mut M, adresse: u32, n: usize) -> Result<Vec<u8>, String> {
    let mut v = vec![0; n];
    m.lire(adresse, &mut v).map_err(|p| panne(&p))?;
    Ok(v)
}

fn appel<M: Machine>(m: &mut M, k: i32) -> Result<i64, String> {
    m.appel(k).map_err(|p| piege(&p))
}

fn globale<M: Machine>(m: &mut M, nom: &str) -> Result<Bits, String> {
    m.globale(nom).map_err(|p| panne(&p))
}

/// Le tampon de la mémoire, tel qu'il est maintenant.
fn tampon<M: Essayable>(m: &M) -> Result<ArrayBuffer, String> {
    m.memoire()
        .buffer()
        .dyn_into::<ArrayBuffer>()
        .map_err(|_| String::from("memory.buffer n'est pas un ArrayBuffer"))
}

// Lire et écrire.

fn ecrire_puis_le_module_lit<M: Essayable>(m: &mut M) -> Issue {
    let v: i64 = 0x0102_0304_0506_0708;
    m.ecrire(1024, &v.to_le_bytes()).map_err(|p| panne(&p))?;
    let r = appel(m, 0)?;
    (r == v)
        .then_some(())
        .ok_or_else(|| format!("le module lit {r:#x}"))?;
    Ok(String::from("le module lit ce que la machine a écrit"))
}

fn lire_ce_que_le_module_ecrit<M: Essayable>(m: &mut M) -> Issue {
    appel(m, 1)?;
    let b = lire(m, 2048, 8)?;
    let attendu = 0x1122_3344_5566_7788_i64.to_le_bytes();
    (b == attendu)
        .then_some(())
        .ok_or_else(|| format!("lu {b:x?}"))?;
    Ok(String::from("la machine lit ce que le module a écrit"))
}

fn lire_ecrire_aller_retour_a_travers_une_page<M: Essayable>(m: &mut M) -> Issue {
    let octets: Vec<u8> = (0..=255u8).cycle().take(70_000).collect();
    m.ecrire(60_000, &octets).map_err(|p| panne(&p))?;
    let relu = lire(m, 60_000, octets.len())?;
    (relu == octets)
        .then_some(())
        .ok_or_else(|| String::from("relu différent"))?;
    Ok(String::from(
        "70 000 octets de part et d'autre de la page 1",
    ))
}

fn lire_hors_de_la_memoire_est_une_panne<M: Essayable>(m: &mut M) -> Issue {
    lire(m, TAILLE.saturating_sub(8), 8)?;
    let mut b = [0u8; 8];
    (m.lire(TAILLE.saturating_sub(7), &mut b).is_err())
        .then_some(())
        .ok_or_else(|| String::from("une lecture qui dépasse d'un octet a réussi"))?;
    (m.lire(u32::MAX, &mut b).is_err())
        .then_some(())
        .ok_or_else(|| String::from("une lecture à u32::MAX a réussi"))?;
    Ok(String::from(
        "les 8 derniers octets se lisent ; un de plus est une panne",
    ))
}

fn ecrire_hors_de_la_memoire_est_une_panne<M: Essayable>(m: &mut M) -> Issue {
    (m.ecrire(TAILLE, &[1]).is_err())
        .then_some(())
        .ok_or_else(|| String::from("une écriture après la fin a réussi"))?;
    m.ecrire(TAILLE.saturating_sub(1), &[1])
        .map_err(|p| panne(&p))?;
    Ok(String::from(
        "le dernier octet s'écrit ; après la fin, une panne",
    ))
}

// Remettre à zéro.

fn remplir_de_zeros_aux_bornes<M: Essayable>(m: &mut M) -> Issue {
    m.ecrire(4000, &[0xff; 100]).map_err(|p| panne(&p))?;
    m.remplir_de_zeros(4010, 80).map_err(|p| panne(&p))?;
    let b = lire(m, 4000, 100)?;
    let mut attendu = vec![0xff_u8; 10];
    attendu.extend([0u8; 80]);
    attendu.extend([0xff_u8; 10]);
    (b == attendu)
        .then_some(())
        .ok_or_else(|| format!("lu {b:x?}"))?;
    Ok(String::from("exactement [4010, 4090)"))
}

fn remplir_hors_de_la_memoire_est_une_panne<M: Essayable>(m: &mut M) -> Issue {
    (m.remplir_de_zeros(TAILLE.saturating_sub(50), 100).is_err())
        .then_some(())
        .ok_or_else(|| String::from("remplir au-delà de la fin a réussi"))?;
    Ok(String::from("une panne"))
}

// Faire grandir.

fn grandir_garde_la_memoire<M: Essayable>(m: &mut M) -> Issue {
    m.ecrire(100, &[7; 4]).map_err(|p| panne(&p))?;
    m.grandir_jusqu_a(3).map_err(|p| panne(&p))?;
    (m.pages() == 3)
        .then_some(())
        .ok_or_else(|| format!("{} pages", m.pages()))?;
    (lire(m, 100, 4)? == [7; 4])
        .then_some(())
        .ok_or_else(|| String::from("l'ancienne page a changé"))?;
    (lire(m, TAILLE, 65_536)?.iter().all(|o| *o == 0))
        .then_some(())
        .ok_or_else(|| String::from("la page nouvelle n'est pas à zéro"))?;
    m.ecrire(196_607, &[9]).map_err(|p| panne(&p))?;
    Ok(String::from(
        "2 à 3 pages ; l'ancienne gardée, la nouvelle à zéro",
    ))
}

/// `memory.grow(0)` détache le tampon : à taille égale, la machine ne demande rien
/// (lot 2, correction, constat 4). L'essai voit le détachement : à la croissance qui
/// suit, l'ancien tampon tombe à zéro octet.
fn grandir_a_la_meme_taille_ne_detache_pas<M: Essayable>(m: &mut M) -> Issue {
    let avant = tampon(m)?;
    m.grandir_jusqu_a(2).map_err(|p| panne(&p))?;
    m.grandir_jusqu_a(1).map_err(|p| panne(&p))?;
    (avant.byte_length() == TAILLE)
        .then_some(())
        .ok_or_else(|| format!("tampon détaché ({} octets)", avant.byte_length()))?;
    m.grandir_jusqu_a(3).map_err(|p| panne(&p))?;
    (avant.byte_length() == 0).then_some(()).ok_or_else(|| {
        String::from("une vraie croissance n'a pas détaché le tampon : l'essai ne voit rien")
    })?;
    Ok(String::from(
        "tampon gardé à taille égale ou moindre, détaché par une vraie croissance",
    ))
}

fn grandir_au_dela_du_maximum_est_refuse<M: Essayable>(m: &mut M) -> Issue {
    match m.grandir_jusqu_a(5) {
        Ok(()) => Err(String::from("5 pages accordées, le maximum est 4")),
        Err(p) if p.cause == CausePanne::CroissanceRefusee => {
            (m.pages() == 2)
                .then_some(())
                .ok_or_else(|| format!("{} pages après le refus", m.pages()))?;
            Ok(format!("CroissanceRefusee : {}", p.detail))
        }
        Err(p) => Err(panne(&p)),
    }
}

fn grandir_par_le_module_se_voit<M: Essayable>(m: &mut M) -> Issue {
    let r = appel(m, 13)?;
    (r == 3)
        .then_some(())
        .ok_or_else(|| format!("le module dit {r} pages"))?;
    (m.pages() == 3)
        .then_some(())
        .ok_or_else(|| format!("la machine dit {} pages", m.pages()))?;
    (lire(m, TAILLE, 1)? == [0x5a])
        .then_some(())
        .ok_or_else(|| String::from("l'octet écrit par le module dans la page nouvelle manque"))?;
    Ok(String::from(
        "le tampon est relu après le memory.grow du module",
    ))
}

// Les globales.

const CARBURANT: &str = "soiree_carburant";

fn globales_i64_lues_exactement<M: Essayable>(m: &mut M) -> Issue {
    let g = globale(m, CARBURANT)?;
    (g == Bits::I64(-9_007_199_254_740_993))
        .then_some(())
        .ok_or_else(|| format!("lu {g:?}"))?;
    Ok(String::from("-(2⁵³ + 1), que le module pose à sa création"))
}

fn globales_i64_posees_exactement<M: Essayable>(m: &mut M) -> Issue {
    for v in [
        i64::MIN,
        i64::MAX,
        -1,
        0,
        9_007_199_254_740_993,
        -9_007_199_254_740_993,
    ] {
        m.poser_globale(CARBURANT, Bits::I64(v))
            .map_err(|p| panne(&p))?;
        let g = globale(m, CARBURANT)?;
        (g == Bits::I64(v))
            .then_some(())
            .ok_or_else(|| format!("posé {v}, relu {g:?}"))?;
        let r = appel(m, 14)?;
        (r == v)
            .then_some(())
            .ok_or_else(|| format!("posé {v}, le module lit {r}"))?;
    }
    Ok(String::from(
        "i64::MIN, i64::MAX, -1, 0, ±(2⁵³ + 1) : relus, et lus par le module",
    ))
}

fn globales_i64_ecrites_par_le_module<M: Essayable>(m: &mut M) -> Issue {
    appel(m, 15)?;
    let g = globale(m, CARBURANT)?;
    (g == Bits::I64(-9_223_372_036_854_775_807))
        .then_some(())
        .ok_or_else(|| format!("lu {g:?}"))?;
    Ok(String::from("0x8000000000000001, posé par le module"))
}

fn globales_des_autres_types<M: Essayable>(m: &mut M) -> Issue {
    let lus = [
        (globale(m, "soiree_globale_0")?, Bits::I32(65_536)),
        (globale(m, "essai_f32")?, Bits::F32(1.5_f32.to_bits())),
        (globale(m, "essai_f64")?, Bits::F64((-0.1_f64).to_bits())),
    ];
    for (lu, attendu) in lus {
        (lu == attendu)
            .then_some(())
            .ok_or_else(|| format!("lu {lu:?}, attendu {attendu:?}"))?;
    }
    let poses = [
        ("soiree_globale_0", Bits::I32(-5)),
        ("essai_f32", Bits::F32(reecriture::passes::NAN_F32)),
        ("essai_f64", Bits::F64(reecriture::passes::NAN_F64)),
        ("essai_f64", Bits::F64((-0.0_f64).to_bits())),
    ];
    for (nom, b) in poses {
        m.poser_globale(nom, b).map_err(|p| panne(&p))?;
        let g = globale(m, nom)?;
        (g == b)
            .then_some(())
            .ok_or_else(|| format!("{nom} : posé {b:?}, relu {g:?}"))?;
    }
    let r = appel(m, 18)?;
    (r.cast_unsigned() == (-0.0_f64).to_bits())
        .then_some(())
        .ok_or_else(|| format!("le module lit {r:#x}"))?;
    Ok(String::from(
        "i32, f32, f64 relus ; NaN canoniques et -0 posés, relus, lus par le module",
    ))
}

fn globales_inconnue_immuable_ou_mal_typee_sont_des_pannes<M: Essayable>(m: &mut M) -> Issue {
    (m.globale("absente").is_err())
        .then_some(())
        .ok_or_else(|| String::from("une globale absente se lit"))?;
    (m.poser_globale("essai_fixe", Bits::I32(1)).is_err())
        .then_some(())
        .ok_or_else(|| String::from("une globale immuable se pose"))?;
    (m.poser_globale(CARBURANT, Bits::I32(1)).is_err())
        .then_some(())
        .ok_or_else(|| String::from("un i32 se pose dans une globale i64"))?;
    (globale(m, "essai_fixe")? == Bits::I32(7))
        .then_some(())
        .ok_or_else(|| String::from("la globale immuable a changé"))?;
    Ok(String::from("trois pannes, rien de changé"))
}

// Appeler.

fn appel_boite_passe_ses_arguments<M: Essayable>(m: &mut M) -> Issue {
    let b = m.boite(3, 2).map_err(|p| piege(&p))?;
    (b == 50)
        .then_some(())
        .ok_or_else(|| format!("soiree_boite(3, 2) = {b}"))?;
    let refus = m.boite(4097, 0).map_err(|p| piege(&p))?;
    (refus == 0)
        .then_some(())
        .ok_or_else(|| format!("soiree_boite(4097, 0) = {refus}"))?;
    Ok(String::from(
        "soiree_boite(3, 2) = 50, au-delà de 4 096 : 0",
    ))
}

fn appel_rend_un_i64_exact<M: Essayable>(m: &mut M) -> Issue {
    for (k, attendu) in [(16, 0x0123_4567_89ab_cdef_i64), (17, -2), (99, -1)] {
        let r = appel(m, k)?;
        (r == attendu)
            .then_some(())
            .ok_or_else(|| format!("scénario {k} : {r:#x}"))?;
    }
    Ok(String::from(
        "0x0123456789abcdef, -2, -1 : par BigInt, exacts",
    ))
}

fn neuve_repart_de_l_origine<M: Essayable>(m: &mut M) -> Issue {
    m.ecrire(1024, &[1; 8]).map_err(|p| panne(&p))?;
    m.poser_globale(CARBURANT, Bits::I64(5))
        .map_err(|p| panne(&p))?;
    m.grandir_jusqu_a(3).map_err(|p| panne(&p))?;
    let mut n = m.neuve().map_err(|p| panne(&p))?;
    (n.pages() == 2)
        .then_some(())
        .ok_or_else(|| format!("{} pages", n.pages()))?;
    (lire(&mut n, 1024, 8)? == [0; 8])
        .then_some(())
        .ok_or_else(|| String::from("la mémoire de l'ancienne instance"))?;
    // Par le module, et non par `globale` : l'essai ne dépend pas de la lecture des
    // `i64`, que le contrôle négatif fausse.
    let g = appel(&mut n, 14)?;
    (g == -9_007_199_254_740_993)
        .then_some(())
        .ok_or_else(|| format!("le module lit {g}"))?;
    (m.pages() == 3)
        .then_some(())
        .ok_or_else(|| String::from("l'ancienne instance a changé"))?;
    Ok(String::from("mémoire, taille et globales d'origine"))
}

// Les pièges.

/// Un scénario de `pieges.txt`.
struct ScenarioPiege {
    k: i32,
    nom: String,
    attendu: Piege,
}

fn scenarios_de_pieges() -> Result<Vec<ScenarioPiege>, String> {
    let mut r = Vec::new();
    for ligne in PIEGES.lines() {
        if ligne.is_empty() || ligne.starts_with('#') {
            continue;
        }
        let mut champs = ligne.splitn(4, ' ');
        let (Some(k), Some(nom), Some(sorte)) = (champs.next(), champs.next(), champs.next())
        else {
            return Err(format!("pieges.txt : « {ligne} »"));
        };
        let k: i32 = k.parse().map_err(|_| format!("pieges.txt : « {ligne} »"))?;
        let attendu = match (sorte, champs.next()) {
            ("hors-memoire", None) => Piege::HorsMemoire,
            ("pile-native", None) => Piege::PileNative,
            ("code", Some(code)) => Piege::Autre(String::from(code)),
            _ => return Err(format!("pieges.txt : « {ligne} »")),
        };
        r.push(ScenarioPiege {
            k,
            nom: format!("pieges_{nom}"),
            attendu,
        });
    }
    Ok(r)
}

/// Le piège rendu doit être celui de wasmtime. Seul un message connu pour désigner
/// plusieurs pièges de wasmtime ([`AMBIGUS`]) reste sous le texte du navigateur, et
/// seulement si le piège que wasmtime rend ici est l'un de ceux qu'il désigne ; un
/// message inconnu échoue (il est à constater, puis à ranger), un message ambigu levé
/// pour un autre piège aussi, et un autre nom de wasmtime aussi.
fn juger_piege<M: Essayable>(m: &mut M, s: &ScenarioPiege, brut: &str) -> Issue {
    let constat = match m.appel(s.k) {
        Ok(r) => return Err(format!("aucun piège, rendu {r}")),
        Err(p) => p,
    };
    let verdict = match (&constat, &s.attendu) {
        (c, a) if c == a => Ok(format!("{c:?}, comme wasmtime")),
        (Piege::Autre(c), a)
            if c.strip_prefix(PREFIXE_NON_RAMENE).is_some_and(|m| {
                AMBIGUS.iter().any(|(_, ambigu, designes)| {
                    *ambigu == m
                        && designes.iter().any(|d| match (d, a) {
                            (Ramene::HorsMemoire, Piege::HorsMemoire) => true,
                            (Ramene::Code(code), Piege::Autre(a)) => code == a,
                            (Ramene::HorsMemoire | Ramene::Code(_), _) => false,
                        })
                })
            }) =>
        {
            Ok(format!(
                "non ramené, le message désigne plusieurs pièges de wasmtime : {c}"
            ))
        }
        (c, a) => Err(format!("{c:?}, wasmtime rend {a:?}")),
    }?;
    // L'instance sert encore après un piège.
    let r = appel(m, 16)?;
    (r == 0x0123_4567_89ab_cdef)
        .then_some(())
        .ok_or_else(|| String::from("l'instance ne répond plus après le piège"))?;
    Ok(format!("{verdict} (levé : {brut})"))
}

/// Ce que le navigateur lève pour le scénario `k`, brut : un nom et un message.
fn leve(module: &WebAssembly::Module, k: i32) -> Result<(String, String), String> {
    let i = WebAssembly::Instance::new(module, &Object::new()).map_err(|e| format!("{e:?}"))?;
    let f = Reflect::get(&i.exports(), &JsValue::from_str("soiree_appel"))
        .ok()
        .and_then(|f| f.dyn_into::<Function>().ok())
        .ok_or_else(|| String::from("soiree_appel absent"))?;
    match f.call1(&JsValue::UNDEFINED, &JsValue::from(k)) {
        Ok(_) => Err(format!("scénario {k} : rien de levé")),
        Err(e) => match e.dyn_ref::<js_sys::Error>() {
            Some(e) => Ok((e.name().into(), e.message().into())),
            None => Ok((String::from("?"), format!("{e:?}"))),
        },
    }
}

/// Chaque message des deux tables que lève le moteur de ce navigateur a été levé par un
/// scénario : elles ne tiennent que des messages constatés. Un navigateur dont le moteur
/// n'est pas rangé (Safari, lot 3c) échoue : ses messages ne sont pas dans les tables.
fn pieges_la_table_est_constatee(leves: &[(i32, String, String)]) -> Issue {
    let agent = Reflect::get(&js_sys::global(), &JsValue::from_str("navigator"))
        .and_then(|n| Reflect::get(&n, &JsValue::from_str("userAgent")))
        .ok()
        .and_then(|a| a.as_string())
        .unwrap_or_default();
    let moteur = MoteurJs::depuis_agent(&agent)
        .ok_or_else(|| format!("moteur inconnu, aucun message rangé pour lui : {agent}"))?;
    let siens = MESSAGES
        .iter()
        .filter(|(x, _, _)| *x == moteur)
        .map(|(_, m, _)| *m)
        .chain(
            AMBIGUS
                .iter()
                .filter(|(x, _, _)| *x == moteur)
                .map(|(_, m, _)| *m),
        );
    let mut ramenes = 0usize;
    let mut manquent = Vec::new();
    for m in siens {
        ramenes = ramenes.saturating_add(1);
        if !leves.iter().any(|(_, _, l)| l == m) {
            manquent.push(m);
        }
    }
    (manquent.is_empty())
        .then_some(())
        .ok_or_else(|| format!("jamais levés ici ({moteur:?}) : {manquent:?}"))?;
    let ambigus = AMBIGUS.iter().filter(|(x, _, _)| *x == moteur).count();
    Ok(format!(
        "{moteur:?} : {} messages ramenés et {ambigus} ambigus, tous levés ici",
        ramenes.saturating_sub(ambigus)
    ))
}

// Le chargement, de bout en bout.

/// Le module de `Bonjour` qui boucle, réécrit et chargé dans le navigateur : la
/// réécriture, le carburant `i64` et le protocole du commun classent la boucle en faute
/// `Budget` du jeu (PHASE-3, P5).
fn chargement_d_une_boucle_est_la_faute_budget() -> Issue {
    let octets =
        wat::parse_str(moteur_commun::essais::BONJOUR_QUI_BOUCLE).map_err(|e| e.to_string())?;
    match charger(&octets, &reecriture::Reglages::PHASE_1) {
        Err(RefusModule::Faute {
            appel,
            faute: Faute::Budget,
        }) => Ok(format!("{appel} : faute Budget")),
        Err(e) => Err(format!("refusé autrement : {e}")),
        Ok(_) => Err(String::from("chargé")),
    }
}

// La suite.

/// Ce que la suite a constaté.
struct Essais {
    liste: Vec<(String, Issue)>,
    leves: Vec<(i32, String, String)>,
}

type Essai<M> = fn(&mut M) -> Issue;

fn suite<M: Essayable>(
    octets: &[u8],
    neuve: fn(&Rc<ModuleNavigateur>) -> Result<M, PanneOutil>,
) -> Result<Essais, String> {
    let module = ModuleNavigateur::compiler(octets).map_err(|p| panne(&p))?;
    let brut =
        WebAssembly::Module::new(&Uint8Array::from(octets).into()).map_err(|e| format!("{e:?}"))?;
    let essais: [(&str, Essai<M>); 20] = [
        ("ecrire_puis_le_module_lit", ecrire_puis_le_module_lit),
        ("lire_ce_que_le_module_ecrit", lire_ce_que_le_module_ecrit),
        (
            "lire_ecrire_aller_retour_a_travers_une_page",
            lire_ecrire_aller_retour_a_travers_une_page,
        ),
        (
            "lire_hors_de_la_memoire_est_une_panne",
            lire_hors_de_la_memoire_est_une_panne,
        ),
        (
            "ecrire_hors_de_la_memoire_est_une_panne",
            ecrire_hors_de_la_memoire_est_une_panne,
        ),
        ("remplir_de_zeros_aux_bornes", remplir_de_zeros_aux_bornes),
        (
            "remplir_hors_de_la_memoire_est_une_panne",
            remplir_hors_de_la_memoire_est_une_panne,
        ),
        ("grandir_garde_la_memoire", grandir_garde_la_memoire),
        (
            "grandir_a_la_meme_taille_ne_detache_pas",
            grandir_a_la_meme_taille_ne_detache_pas,
        ),
        (
            "grandir_au_dela_du_maximum_est_refuse",
            grandir_au_dela_du_maximum_est_refuse,
        ),
        (
            "grandir_par_le_module_se_voit",
            grandir_par_le_module_se_voit,
        ),
        ("globales_i64_lues_exactement", globales_i64_lues_exactement),
        (
            "globales_i64_posees_exactement",
            globales_i64_posees_exactement,
        ),
        (
            "globales_i64_ecrites_par_le_module",
            globales_i64_ecrites_par_le_module,
        ),
        ("globales_des_autres_types", globales_des_autres_types),
        (
            "globales_inconnue_immuable_ou_mal_typee_sont_des_pannes",
            globales_inconnue_immuable_ou_mal_typee_sont_des_pannes,
        ),
        (
            "appel_boite_passe_ses_arguments",
            appel_boite_passe_ses_arguments,
        ),
        ("appel_rend_un_i64_exact", appel_rend_un_i64_exact),
        ("neuve_repart_de_l_origine", neuve_repart_de_l_origine),
        ("neuve_apres_un_piege", neuve_apres_un_piege),
    ];
    let mut liste = Vec::new();
    for (nom, essai) in essais {
        let issue = neuve(&module)
            .map_err(|p| panne(&p))
            .and_then(|mut m| essai(&mut m));
        liste.push((String::from(nom), issue));
    }
    let mut leves = Vec::new();
    for s in scenarios_de_pieges()? {
        let (nom_leve, message) = leve(&brut, s.k)?;
        let brut_texte = format!("{nom_leve} : {message}");
        let issue = neuve(&module)
            .map_err(|p| panne(&p))
            .and_then(|mut m| juger_piege(&mut m, &s, &brut_texte));
        liste.push((s.nom, issue));
        leves.push((s.k, nom_leve, message));
    }
    liste.push((
        String::from("pieges_la_table_est_constatee"),
        pieges_la_table_est_constatee(&leves),
    ));
    liste.push((
        String::from("chargement_d_une_boucle_est_la_faute_budget"),
        chargement_d_une_boucle_est_la_faute_budget(),
    ));
    Ok(Essais { liste, leves })
}

/// Après un piège, une instance neuve repart de l'origine : le protocole en demande une
/// pour restaurer un instantané plus petit.
fn neuve_apres_un_piege<M: Essayable>(m: &mut M) -> Issue {
    m.grandir_jusqu_a(3).map_err(|p| panne(&p))?;
    (m.appel(2).is_err())
        .then_some(())
        .ok_or_else(|| String::from("unreachable n'a pas piégé"))?;
    let mut n = m.neuve().map_err(|p| panne(&p))?;
    (n.pages() == 2)
        .then_some(())
        .ok_or_else(|| format!("{} pages", n.pages()))?;
    let r = appel(&mut n, 16)?;
    (r == 0x0123_4567_89ab_cdef)
        .then_some(())
        .ok_or_else(|| format!("rendu {r:#x}"))?;
    Ok(String::from("deux pages, et elle répond"))
}

fn objet(champs: &[(&str, JsValue)]) -> JsValue {
    let o = Object::new();
    for (cle, v) in champs {
        let _: Result<bool, JsValue> = Reflect::set(&o, &JsValue::from_str(cle), v);
    }
    o.into()
}

fn rapport(e: &Essais) -> JsValue {
    let essais = Array::new();
    for (nom, issue) in &e.liste {
        let (reussi, detail) = match issue {
            Ok(d) => (true, d),
            Err(d) => (false, d),
        };
        essais.push(&objet(&[
            ("nom", JsValue::from_str(nom)),
            ("reussi", JsValue::from_bool(reussi)),
            ("detail", JsValue::from_str(detail)),
        ]));
    }
    let leves = Array::new();
    for (k, nom, message) in &e.leves {
        leves.push(&objet(&[
            ("scenario", JsValue::from(*k)),
            ("nom", JsValue::from_str(nom)),
            ("message", JsValue::from_str(message)),
        ]));
    }
    objet(&[("essais", essais.into()), ("leves", leves.into())])
}

/// Joue les essais de `Machine` et rend leur rapport : `{ essais: [{ nom, reussi,
/// detail }], leves: [{ scenario, nom, message }] }`, ou `{ erreur }` si la suite n'a
/// pas pu tourner. `fautif` vaut `""` pour la vraie machine, `"i64-par-nombre"` pour
/// le contrôle négatif.
#[wasm_bindgen]
pub fn essayer(fautif: &str) -> JsValue {
    let issue =
        wat::parse_str(MACHINE)
            .map_err(|e| e.to_string())
            .and_then(|octets| match fautif {
                "" => suite(&octets, MachineNavigateur::new),
                "i64-par-nombre" => suite(&octets, |m| MachineNavigateur::new(m).map(I64ParNombre)),
                autre => Err(format!("fautif inconnu : {autre}")),
            });
    match issue {
        Ok(e) => rapport(&e),
        Err(erreur) => objet(&[("erreur", JsValue::from_str(&erreur))]),
    }
}
