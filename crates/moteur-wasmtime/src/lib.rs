//! Le module réécrit dans wasmtime, comme `partie::Instance` (INTERFACE.md, § 3.4,
//! § 4.6 et § 8.7) : les primitives de wasmtime sous le protocole du commun
//! (`moteur-commun`, PHASE-3, § 2.3, P2).
//!
//! [`Moteur::charger`] réécrit un module ([`reecriture::reecrire`]), le compile,
//! l'instancie une fois et le fait vérifier par [`moteur_commun::verifier_chargement`]
//! (`Bonjour`, `Decrire`, l'ABI, les zones et la déclaration). Le [`ModuleCharge`]
//! fabrique ensuite des [`InstanceWasm`].
//!
//! Ce qui est ici : l'`impl Machine` sur wasmtime (lire, écrire et remettre à zéro la
//! mémoire, la faire grandir, lire et poser une globale, appeler, rendre un piège brut),
//! l'époque du garde-fou mural, et le fil de chaque instance. Le déroulé d'un appel, le
//! classement d'un piège en faute ou en panne, l'instantané hors des quatre zones et
//! l'empreinte sont dans `moteur-commun`, une seule fois pour tous les moteurs ; la table
//! « une faute, une panne » y est.
//!
//! # Le moteur est réglé pour ne rien décider
//!
//! Cranelift ; fils, SIMD, SIMD relâché, plusieurs mémoires, mémoire 64 bits, appels
//! terminaux, exceptions et GC désactivés (les trois derniers ne sont pas compilés
//! dans cette version de wasmtime) ; ni carburant ni NaN canoniques de wasmtime : la
//! réécriture les porte, pour que le même module se comporte pareil dans tout moteur
//! (D32), et ses tests doivent pouvoir rougir. L'interruption par époque sert au seul
//! garde-fou mural : son échéance se pose une fois par appel, avant `soiree_boite`, et
//! borne la boîte et `soiree_appel` ensemble à [`ReglagesMoteur::garde_fou_murale_ms`]
//! (INTERFACE, § 4.6).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod horloge;
mod instance;
mod machine;

use std::sync::Arc;

use jeu_abi::{Bonjour, DeclarationFil};
use moteur_commun::{Carte, Plan, globales_d_origine};
use partie::{CausePanne, PanneOutil};
use reecriture::Reecrit;
use wasmtime::{Config, Engine, Module, Strategy};

pub use horloge::TIC_MS;
pub use instance::{FabriqueWasm, InstanceWasm};
pub use moteur_commun::{BUDGET_DE_CHARGEMENT, RefusModule};

/// L'instantané d'une instance : celui du commun, la mémoire hors des quatre zones
/// (PHASE-3, P3).
pub type InstantaneWasm = moteur_commun::Instantane;

use crate::horloge::Horloge;
use crate::instance::Commun;
use crate::machine::Base;

pub use jeu_abi::{PANIQUE_CODE, PANIQUE_ENTETE};

/// La pile native accordée par étage de profondeur permis, en octets : celle que
/// la réécriture compte pour un étage ([`reecriture::passes::OCTETS_PAR_ETAGE`]),
/// qui donne plusieurs étages à une fonction au cadre épais. Une seule définition :
/// les deux s'accordent par le compilateur.
pub const PILE_PAR_APPEL: usize = reecriture::passes::OCTETS_PAR_ETAGE as usize;

/// Les réglages du moteur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReglagesMoteur {
    /// Les réglages de la réécriture.
    pub reecriture: reecriture::Reglages,
    /// La durée murale au-delà de laquelle un appel est arrêté, en millisecondes :
    /// une panne de l'outil (§ 4.6).
    pub garde_fou_murale_ms: u32,
}

impl ReglagesMoteur {
    /// Des points de départ pour la phase 1, non mesurés : ceux de la réécriture, et
    /// un garde-fou d'une seconde.
    pub const PHASE_1: ReglagesMoteur = ReglagesMoteur {
        reecriture: reecriture::Reglages::PHASE_1,
        garde_fou_murale_ms: 1000,
    };
}

/// Le moteur : wasmtime réglé pour la partie, et son garde-fou.
pub struct Moteur {
    engine: Engine,
    horloge: Arc<Horloge>,
    reglages: ReglagesMoteur,
    pile_wasm: usize,
}

fn interne(quoi: impl Into<String>) -> PanneOutil {
    PanneOutil::new(CausePanne::Interne, quoi)
}

impl Moteur {
    /// Un moteur réglé (§ 8.7), et son garde-fou.
    pub fn new(r: ReglagesMoteur) -> Result<Moteur, PanneOutil> {
        let pile_wasm = usize::try_from(r.reecriture.profondeur_max)
            .unwrap_or(usize::MAX)
            .saturating_add(16)
            .saturating_mul(PILE_PAR_APPEL)
            .max(512 * 1024);
        let mut c = Config::new();
        c.strategy(Strategy::Cranelift)
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
            .max_wasm_stack(pile_wasm)
            // Aucun appel n'est asynchrone ; wasmtime exige seulement que cette pile
            // ne soit pas plus petite que la précédente.
            .async_stack_size(pile_wasm);
        let engine = Engine::new(&c).map_err(|e| interne(format!("wasmtime : {e}")))?;
        let horloge = Arc::new(Horloge::lancer(engine.clone())?);
        Ok(Moteur {
            engine,
            horloge,
            reglages: r,
            pile_wasm,
        })
    }

    /// Réécrit, compile, instancie une fois, appelle `Bonjour` et `Decrire`, vérifie
    /// l'ABI, les zones et la déclaration, puis fixe `soiree_memoire_max` à
    /// `limites.memoire` pour chaque instance.
    pub fn charger(&self, original: &[u8]) -> Result<ModuleCharge, RefusModule> {
        let reecrit = reecriture::reecrire(original, &self.reglages.reecriture)?;
        let module = Module::new(&self.engine, &reecrit.octets)
            .map_err(|e| interne(format!("wasmtime refuse le module réécrit : {e}")))?;
        let globales = globales_d_origine(module.exports().map(|e| e.name()))?;
        let plan = Plan::new(&reecrit, &self.reglages.reecriture, &globales);
        let base = Arc::new(Base {
            module,
            garde_fou_tics: horloge::tics(self.reglages.garde_fou_murale_ms),
            pile_fil: self.pile_wasm.saturating_add(2 * 1024 * 1024),
        });
        let charge = instance::sonder(&base, plan)?;
        Ok(ModuleCharge {
            reecrit: Arc::new(reecrit),
            bonjour: charge.bonjour,
            declaration: Arc::new(charge.declaration),
            carburant_du_chargement: charge.carburant,
            commun: Arc::new(Commun {
                base,
                carte: Arc::new(charge.carte),
                _horloge: Arc::clone(&self.horloge),
            }),
        })
    }
}

/// Un module chargé : réécrit, compilé, son `Bonjour` et sa déclaration vérifiés.
#[derive(Clone)]
pub struct ModuleCharge {
    reecrit: Arc<Reecrit>,
    bonjour: Bonjour,
    declaration: Arc<DeclarationFil>,
    carburant_du_chargement: (u64, u64),
    commun: Arc<Commun>,
}

impl ModuleCharge {
    /// Ce que le module peut faire, constaté sur ses octets (§ 3.6).
    pub fn constat(&self) -> &reecriture::Constat {
        &self.reecrit.constat
    }

    /// L'empreinte du module livré, avant réécriture : la version du jeu (§ 3.8).
    #[expect(
        clippy::misnamed_getters,
        reason = "l'empreinte d'un module est celle des octets livrés (§ 3.8) ; celle du réécrit est dans reecrit()"
    )]
    pub fn empreinte(&self) -> [u8; 32] {
        self.reecrit.empreinte_originale
    }

    /// Le module réécrit.
    pub fn reecrit(&self) -> &Reecrit {
        &self.reecrit
    }

    /// Ce que le module a dit de lui-même.
    pub fn bonjour(&self) -> &Bonjour {
        &self.bonjour
    }

    /// Sa déclaration.
    pub fn declaration(&self) -> &DeclarationFil {
        &self.declaration
    }

    /// `soiree_memoire_max` de chaque instance, en pages.
    pub fn memoire_max(&self) -> u32 {
        self.commun.carte.memoire_max()
    }

    /// Les octets que le moteur remet à zéro avant chaque appel : la pile, la boîte,
    /// le brouillon et la zone de panique (§ 3.2). Ce travail n'est pas compté dans le
    /// carburant, qui ne compte que les instructions du module ; il est le même à
    /// chaque appel, et l'épreuve le compte à part dans ce que juge E19.
    pub fn octets_remis_a_zero(&self) -> u64 {
        self.commun.carte.octets_remis_a_zero()
    }

    /// Ce que ses instances partagent : ses zones, `soiree_memoire_max`, la mémoire
    /// qu'un instantané garde ([`Carte::plages`]).
    pub fn carte(&self) -> &Carte {
        &self.commun.carte
    }

    /// Le carburant du chargement : celui du premier `Bonjour`, et celui de `Decrire`,
    /// chacun sous [`BUDGET_DE_CHARGEMENT`] (PHASE-3, P5).
    pub fn carburant_du_chargement(&self) -> (u64, u64) {
        self.carburant_du_chargement
    }

    /// Ce qui fabrique ses instances.
    pub fn fabrique(&self) -> FabriqueWasm {
        FabriqueWasm {
            commun: Arc::clone(&self.commun),
            machine: None,
        }
    }
}
