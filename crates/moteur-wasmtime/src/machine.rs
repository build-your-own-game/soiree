//! Les primitives de wasmtime sur une instance du module réécrit
//! ([`moteur_commun::Machine`]) : le magasin, l'instance, ses exports, et l'échéance du
//! garde-fou mural. Le protocole d'un appel est dans `moteur-commun`.

use std::sync::Arc;

use moteur_commun::{Bits, Machine, Piege};
use partie::{CausePanne, PanneOutil};
use wasmtime::{
    Global, Instance, Memory, Module, Store, StoreLimits, StoreLimitsBuilder, Trap, TypedFunc, Val,
};

/// Ce que toutes les instances wasmtime d'un module partagent.
pub(crate) struct Base {
    pub(crate) module: Module,
    /// L'échéance de chaque appel, en tics de l'époque.
    pub(crate) garde_fou_tics: u64,
    /// La pile du fil d'une instance, en octets.
    pub(crate) pile_fil: usize,
}

pub(crate) struct Donnees {
    limites: StoreLimits,
}

/// Une instance wasmtime du module réécrit, sur son fil.
pub(crate) struct MachineWasm {
    base: Arc<Base>,
    /// La mémoire que la machine accorde, en octets (`simuler_une_machine_limitee`).
    limite: Option<usize>,
    store: Store<Donnees>,
    memoire: Memory,
    boite: TypedFunc<(i32, i32), i32>,
    appel: TypedFunc<i32, i64>,
    /// Les globales exportées, par leur nom : celles du moteur et celles d'origine.
    globales: Vec<(String, Global)>,
}

fn interne(quoi: impl Into<String>) -> PanneOutil {
    PanneOutil::new(CausePanne::Interne, quoi)
}

impl MachineWasm {
    /// Une instance neuve : rien n'y a été appelé.
    pub(crate) fn new(base: &Arc<Base>, limite: Option<usize>) -> Result<MachineWasm, PanneOutil> {
        let mut limites = StoreLimitsBuilder::new();
        if let Some(octets) = limite {
            limites = limites.memory_size(octets);
        }
        let mut store = Store::new(
            base.module.engine(),
            Donnees {
                limites: limites.build(),
            },
        );
        store.limiter(|d| &mut d.limites);
        store.set_epoch_deadline(base.garde_fou_tics);
        let instance = Instance::new(&mut store, &base.module, &[])
            .map_err(|e| PanneOutil::new(CausePanne::Instanciation, e.to_string()))?;
        let mut globales = Vec::new();
        for e in base.module.exports() {
            if e.ty().global().is_some() {
                let g = instance.get_global(&mut store, e.name()).ok_or_else(|| {
                    interne(format!("export {} absent du module réécrit", e.name()))
                })?;
                globales.push((String::from(e.name()), g));
            }
        }
        let memoire = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| interne("mémoire absente du module réécrit"))?;
        let boite = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "soiree_boite")
            .map_err(|e| interne(e.to_string()))?;
        let appel = instance
            .get_typed_func::<i32, i64>(&mut store, "soiree_appel")
            .map_err(|e| interne(e.to_string()))?;
        Ok(MachineWasm {
            base: Arc::clone(base),
            limite,
            store,
            memoire,
            boite,
            appel,
            globales,
        })
    }

    fn global(&self, nom: &str) -> Result<Global, PanneOutil> {
        self.globales
            .iter()
            .find(|(n, _)| n == nom)
            .map(|(_, g)| *g)
            .ok_or_else(|| interne(format!("export {nom} absent du module réécrit")))
    }

    /// `[adresse, adresse + n)` dans la mémoire, en indices.
    fn bornes(&self, adresse: u32, n: usize) -> Result<std::ops::Range<usize>, PanneOutil> {
        let debut = usize::try_from(adresse).map_err(|_| interne("adresse hors de la mémoire"))?;
        let fin = debut
            .checked_add(n)
            .ok_or_else(|| interne("adresse hors de la mémoire"))?;
        if fin > self.memoire.data_size(&self.store) {
            return Err(interne("lecture ou écriture hors de la mémoire"));
        }
        Ok(debut..fin)
    }

    /// L'échéance du garde-fou mural, posée une fois par appel du protocole, avant
    /// `soiree_boite` : elle borne la boîte et l'appel ensemble (INTERFACE, § 4.6).
    fn echeance(&mut self) {
        self.store.set_epoch_deadline(self.base.garde_fou_tics);
    }
}

/// Un piège de wasmtime, brut : le protocole le classe ensuite.
fn piege(e: &wasmtime::Error) -> Piege {
    let Some(t) = e.downcast_ref::<Trap>().copied() else {
        return Piege::Moteur(format!("erreur de wasmtime : {e}"));
    };
    if t == Trap::Interrupt {
        Piege::Interrompu
    } else if t == Trap::MemoryOutOfBounds {
        Piege::HorsMemoire
    } else if t == Trap::StackOverflow {
        Piege::PileNative
    } else {
        Piege::Autre(t.to_string())
    }
}

fn bits(v: &Val) -> Option<Bits> {
    match v {
        Val::I32(x) => Some(Bits::I32(*x)),
        Val::I64(x) => Some(Bits::I64(*x)),
        Val::F32(x) => Some(Bits::F32(*x)),
        Val::F64(x) => Some(Bits::F64(*x)),
        Val::V128(_)
        | Val::FuncRef(_)
        | Val::ExternRef(_)
        | Val::AnyRef(_)
        | Val::ExnRef(_)
        | Val::ContRef(_) => None,
    }
}

const fn val(b: Bits) -> Val {
    match b {
        Bits::I32(x) => Val::I32(x),
        Bits::I64(x) => Val::I64(x),
        Bits::F32(x) => Val::F32(x),
        Bits::F64(x) => Val::F64(x),
    }
}

impl Machine for MachineWasm {
    fn neuve(&mut self) -> Result<MachineWasm, PanneOutil> {
        MachineWasm::new(&self.base, self.limite)
    }

    fn pages(&mut self) -> u64 {
        self.memoire.size(&self.store)
    }

    fn lire(&mut self, adresse: u32, dans: &mut [u8]) -> Result<(), PanneOutil> {
        let r = self.bornes(adresse, dans.len())?;
        let source = self
            .memoire
            .data(&self.store)
            .get(r)
            .ok_or_else(|| interne("lecture hors de la mémoire"))?;
        dans.copy_from_slice(source);
        Ok(())
    }

    fn ecrire(&mut self, adresse: u32, octets: &[u8]) -> Result<(), PanneOutil> {
        let r = self.bornes(adresse, octets.len())?;
        self.memoire
            .data_mut(&mut self.store)
            .get_mut(r)
            .ok_or_else(|| interne("écriture hors de la mémoire"))?
            .copy_from_slice(octets);
        Ok(())
    }

    fn remplir_de_zeros(&mut self, adresse: u32, longueur: u32) -> Result<(), PanneOutil> {
        let n = usize::try_from(longueur).map_err(|_| interne("zone hors de la mémoire"))?;
        let r = self.bornes(adresse, n)?;
        self.memoire
            .data_mut(&mut self.store)
            .get_mut(r)
            .ok_or_else(|| interne("zone hors de la mémoire"))?
            .fill(0);
        Ok(())
    }

    fn grandir_jusqu_a(&mut self, pages: u64) -> Result<(), PanneOutil> {
        let avant = self.memoire.size(&self.store);
        self.memoire
            .grow(&mut self.store, pages.saturating_sub(avant))
            .map(|_| ())
            .map_err(|e| {
                PanneOutil::new(
                    CausePanne::CroissanceRefusee,
                    format!("restaurer : la machine refuse la mémoire ({e})"),
                )
            })
    }

    fn globale(&mut self, nom: &str) -> Result<Bits, PanneOutil> {
        let g = self.global(nom)?;
        bits(&g.get(&mut self.store)).ok_or_else(|| interne("globale d'un type inattendu"))
    }

    fn poser_globale(&mut self, nom: &str, b: Bits) -> Result<(), PanneOutil> {
        let g = self.global(nom)?;
        g.set(&mut self.store, val(b))
            .map_err(|e| interne(e.to_string()))
    }

    fn boite(&mut self, taille: i32, sorte: i32) -> Result<i32, Piege> {
        self.echeance();
        self.boite
            .call(&mut self.store, (taille, sorte))
            .map_err(|e| piege(&e))
    }

    /// L'échéance posée par [`Machine::boite`] court encore : elle n'est pas reposée.
    fn appel(&mut self, taille: i32) -> Result<i64, Piege> {
        self.appel
            .call(&mut self.store, taille)
            .map_err(|e| piege(&e))
    }
}
