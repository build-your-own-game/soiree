//! Ce que partagent les tests : des modules écrits en `wat`, la chaîne des passes,
//! et un petit exécuteur wasmtime dont on choisit les réglages.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use reecriture::{CARBURANT, Reglages, passes};
use wasmtime::{Config, Engine, Instance, Module, Store, Trap, Val};

/// Les réglages des tests : petits, pour que les fautes arrivent vite.
pub const REGLAGES: Reglages = Reglages {
    profondeur_max: 500,
    memoire_plafond_pages: 64,
};

pub fn wat(texte: &str) -> Vec<u8> {
    wat::parse_str(texte).unwrap_or_else(|e| panic!("wat invalide : {e}\n{texte}"))
}

/// Un module qui respecte § 3.1 et § 3.3, avec `extra` en plus (fonctions, globales,
/// données). La pile occupe la première page ; la boîte, le brouillon et la zone de
/// panique suivent.
pub fn module_abi(extra: &str) -> Vec<u8> {
    wat(&format!(
        r#"(module
  (memory (export "memory") 4 16)
  (global $__stack_pointer (mut i32) (i32.const 65536))
  (func (export "soiree_boite") (param i32 i32) (result i32) (i32.const 65536))
  (func (export "soiree_appel") (param i32) (result i64) (i64.const 0))
  {extra})"#
    ))
}

/// Une passe de code, pour la retirer de la chaîne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Passe {
    Globales,
    Carburant,
    Profondeur,
    Nan,
    Croissance,
    Pile,
}

/// Les passes de code dans l'ordre de `reecrire`, sans `sans`.
pub fn chaine(m: &[u8], sans: Option<Passe>) -> Vec<u8> {
    let garder = |p| sans != Some(p);
    // La passe de la pile ne vaut que pour un module de § 3.3 : `reecrire` y trouve
    // `__stack_pointer`.
    let pile = reecriture::reecrire(m, &REGLAGES)
        .ok()
        .and_then(|r| r.pile_globale);
    let mut m = m.to_vec();
    if garder(Passe::Globales) {
        m = passes::globales(&m).unwrap();
    }
    if garder(Passe::Carburant) {
        m = passes::carburant(&m).unwrap();
    }
    if garder(Passe::Profondeur) {
        m = passes::profondeur(&m, REGLAGES.profondeur_max).unwrap();
    }
    if garder(Passe::Nan) {
        m = passes::nan(&m).unwrap();
    }
    if garder(Passe::Croissance) {
        m = passes::croissance(&m, REGLAGES.memoire_plafond_pages).unwrap();
    }
    if garder(Passe::Pile) && pile.is_some() {
        m = passes::pile(&m, pile).unwrap();
    }
    wasmparser::validate(&m).unwrap();
    m
}

/// Deux réglages de moteur qui diffèrent là où la norme laisse le choix : la pile
/// native, et la canonisation des NaN par Cranelift.
#[derive(Debug, Clone, Copy)]
pub struct Reglage {
    pub pile: usize,
    pub nan_canoniques: bool,
}

pub const MOTEUR_A: Reglage = Reglage {
    pile: 256 * 1024,
    nan_canoniques: false,
};
pub const MOTEUR_B: Reglage = Reglage {
    pile: 1024 * 1024,
    nan_canoniques: true,
};

/// Ce qu'a donné un appel.
#[derive(Debug, Clone, PartialEq)]
pub enum Issue {
    Valeurs(Vec<Valeur>),
    Piege(Trap),
}

/// Une valeur comparable : les flottants par leurs bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Valeur {
    I32(i32),
    I64(i64),
    F32(u32),
    F64(u64),
}

impl Valeur {
    fn de(v: &Val) -> Valeur {
        match v {
            Val::I32(x) => Valeur::I32(*x),
            Val::I64(x) => Valeur::I64(*x),
            Val::F32(x) => Valeur::F32(*x),
            Val::F64(x) => Valeur::F64(*x),
            autre => panic!("valeur inattendue {autre:?}"),
        }
    }

    pub fn vers(self) -> Val {
        match self {
            Valeur::I32(x) => Val::I32(x),
            Valeur::I64(x) => Val::I64(x),
            Valeur::F32(x) => Val::F32(x),
            Valeur::F64(x) => Val::F64(x),
        }
    }

    /// Égales, à ceci près que deux NaN sont égaux quelle que soit leur charge.
    pub fn meme_sens(self, autre: Valeur) -> bool {
        match (self, autre) {
            (Valeur::F32(a), Valeur::F32(b)) => {
                a == b || (f32::from_bits(a).is_nan() && f32::from_bits(b).is_nan())
            }
            (Valeur::F64(a), Valeur::F64(b)) => {
                a == b || (f64::from_bits(a).is_nan() && f64::from_bits(b).is_nan())
            }
            (a, b) => a == b,
        }
    }
}

/// Une instance d'un module, sous un réglage, avec son garde-fou mural.
pub struct Execution {
    pub store: Store<()>,
    pub instance: Instance,
    arret: Arc<AtomicBool>,
}

impl Drop for Execution {
    fn drop(&mut self) {
        self.arret.store(true, Ordering::SeqCst);
    }
}

impl Execution {
    pub fn new(octets: &[u8], r: Reglage) -> Execution {
        let mut c = Config::new();
        c.max_wasm_stack(r.pile)
            .cranelift_nan_canonicalization(r.nan_canoniques)
            .epoch_interruption(true);
        let engine = Engine::new(&c).unwrap();
        let module = Module::new(&engine, octets).unwrap();
        let mut store = Store::new(&engine, ());
        store.set_epoch_deadline(1);
        let instance = Instance::new(&mut store, &module, &[]).unwrap();
        // Le garde-fou : une époque toutes les 300 ms. Un appel qui ne rend pas la main
        // est interrompu au plus tard 600 ms après son début.
        let arret = Arc::new(AtomicBool::new(false));
        let a = Arc::clone(&arret);
        thread::spawn(move || {
            while !a.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(300));
                engine.increment_epoch();
            }
        });
        let mut e = Execution {
            store,
            instance,
            arret,
        };
        if e.instance.get_global(&mut e.store, CARBURANT).is_some() {
            e.poser(CARBURANT, Val::I64(i64::MAX / 2));
        }
        e
    }

    pub fn appeler(&mut self, nom: &str, args: &[Valeur]) -> Issue {
        self.store.set_epoch_deadline(2);
        let f = self.instance.get_func(&mut self.store, nom).unwrap();
        let n = f.ty(&self.store).results().len();
        let mut sorties = vec![Val::I32(0); n];
        let args: Vec<Val> = args.iter().map(|a| a.vers()).collect();
        match f.call(&mut self.store, &args, &mut sorties) {
            Ok(()) => Issue::Valeurs(sorties.iter().map(Valeur::de).collect()),
            Err(e) => Issue::Piege(
                *e.downcast_ref::<Trap>()
                    .unwrap_or_else(|| panic!("erreur qui n'est pas un piège : {e}")),
            ),
        }
    }

    pub fn globale(&mut self, nom: &str) -> Valeur {
        let g = self.instance.get_global(&mut self.store, nom).unwrap();
        Valeur::de(&g.get(&mut self.store))
    }

    pub fn poser(&mut self, nom: &str, v: Val) {
        let g = self.instance.get_global(&mut self.store, nom).unwrap();
        g.set(&mut self.store, v).unwrap();
    }

    pub fn memoire(&mut self) -> Vec<u8> {
        let m = self.instance.get_memory(&mut self.store, "memory").unwrap();
        m.data(&self.store).to_vec()
    }
}
