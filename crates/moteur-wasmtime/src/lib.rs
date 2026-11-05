//! Le module réécrit dans wasmtime, comme `partie::Instance` (INTERFACE.md, § 3.4,
//! § 4.6 et § 8.7).
//!
//! [`Moteur::charger`] réécrit un module ([`reecriture::reecrire`]), le compile,
//! l'instancie une fois, appelle `Bonjour` et `Decrire`, et vérifie l'ABI, les zones
//! et la déclaration. Le [`ModuleCharge`] fabrique ensuite des [`InstanceWasm`].
//!
//! # Le moteur est réglé pour ne rien décider
//!
//! Cranelift ; fils, SIMD, SIMD relâché, plusieurs mémoires, mémoire 64 bits, appels
//! terminaux, exceptions et GC désactivés (les trois derniers ne sont pas compilés
//! dans cette version de wasmtime) ; ni carburant ni NaN canoniques de wasmtime : la
//! réécriture les porte, pour que le même module se comporte pareil dans tout moteur
//! (D32), et ses tests doivent pouvoir rougir. L'interruption par époque sert au seul
//! garde-fou mural.
//!
//! # Une faute, une panne
//!
//! | Constat après un piège | Issue |
//! |---|---|
//! | l'époque du garde-fou est passée | panne de l'outil (`GardeFouMural`) |
//! | `soiree_croissance_refusee` posé | panne de la machine (`CroissanceRefusee`) |
//! | `soiree_carburant` négatif | [`partie::Faute::Budget`] |
//! | `soiree_profondeur` au-delà du plafond | [`partie::Faute::Profondeur`] |
//! | `soiree_croissance_bornee` posé | [`partie::Faute::Memoire`] |
//! | un message dans la zone de panique | [`partie::Faute::Panique`] |
//! | accès hors de la mémoire | [`partie::Faute::HorsMemoire`] |
//! | pile native épuisée | panne de l'outil : la profondeur devait piquer avant |
//! | un autre piège | [`partie::Faute::Piege`], nommé par wasmtime |
//!
//! Après un appel réussi : la pile non rendue est une panne de l'outil
//! (`PileNonRendue`), une sortie hors du brouillon une [`partie::Faute::SortieInvalide`].
//!
//! # La zone de panique
//!
//! Sa forme est celle de `jeu-abi` ([`jeu_abi::lire_panique`]), que le gestionnaire
//! de panique de `jeu::module!` écrit : à l'adresse de la zone, [`PANIQUE_CODE`] en
//! `u32` petit-boutiste, puis la longueur du message en `u32` petit-boutiste, puis
//! le message en UTF-8, coupé à la capacité de la zone. Le moteur remet la zone
//! entière à zéro avant chaque appel, avec la pile, la boîte et le brouillon : les
//! quatre zones que l'empreinte exclut ne portent rien d'un appel au suivant.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod horloge;
mod instance;
mod interieur;

use std::fmt;
use std::sync::Arc;

use jeu_abi::{
    ABI, Appel, ApplicableFil, Bonjour, DeclarationFil, PauseFil, Retour, SorteAppel, decoder,
    encoder, raison_suffisante,
};
use partie::{CausePanne, Echec, Faute, PanneOutil};
use reecriture::{Reecrit, Refus};
use wasmtime::{Config, Engine, Module, Strategy};

pub use horloge::TIC_MS;
pub use instance::{FabriqueWasm, InstanceWasm};
pub use interieur::InstantaneWasm;

use crate::horloge::Horloge;
use crate::instance::{Commun, Sonde};
use crate::interieur::{Base, Zones};

pub use jeu_abi::{PANIQUE_CODE, PANIQUE_ENTETE};

/// Le carburant de `Bonjour` et de `Decrire` au chargement : de quoi décrire un gros
/// jeu, sans laisser une boucle infinie au seul garde-fou. Point de départ, non
/// mesuré.
pub const BUDGET_DE_CHARGEMENT: u64 = 1 << 32;

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

/// Pourquoi un module n'est pas chargé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusModule {
    /// La réécriture le refuse (§ 3.1, § 3.3).
    Reecriture(Refus),
    /// Il parle une autre enveloppe : son numéro, et la plage du moteur.
    Abi {
        /// L'ABI du module.
        module: u32,
        /// La plus ancienne et la plus récente que le moteur parle.
        moteur: (u32, u32),
    },
    /// Sa déclaration ou ses zones ne tiennent pas.
    Declaration(String),
    /// `Bonjour` ou `Decrire` a fauté.
    Faute {
        /// L'appel.
        appel: &'static str,
        /// La faute.
        faute: Faute,
    },
    /// Le moteur ou la machine a échoué.
    Panne(PanneOutil),
}

impl fmt::Display for RefusModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefusModule::Reecriture(r) => write!(f, "{r}"),
            RefusModule::Abi { module, moteur } => write!(
                f,
                "le module parle l'ABI {module}, le moteur de {} à {}",
                moteur.0, moteur.1
            ),
            RefusModule::Declaration(d) => write!(f, "déclaration refusée : {d}"),
            RefusModule::Faute { appel, faute } => write!(f, "{appel} : {faute}"),
            RefusModule::Panne(p) => write!(f, "{p}"),
        }
    }
}

impl std::error::Error for RefusModule {}

impl From<PanneOutil> for RefusModule {
    fn from(p: PanneOutil) -> RefusModule {
        RefusModule::Panne(p)
    }
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
        let reecrit =
            reecriture::reecrire(original, &self.reglages.reecriture).map_err(|r| match r {
                Refus::Reecriture { quoi } => {
                    RefusModule::Panne(interne(format!("réécriture : {quoi}")))
                }
                autre @ (Refus::Illisible { .. }
                | Refus::Import { .. }
                | Refus::Export { .. }
                | Refus::ExportManquant { .. }
                | Refus::Fonctionnalite { .. }
                | Refus::Instruction { .. }
                | Refus::Depart
                | Refus::Memoire(_)
                | Refus::PileNonEnBas) => RefusModule::Reecriture(autre),
            })?;
        let module = Module::new(&self.engine, &reecrit.octets)
            .map_err(|e| interne(format!("wasmtime refuse le module réécrit : {e}")))?;
        let globales = globales_mutables(&module)?;
        let base = Arc::new(Base {
            module,
            empreinte: reecrit.empreinte,
            pile_initiale: reecrit.pile_initiale,
            pile_globale: reecrit.pile_globale,
            globales,
            profondeur_max: self.reglages.reecriture.profondeur_max,
            garde_fou_tics: horloge::tics(self.reglages.garde_fou_murale_ms),
            pile_fil: self.pile_wasm.saturating_add(2 * 1024 * 1024),
        });
        let (min, max) = reecrit.constat.memoire;
        let max_permis = max
            .unwrap_or(u32::MAX)
            .min(self.reglages.reecriture.memoire_plafond_pages);
        let sonde = Sonde::new(Arc::clone(&base), max_permis)?;

        // Bonjour, une première fois sans zones connues, puis avec elles : la seconde
        // vérifie que la sortie est bien dans le brouillon.
        let bonjour = lire_bonjour(&sonde, None)?;
        if bonjour.abi != ABI {
            return Err(RefusModule::Abi {
                module: bonjour.abi,
                moteur: (ABI, ABI),
            });
        }
        let pages = sonde.pages()?;
        let zones = zones(&bonjour, reecrit.pile_initiale, pages)?;
        if lire_bonjour(&sonde, Some(zones))? != bonjour {
            return Err(RefusModule::Declaration(String::from(
                "Bonjour ne rend pas deux fois la même chose",
            )));
        }
        let declaration = lire_declaration(&sonde, zones)?;
        let memoire_max = accord(&declaration, &bonjour, min, max_permis)?;
        Ok(ModuleCharge {
            reecrit: Arc::new(reecrit),
            bonjour,
            declaration: Arc::new(declaration),
            commun: Arc::new(Commun {
                base,
                zones,
                memoire_max,
                _horloge: Arc::clone(&self.horloge),
            }),
        })
    }
}

/// Les indices des globales mutables d'origine, lus dans les exports du module
/// réécrit.
fn globales_mutables(m: &Module) -> Result<Vec<u32>, PanneOutil> {
    let mut indices = Vec::new();
    for e in m.exports() {
        if let Some(n) = e.name().strip_prefix(reecriture::PREFIXE_GLOBALE) {
            indices.push(n.parse().map_err(|_| interne("export de globale"))?);
        }
    }
    indices.sort_unstable();
    Ok(indices)
}

fn appel_encode(a: &Appel<'_>) -> Result<Vec<u8>, PanneOutil> {
    let mut v = Vec::new();
    encoder(a, &mut v).map_err(|e| interne(e.to_string()))?;
    Ok(v)
}

fn refus_d_appel(appel: &'static str, e: Echec) -> RefusModule {
    match e {
        Echec::Faute(faute) => RefusModule::Faute { appel, faute },
        Echec::Panne(p) => RefusModule::Panne(p),
    }
}

fn lire_bonjour(sonde: &Sonde, zones: Option<Zones>) -> Result<Bonjour, RefusModule> {
    let octets = sonde
        .appeler(
            zones,
            SorteAppel::Lecture,
            appel_encode(&Appel::Bonjour)?,
            BUDGET_DE_CHARGEMENT,
        )
        .map_err(|e| refus_d_appel("Bonjour", e))?;
    match decoder::<Retour<'_>>(&octets) {
        Ok(Retour::Bonjour(b)) => Ok(b),
        Ok(Retour::Illisible(e)) => Err(RefusModule::Panne(PanneOutil::new(
            CausePanne::AppelIllisible,
            format!("Bonjour : {e}"),
        ))),
        Ok(_) | Err(_) => Err(RefusModule::Faute {
            appel: "Bonjour",
            faute: Faute::SortieInvalide,
        }),
    }
}

fn lire_declaration(sonde: &Sonde, zones: Zones) -> Result<DeclarationFil, RefusModule> {
    let octets = sonde
        .appeler(
            Some(zones),
            SorteAppel::Lecture,
            appel_encode(&Appel::Decrire)?,
            BUDGET_DE_CHARGEMENT,
        )
        .map_err(|e| refus_d_appel("Decrire", e))?;
    match decoder::<Retour<'_>>(&octets) {
        Ok(Retour::Decrit(d)) => Ok(*d),
        Ok(Retour::Illisible(e)) => Err(RefusModule::Panne(PanneOutil::new(
            CausePanne::AppelIllisible,
            format!("Decrire : {e}"),
        ))),
        Ok(_) | Err(_) => Err(RefusModule::Faute {
            appel: "Decrire",
            faute: Faute::SortieInvalide,
        }),
    }
}

/// Les zones de `Bonjour` : dans la mémoire minimale, au-dessus de la pile, sans se
/// chevaucher, la zone de panique assez grande pour son entête (§ 3.2).
fn zones(b: &Bonjour, pile: u32, pages: u64) -> Result<Zones, RefusModule> {
    let octets = pages.saturating_mul(65_536);
    let z = Zones {
        boite: b.boite,
        brouillon: b.brouillon,
        panique: b.panique,
    };
    let toutes = [
        ("boîte", z.boite),
        ("brouillon", z.brouillon),
        ("panique", z.panique),
    ];
    for (nom, (a, n)) in toutes {
        let fin = u64::from(a).saturating_add(u64::from(n));
        if n == 0 || a < pile || fin > octets {
            return Err(RefusModule::Declaration(format!(
                "zone {nom} [{a}, {fin}) hors de [{pile}, {octets})"
            )));
        }
    }
    for (i, (na, (a, n))) in toutes.iter().enumerate() {
        for (nb, (c, m)) in toutes.iter().skip(i.saturating_add(1)) {
            let fa = u64::from(*a).saturating_add(u64::from(*n));
            let fc = u64::from(*c).saturating_add(u64::from(*m));
            if u64::from(*a) < fc && u64::from(*c) < fa {
                return Err(RefusModule::Declaration(format!(
                    "les zones {na} et {nb} se chevauchent"
                )));
            }
        }
    }
    if z.panique.1 <= PANIQUE_ENTETE {
        return Err(RefusModule::Declaration(String::from(
            "zone de panique plus petite que son entête",
        )));
    }
    Ok(z)
}

/// Ce que le moteur vérifie de la déclaration au chargement (§ 2.12) : ce que
/// `jeu::module!` ne peut pas vérifier en `const`, ou qu'un module écrit sans `jeu`
/// pourrait mentir. Rend `soiree_memoire_max`.
fn accord(
    d: &DeclarationFil,
    b: &Bonjour,
    pages_min: u32,
    max_permis: u32,
) -> Result<u32, RefusModule> {
    let refus = |quoi: String| Err(RefusModule::Declaration(quoi));
    let raisons = [
        ("fenetre", sans_objet(&d.fenetre)),
        ("telephones", sans_objet(&d.telephones)),
        ("manettes", sans_objet(&d.manettes)),
        ("prediction", sans_objet(&d.prediction)),
        ("hasard", sans_objet(&d.hasard)),
        ("secrets", sans_objet(&d.secrets)),
        ("fin", sans_objet(&d.fin)),
        (
            "pause",
            match &d.pause {
                PauseFil::Aucune(r) => Some(r.as_str()),
                PauseFil::DuJeu | PauseFil::DuCommun => None,
            },
        ),
    ];
    if let ApplicableFil::Oui(t) = &d.telephones {
        for (nom, r) in [
            ("telephones.tenu.fraicheur", sans_objet(&t.tenu.fraicheur)),
            ("telephones.tenu.age_max", sans_objet(&t.tenu.age_max)),
        ] {
            if let Some(r) = r
                && !raison_suffisante(r)
            {
                return refus(format!("{nom} : raison creuse « {r} »"));
            }
        }
    }
    for (nom, r) in raisons {
        if let Some(r) = r
            && !raison_suffisante(r)
        {
            return refus(format!("{nom} : raison creuse « {r} » (HIS-04)"));
        }
    }
    if matches!(d.prediction, ApplicableFil::Oui(()))
        && matches!(d.telephones, ApplicableFil::SansObjet(_))
    {
        return refus(String::from("prediction sans téléphones"));
    }
    if d.limites.brouillon != b.brouillon.1 {
        return refus(format!(
            "limites.brouillon vaut {}, la zone de brouillon {}",
            d.limites.brouillon, b.brouillon.1
        ));
    }
    if d.limites.memoire < pages_min {
        return refus(format!(
            "limites.memoire ({} pages) sous la mémoire minimale du module ({pages_min})",
            d.limites.memoire
        ));
    }
    Ok(d.limites.memoire.min(max_permis))
}

fn sans_objet<T>(a: &ApplicableFil<T>) -> Option<&str> {
    match a {
        ApplicableFil::Oui(_) => None,
        ApplicableFil::SansObjet(r) => Some(r.as_str()),
    }
}

/// Un module chargé : réécrit, compilé, son `Bonjour` et sa déclaration vérifiés.
#[derive(Clone)]
pub struct ModuleCharge {
    reecrit: Arc<Reecrit>,
    bonjour: Bonjour,
    declaration: Arc<DeclarationFil>,
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
        self.commun.memoire_max
    }

    /// Les octets que le moteur remet à zéro avant chaque appel : la pile, la boîte,
    /// le brouillon et la zone de panique (§ 3.2). Ce travail n'est pas compté dans le
    /// carburant, qui ne compte que les instructions du module ; il est le même à
    /// chaque appel, et l'épreuve le compte à part dans ce que juge E19.
    pub fn octets_remis_a_zero(&self) -> u64 {
        crate::interieur::remises_a_zero(&self.commun.base, self.commun.zones)
            .iter()
            .fold(0u64, |a, (_, n)| a.saturating_add(u64::from(*n)))
    }

    /// Ce qui fabrique ses instances.
    pub fn fabrique(&self) -> FabriqueWasm {
        FabriqueWasm {
            commun: Arc::clone(&self.commun),
            machine: None,
        }
    }
}
