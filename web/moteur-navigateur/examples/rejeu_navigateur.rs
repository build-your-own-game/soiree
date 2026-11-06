//! Le rejeu de E20 dans un vrai navigateur (PHASE-3, § 8.2 et § 8.3, lot 3b).
//!
//! Une bibliothèque WebAssembly : `wasm-bindgen` la relie au Worker de la page de
//! `soiree-banc rejouer`, qui appelle [`rejouer`] avec le module livré, les soirées de
//! l'épreuve et le relevé de wasmtime, puis envoie ce qu'elle rend à
//! `POST /banc/rapport`. Le module est chargé ici comme le Worker de l'autorité le
//! chargera (`moteur_navigateur::charger` : réécriture, compilation, `Bonjour`,
//! `Decrire`), puis chaque soirée est reprise depuis sa création par la procédure de
//! `moteur_commun::releve`, chaque événement comparé au relevé de wasmtime dès qu'il
//! arrive.
//!
//! # Le moteur fautif
//!
//! `fausser-apres:<n>` joue le contrôle négatif de E20 : une machine qui, passé `n`
//! appels de `soiree_appel`, rend le carburant restant d'une unité de trop. Le relevé
//! doit diverger à l'appel `n + 1` (le premier faussé), sur le carburant, et pas avant.
//! Ce n'est pas une crate de module : c'est le moteur qui est faux, et aucun jeu ne sait
//! faire échouer E20 (PHASE-3, § 8.3).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::cell::Cell;
use std::rc::Rc;

use js_sys::{Array, Function, Object, Reflect};
use moteur_commun::releve::{self, Chargement, Enregistreuse, Evenement, Releve};
use moteur_commun::{Bits, Machine, Piege, Protocole};
use moteur_navigateur::{MachineNavigateur, ModuleCharge, charger};
use partie::{Fabrique, PanneOutil};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};

/// Le préfixe du moteur fautif dans `fautif`.
const FAUSSER_APRES: &str = "fausser-apres:";

/// Une machine qui, passé `apres` appels de `soiree_appel` comptés sur toutes les
/// machines de sa fabrique, lit le carburant restant d'une unité de trop.
struct Faussee {
    vraie: MachineNavigateur,
    appels: Rc<Cell<u64>>,
    apres: u64,
}

impl Machine for Faussee {
    fn neuve(&mut self) -> Result<Faussee, PanneOutil> {
        Ok(Faussee {
            vraie: self.vraie.neuve()?,
            appels: Rc::clone(&self.appels),
            apres: self.apres,
        })
    }
    fn pages(&mut self) -> u64 {
        self.vraie.pages()
    }
    fn lire(&mut self, adresse: u32, dans: &mut [u8]) -> Result<(), PanneOutil> {
        self.vraie.lire(adresse, dans)
    }
    fn ecrire(&mut self, adresse: u32, octets: &[u8]) -> Result<(), PanneOutil> {
        self.vraie.ecrire(adresse, octets)
    }
    fn remplir_de_zeros(&mut self, adresse: u32, longueur: u32) -> Result<(), PanneOutil> {
        self.vraie.remplir_de_zeros(adresse, longueur)
    }
    fn grandir_jusqu_a(&mut self, pages: u64) -> Result<(), PanneOutil> {
        self.vraie.grandir_jusqu_a(pages)
    }
    fn globale(&mut self, nom: &str) -> Result<Bits, PanneOutil> {
        let b = self.vraie.globale(nom)?;
        Ok(match b {
            Bits::I64(x) if nom == reecriture::CARBURANT && self.appels.get() > self.apres => {
                Bits::I64(x.wrapping_add(1))
            }
            autre @ (Bits::I32(_) | Bits::I64(_) | Bits::F32(_) | Bits::F64(_)) => autre,
        })
    }
    fn poser_globale(&mut self, nom: &str, b: Bits) -> Result<(), PanneOutil> {
        self.vraie.poser_globale(nom, b)
    }
    fn boite(&mut self, taille: i32, sorte: i32) -> Result<i32, Piege> {
        self.vraie.boite(taille, sorte)
    }
    fn appel(&mut self, taille: i32) -> Result<i64, Piege> {
        self.appels.set(self.appels.get().saturating_add(1));
        self.vraie.appel(taille)
    }
}

/// La fabrique du moteur fautif.
#[derive(Clone)]
struct FabriqueFaussee {
    module: ModuleCharge,
    appels: Rc<Cell<u64>>,
    apres: u64,
}

impl Fabrique for FabriqueFaussee {
    type I = Protocole<Faussee>;

    fn neuve(&mut self) -> Result<Protocole<Faussee>, PanneOutil> {
        let (appels, apres) = (Rc::clone(&self.appels), self.apres);
        self.module.instance_sur(|vraie| Faussee {
            vraie,
            appels,
            apres,
        })
    }
}

/// `performance.now()`, en millisecondes ; 0 si le navigateur ne le donne pas.
fn maintenant_ms() -> f64 {
    let p = Reflect::get(&js_sys::global(), &JsValue::from_str("performance")).ok();
    let now = p
        .as_ref()
        .and_then(|p| Reflect::get(p, &JsValue::from_str("now")).ok())
        .and_then(|f| f.dyn_into::<Function>().ok());
    match (p, now) {
        (Some(p), Some(f)) => f.call0(&p).ok().and_then(|v| v.as_f64()).unwrap_or(0.0),
        _ => 0.0,
    }
}

fn objet(champs: &[(&str, JsValue)]) -> JsValue {
    let o = Object::new();
    for (cle, v) in champs {
        let _: Result<bool, JsValue> = Reflect::set(&o, &JsValue::from_str(cle), v);
    }
    o.into()
}

fn nombre(n: usize) -> JsValue {
    JsValue::from(u32::try_from(n).unwrap_or(u32::MAX))
}

fn erreur(quoi: &str) -> JsValue {
    objet(&[("erreur", JsValue::from_str(quoi))])
}

/// Ce que le rejeu a constaté.
struct Constat {
    chargement: Chargement,
    evenements: Vec<Evenement>,
    ecart: Option<(usize, String)>,
    chargement_ms: f64,
    rejeu_ms: f64,
}

fn rejouer_sur<F: Fabrique + Clone>(
    f: F,
    soirees: &[releve::SoireeJouee],
    attendu: Vec<Evenement>,
) -> (Vec<Evenement>, Option<(usize, String)>) {
    let e = Enregistreuse::comparant(f, attendu);
    releve::rejouer(&e, soirees);
    let ecart = e.ecart().map(|x| (x.indice, x.to_string()));
    (e.evenements(), ecart)
}

fn constater(
    module: &[u8],
    soirees: &[releve::SoireeJouee],
    attendu: Releve,
    fautif: Option<u64>,
) -> Constat {
    let debut = maintenant_ms();
    let charge = charger(module, &reecriture::Reglages::PHASE_3);
    let chargement = match &charge {
        Ok(m) => Chargement::Accepte {
            empreinte: m.reecrit().empreinte,
            carburant: m.carburant_du_chargement(),
        },
        Err(r) => Chargement::Refuse(releve::classer_refus(r)),
    };
    let chargement_ms = maintenant_ms() - debut;
    if chargement != attendu.chargement {
        return Constat {
            ecart: Some((
                0,
                format!(
                    "le chargement diffère : wasmtime {:?} ; le navigateur {chargement:?}",
                    attendu.chargement
                ),
            )),
            chargement,
            evenements: Vec::new(),
            chargement_ms,
            rejeu_ms: 0.0,
        };
    }
    let debut = maintenant_ms();
    let (evenements, ecart) = match (charge, fautif) {
        (Ok(m), None) => rejouer_sur(m.fabrique(), soirees, attendu.evenements),
        (Ok(m), Some(apres)) => rejouer_sur(
            FabriqueFaussee {
                module: m,
                appels: Rc::new(Cell::new(0)),
                apres,
            },
            soirees,
            attendu.evenements,
        ),
        // Refusé pareil des deux côtés : rien à reprendre, et l'attendu n'a rien.
        (Err(_), _) => {
            let ecart = (!attendu.evenements.is_empty()).then(|| {
                (
                    0,
                    String::from("wasmtime a des événements pour un module refusé"),
                )
            });
            (Vec::new(), ecart)
        }
    };
    Constat {
        chargement,
        evenements,
        ecart,
        chargement_ms,
        rejeu_ms: maintenant_ms() - debut,
    }
}

/// Reprend les soirées de l'épreuve depuis leur création, dans ce navigateur, et
/// compare chaque événement au relevé de wasmtime. Rend `{ fautif, chargement,
/// attendus, obtenus, appels, fautes, empreinte, ecart, ecart_indice, ecart_appel,
/// chargement_ms, rejeu_ms }`, ou `{ erreur }` si le rejeu n'a pas pu tourner.
///
/// `empreinte` est le BLAKE3 du relevé obtenu, encodé comme celui de wasmtime : le banc
/// le compare lui-même au fichier qu'il a servi, sans croire la page sur parole.
/// `ecart_appel` est le rang de l'appel où tombe l'écart, parmi les appels du relevé.
/// `fautif` vaut `""` pour le vrai moteur, `"fausser-apres:<n>"` pour le moteur fautif.
#[wasm_bindgen]
pub fn rejouer(module: &[u8], soirees: &[u8], releve_wasmtime: &[u8], fautif: &str) -> JsValue {
    let apres = if fautif.is_empty() {
        None
    } else if let Some(n) = fautif.strip_prefix(FAUSSER_APRES) {
        match n.parse::<u64>() {
            Ok(n) => Some(n),
            Err(_) => return erreur(&format!("fautif illisible : {fautif}")),
        }
    } else {
        return erreur(&format!("fautif inconnu : {fautif}"));
    };
    let soirees = match releve::decoder_soirees(soirees) {
        Ok(s) => s,
        Err(e) => return erreur(&format!("soirées illisibles : {e}")),
    };
    let attendu = match releve::decoder_releve(releve_wasmtime) {
        Ok(r) => r,
        Err(e) => return erreur(&format!("relevé de wasmtime illisible : {e}")),
    };
    let attendus = attendu.evenements.len();
    let c = constater(module, &soirees, attendu, apres);
    let (appels, fautes) = releve::compter(&c.evenements);
    let liste = Array::new();
    for (sorte, n) in &fautes {
        let paire = Array::new();
        paire.push(&JsValue::from_str(sorte));
        paire.push(&JsValue::from(u32::try_from(*n).unwrap_or(u32::MAX)));
        liste.push(&paire);
    }
    let obtenu = Releve {
        chargement: c.chargement.clone(),
        evenements: c.evenements,
    };
    let empreinte = releve::hex(&releve::empreinte(&releve::encoder_releve(&obtenu)));
    let (ecart, ecart_indice, ecart_appel) = match &c.ecart {
        None => (JsValue::NULL, JsValue::NULL, JsValue::NULL),
        Some((k, texte)) => {
            let appel = obtenu
                .evenements
                .iter()
                .take(*k)
                .filter(|e| matches!(e, Evenement::Appel { .. }))
                .count();
            (JsValue::from_str(texte), nombre(*k), nombre(appel))
        }
    };
    let chargement = match &c.chargement {
        Chargement::Accepte { .. } => String::from("accepté"),
        Chargement::Refuse(r) => format!("refusé : {r}"),
    };
    objet(&[
        ("fautif", JsValue::from_str(fautif)),
        ("chargement", JsValue::from_str(&chargement)),
        ("attendus", nombre(attendus)),
        ("obtenus", nombre(obtenu.evenements.len())),
        (
            "appels",
            JsValue::from(u32::try_from(appels).unwrap_or(u32::MAX)),
        ),
        ("fautes", liste.into()),
        ("empreinte", JsValue::from_str(&empreinte)),
        ("ecart", ecart),
        ("ecart_indice", ecart_indice),
        ("ecart_appel", ecart_appel),
        ("chargement_ms", JsValue::from_f64(c.chargement_ms)),
        ("rejeu_ms", JsValue::from_f64(c.rejeu_ms)),
    ])
}
