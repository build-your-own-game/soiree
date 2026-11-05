//! Le relevé des appels d'une instance, que E20 compare d'un moteur à l'autre (PHASE-3,
//! § 8.2 et § 8.3, lot 3b).
//!
//! E20, « Deux moteurs », reprend les journaux de l'épreuve depuis la création, une
//! fois dans wasmtime et une fois dans chaque navigateur, par la même procédure
//! ([`rejouer`]) sur une fabrique dont chaque opération s'écrit ici ([`Enregistreuse`]).
//! La reprise refait aussi chaque entrée écartée
//! ([`Partie::reprendre_en_refaisant_les_fautes`]) : sans elle, une faute ne se
//! rejouerait jamais, et ne se comparerait pas.
//! Les deux relevés doivent être égaux, événement pour événement : le carburant,
//! l'issue et le retour de chaque appel (faute classée comprise), chaque instantané et
//! chaque restauration, chaque empreinte, et la forme canonique finale de chaque soirée
//! (les appels `Canonique` de la fin). La procédure est écrite une fois, ici, et tourne
//! dans les deux moteurs : seul le moteur change.
//!
//! Les journaux des modules d'aujourd'hui ne font refaire que des fautes `Panique` :
//! avant les soirées, la procédure provoque donc deux fautes `Budget` sur chaque module
//! (`Decrire` à la moitié de son carburant, puis à une unité près), pour que cette
//! sorte-là aussi se compare d'un moteur à l'autre. Les autres (`Profondeur`,
//! `HorsMemoire`, `Memoire`, `Piege`) ne se provoquent pas sur un module compilé par
//! Rust sûr : les essais de `Machine` les tiennent, dans chaque navigateur (lot 3a).
//!
//! Le journal ne note pas le carburant de chaque appel : c'est le relevé qui le donne.
//! Il reprend ce que faisait la capture du harnais de mesure
//! (`mesures/navigateur/capture`, `Enregistree` et `Enregistreuse`), sur le protocole du
//! commun au lieu d'un protocole recopié.
//!
//! # Comparer en chemin
//!
//! Une [`Enregistreuse`] peut recevoir le relevé attendu ([`Enregistreuse::comparant`]) :
//! chaque événement s'y compare dès qu'il arrive, et la première différence
//! ([`Ecart`]) arrête le rejeu (chaque opération suivante rend une panne). Un rejeu qui
//! diverge coûte alors ce qui précède la divergence, pas toute la soirée : c'est ce qui
//! rend bon marché le contrôle négatif joué à chaque E20 (le moteur fautif).
//!
//! # Les formes
//!
//! Petit-boutistes, relues strictement : un octet de trop, un message coupé, une
//! valeur hors de son domaine sont refusés.
//!
//! ```text
//! les soirées : "SOIRJRN1", u32 nombre, puis pour chacune :
//!               texte nom, octets ouverture (garde), octets journal (garde)
//! le relevé :   "SOIRREL1", chargement, u32 nombre, puis chaque événement
//!   chargement : 0 accepté (32 empreinte réécrite, u64 carburant de Bonjour,
//!                u64 carburant de Decrire) | 1 refusé (texte)
//!   événement :  1 Soiree (u32) | 2 Neuve (u32) | 3 Appel | 4 Instantane (u32, texte?)
//!                | 5 Restaurer (u32, texte?) | 6 Empreinte (u32, 0 + 32 octets | 1 texte)
//!                | 7 Fin (texte)
//!   Appel :      u32 instance, texte nom, u64 budget, 8 empreinte de l'appel,
//!                entrée? (u64 ordre, u64 place), carburant? (u64), issue
//!   issue :      0 rendu (u32 longueur, 8 empreinte) | 1 faute (octets postcard)
//!                | 2 panne (texte)
//!   texte :      u32 longueur, UTF-8 ; octets : u32 longueur, octets ;
//!   X? :         0 absent | 1 présent, X
//! ```
//!
//! Les empreintes de 8 octets sont les premiers octets du BLAKE3 de l'appel ou du
//! retour : elles disent qu'un retour diffère, pas comment.

use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::fmt;

use jeu_abi::{Appel, SorteAppel, decoder, encoder};
use partie::persistance::{decoder_journal, decoder_ouverture, encoder_journal, encoder_ouverture};
use partie::{
    CausePanne, Echec, Fabrique, Faute, Instance, Journal, Ouverture, PanneOutil, Partie,
    RefusOuverture, RefusReprise,
};

use crate::RefusModule;

/// L'en-tête des soirées.
const ENTETE_SOIREES: &[u8; 8] = b"SOIRJRN1";
/// L'en-tête du relevé.
const ENTETE_RELEVE: &[u8; 8] = b"SOIRREL1";

/// Une soirée de l'épreuve, telle que E20 la reprend : son nom dans le rapport, son
/// ouverture et son journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoireeJouee {
    /// Son nom : « soirée principale 0 », « soirée hostile »…
    pub nom: String,
    /// Ce qui l'a ouverte.
    pub ouverture: Ouverture,
    /// Son journal entier, depuis la création.
    pub journal: Journal,
}

/// Le chargement du module, tel qu'un moteur l'a vu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chargement {
    /// Accepté : l'empreinte du module réécrit, et le carburant de `Bonjour` et de
    /// `Decrire`.
    Accepte {
        /// L'empreinte du module réécrit : deux moteurs qui ne l'ont pas égale n'ont pas
        /// réécrit le même module (une autre version, d'autres réglages).
        empreinte: [u8; 32],
        /// Le carburant du premier `Bonjour`, et celui de `Decrire`.
        carburant: (u64, u64),
    },
    /// Refusé, classé ([`classer_refus`]).
    Refuse(String),
}

/// L'issue d'un appel relevé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Issue {
    /// Un retour : sa longueur et son empreinte courte.
    Rendu {
        /// Sa longueur, en octets.
        longueur: u32,
        /// Les huit premiers octets de son BLAKE3.
        empreinte: [u8; 8],
    },
    /// Une faute du jeu, classée par le protocole du commun.
    Faute(Faute),
    /// Une panne, par sa cause : son détail dit le moteur, et diffère d'un moteur à
    /// l'autre sans rien dire du jeu.
    Panne(String),
}

/// Un événement du relevé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evenement {
    /// Le rejeu d'une soirée commence (son rang dans les soirées).
    Soiree(u32),
    /// Une instance neuve, par son numéro.
    Neuve(u32),
    /// Un appel.
    Appel {
        /// L'instance.
        instance: u32,
        /// Le nom de l'appel (`Entree`, `Etat`…), ou `?` s'il ne se décode pas.
        nom: String,
        /// Son budget de carburant.
        budget: u64,
        /// Les huit premiers octets du BLAKE3 de l'appel encodé.
        appel: [u8; 8],
        /// Pour une entrée : son ordre et sa place.
        entree: Option<(u64, u64)>,
        /// Le carburant brûlé, faute comprise ; `None` pour une instance sans carburant.
        carburant: Option<u64>,
        /// Ce qu'il a rendu.
        issue: Issue,
    },
    /// Un instantané pris ; `Some` : la cause de sa panne.
    Instantane(u32, Option<String>),
    /// Une restauration ; `Some` : la cause de sa panne.
    Restaurer(u32, Option<String>),
    /// Une empreinte de l'état, ou la cause de sa panne.
    Empreinte(u32, Result<[u8; 32], String>),
    /// La fin du rejeu d'une soirée : `rejouée`, ou le refus classé.
    Fin(String),
}

/// Un relevé : le chargement, puis les événements du rejeu de chaque soirée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Releve {
    /// Le chargement du module.
    pub chargement: Chargement,
    /// Les événements, dans l'ordre.
    pub evenements: Vec<Evenement>,
}

/// La fin d'une soirée rejouée jusqu'au bout.
pub const REJOUEE: &str = "rejouée";

fn cause(c: CausePanne) -> String {
    format!("{c:?}")
}

/// Un refus du chargement, classé pour se comparer d'un moteur à l'autre : ce qui dit
/// le jeu (la réécriture, l'ABI, la déclaration, la faute et l'appel), et d'une panne
/// sa seule cause.
pub fn classer_refus(r: &RefusModule) -> String {
    match r {
        RefusModule::Reecriture(refus) => format!("réécriture : {refus}"),
        RefusModule::Abi { .. } | RefusModule::Declaration(_) => format!("{r}"),
        RefusModule::Faute { appel, faute } => format!("{appel} : {faute:?}"),
        RefusModule::Panne(p) => format!("panne : {}", cause(p.cause)),
    }
}

fn classer_ouverture(r: &RefusOuverture) -> String {
    match r {
        RefusOuverture::Panne(p) => format!("ouverture : panne {}", cause(p.cause)),
        RefusOuverture::Faute(f) => format!("ouverture : faute {f:?}"),
        RefusOuverture::Abi { module, coeur } => {
            format!("ouverture : ABI {module}, le cœur {coeur}")
        }
        RefusOuverture::Retour { appel } => format!("ouverture : {appel} rend autre chose"),
    }
}

fn classer_reprise(r: &RefusReprise) -> String {
    match r {
        RefusReprise::Ouverture(o) => format!("reprise : {}", classer_ouverture(o)),
        RefusReprise::Panne(p) => format!("reprise : panne {}", cause(p.cause)),
        RefusReprise::PointHorsJournal => String::from("reprise : point hors du journal"),
    }
}

fn court(octets: &[u8]) -> [u8; 8] {
    let h = blake3::hash(octets);
    let mut c = [0u8; 8];
    if let Some(debut) = h.as_bytes().first_chunk::<8>() {
        c = *debut;
    }
    c
}

/// Des octets en hexadécimal.
pub fn hex(octets: &[u8]) -> String {
    use core::fmt::Write as _;
    let mut s = String::with_capacity(octets.len().saturating_mul(2));
    for b in octets {
        let _: fmt::Result = write!(s, "{b:02x}");
    }
    s
}

/// Le nom d'un appel décodé.
const fn nom_de(a: &Appel<'_>) -> &'static str {
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
    }
}

fn issue_de(r: &Result<Vec<u8>, Echec>) -> Issue {
    match r {
        Ok(o) => Issue::Rendu {
            longueur: u32::try_from(o.len()).unwrap_or(u32::MAX),
            empreinte: court(o),
        },
        Err(Echec::Faute(f)) => Issue::Faute(f.clone()),
        Err(Echec::Panne(p)) => Issue::Panne(cause(p.cause)),
    }
}

/// La première différence entre deux relevés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ecart {
    /// Le rang de l'événement qui diffère, dans le relevé.
    pub indice: usize,
    /// La soirée où il tombe (son rang), si une a commencé.
    pub soiree: Option<u32>,
    /// La dernière entrée appelée avant lui, ordre et place.
    pub apres_l_entree: Option<(u64, u64)>,
    /// Ce que le relevé attendu a à ce rang ; `None` : il est fini.
    pub attendu: Option<Evenement>,
    /// Ce que l'autre a ; `None` : il s'arrête là.
    pub obtenu: Option<Evenement>,
}

fn decrire(e: &Evenement) -> String {
    match e {
        Evenement::Soiree(s) => format!("le début de la soirée {s}"),
        Evenement::Neuve(i) => format!("l'instance neuve {i}"),
        Evenement::Appel {
            instance,
            nom,
            entree,
            carburant,
            issue,
            ..
        } => {
            let quoi = match entree {
                Some((ordre, place)) => format!("{nom} d'ordre {ordre} (place {place})"),
                None => nom.clone(),
            };
            let carburant =
                carburant.map_or_else(|| String::from("sans carburant"), |c| format!("{c} unités"));
            let issue = match issue {
                Issue::Rendu {
                    longueur,
                    empreinte,
                } => {
                    format!("rend {longueur} octets ({})", hex(empreinte))
                }
                Issue::Faute(f) => format!("faute : {f}"),
                Issue::Panne(c) => format!("panne {c}"),
            };
            format!("{quoi} sur l'instance {instance} : {carburant}, {issue}")
        }
        Evenement::Instantane(i, p) => match p {
            None => format!("un instantané de l'instance {i}"),
            Some(c) => format!("un instantané de l'instance {i} en panne {c}"),
        },
        Evenement::Restaurer(i, p) => match p {
            None => format!("une restauration de l'instance {i}"),
            Some(c) => format!("une restauration de l'instance {i} en panne {c}"),
        },
        Evenement::Empreinte(i, r) => match r {
            Ok(e) => format!(
                "l'empreinte {}… de l'instance {i}",
                hex(e.get(..8).unwrap_or_default())
            ),
            Err(c) => format!("une empreinte de l'instance {i} en panne {c}"),
        },
        Evenement::Fin(t) => format!("la fin de la soirée : {t}"),
    }
}

/// Ce qui diffère entre deux appels, champ par champ.
fn differences(a: &Evenement, b: &Evenement) -> Vec<String> {
    let mut d = Vec::new();
    if let (
        Evenement::Appel {
            nom: na,
            budget: ba,
            appel: aa,
            carburant: ca,
            issue: ia,
            ..
        },
        Evenement::Appel {
            nom: nb,
            budget: bb,
            appel: ab,
            carburant: cb,
            issue: ib,
            ..
        },
    ) = (a, b)
    {
        if na != nb {
            d.push(format!("l'appel {nb} au lieu de {na}"));
        }
        if ba != bb {
            d.push(format!("un budget de {bb} au lieu de {ba}"));
        }
        if aa != ab {
            d.push(String::from("d'autres octets d'appel"));
        }
        if ca != cb {
            d.push(format!(
                "un carburant de {} au lieu de {}",
                cb.map_or_else(|| String::from("rien"), |c| format!("{c}")),
                ca.map_or_else(|| String::from("rien"), |c| format!("{c}"))
            ));
        }
        if ia != ib {
            d.push(String::from("une autre issue"));
        }
    }
    d
}

impl fmt::Display for Ecart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "événement {}", self.indice)?;
        if let Some(s) = self.soiree {
            write!(f, ", soirée {s}")?;
        }
        if let Some((ordre, place)) = self.apres_l_entree {
            write!(f, ", après l'entrée d'ordre {ordre} (place {place})")?;
        }
        f.write_str(" : ")?;
        match (&self.attendu, &self.obtenu) {
            (Some(a), Some(o)) => {
                let d = differences(a, o);
                if !d.is_empty() {
                    write!(f, "{} ; ", d.join(", "))?;
                }
                write!(
                    f,
                    "wasmtime : {} ; l'autre moteur : {}",
                    decrire(a),
                    decrire(o)
                )
            }
            (Some(a), None) => write!(f, "l'autre moteur s'arrête avant {}", decrire(a)),
            (None, Some(o)) => write!(f, "l'autre moteur a de plus {}", decrire(o)),
            (None, None) => f.write_str("aucun"),
        }
    }
}

/// Le contexte d'un rang : la soirée commencée et la dernière entrée appelée avant lui.
fn contexte(evenements: &[Evenement], indice: usize) -> (Option<u32>, Option<(u64, u64)>) {
    let mut soiree = None;
    let mut entree = None;
    for e in evenements.iter().take(indice) {
        match e {
            Evenement::Soiree(s) => {
                soiree = Some(*s);
                entree = None;
            }
            Evenement::Appel {
                entree: Some(x), ..
            } => entree = Some(*x),
            Evenement::Appel { entree: None, .. }
            | Evenement::Neuve(_)
            | Evenement::Instantane(..)
            | Evenement::Restaurer(..)
            | Evenement::Empreinte(..)
            | Evenement::Fin(_) => {}
        }
    }
    (soiree, entree)
}

/// La première différence entre le relevé `attendu` (wasmtime) et `obtenu`, ou `None`
/// s'ils sont égaux.
pub fn comparer(attendu: &[Evenement], obtenu: &[Evenement]) -> Option<Ecart> {
    let n = attendu.len().max(obtenu.len());
    (0..n).find_map(|k| {
        let (a, o) = (attendu.get(k), obtenu.get(k));
        (a != o).then(|| {
            let (soiree, apres_l_entree) = contexte(attendu, k);
            Ecart {
                indice: k,
                soiree,
                apres_l_entree,
                attendu: a.cloned(),
                obtenu: o.cloned(),
            }
        })
    })
}

/// Ce qu'une enregistreuse et ses instances partagent.
#[derive(Debug, Default)]
struct Etat {
    evenements: Vec<Evenement>,
    instances: u32,
    attendu: Option<Rc<[Evenement]>>,
    ecart: Option<Ecart>,
}

impl Etat {
    /// Écrit un événement ; avec un relevé attendu, le compare d'abord. Rend `false` si
    /// le rejeu doit s'arrêter : un écart est constaté, maintenant ou avant.
    fn noter(&mut self, e: Evenement) -> bool {
        if self.ecart.is_some() {
            return false;
        }
        if let Some(attendu) = &self.attendu {
            let k = self.evenements.len();
            let a = attendu.get(k);
            if a != Some(&e) {
                let (soiree, apres_l_entree) = contexte(&self.evenements, k);
                self.ecart = Some(Ecart {
                    indice: k,
                    soiree,
                    apres_l_entree,
                    attendu: a.cloned(),
                    obtenu: Some(e.clone()),
                });
                self.evenements.push(e);
                return false;
            }
        }
        self.evenements.push(e);
        true
    }
}

fn arrete() -> PanneOutil {
    PanneOutil::new(
        CausePanne::Interne,
        "E20 : le relevé a divergé, le rejeu s'arrête",
    )
}

/// Une fabrique dont chaque instance écrit ce qu'on lui fait au relevé.
pub struct Enregistreuse<F> {
    inner: F,
    etat: Rc<RefCell<Etat>>,
}

impl<F: Clone> Clone for Enregistreuse<F> {
    fn clone(&self) -> Enregistreuse<F> {
        Enregistreuse {
            inner: self.inner.clone(),
            etat: Rc::clone(&self.etat),
        }
    }
}

impl<F: Fabrique> Enregistreuse<F> {
    /// Une enregistreuse qui écrit tout, sans rien comparer (le relevé de wasmtime).
    pub fn new(inner: F) -> Enregistreuse<F> {
        Enregistreuse {
            inner,
            etat: Rc::new(RefCell::new(Etat::default())),
        }
    }

    /// Une enregistreuse qui compare chaque événement au relevé `attendu` dès qu'il
    /// arrive, et arrête le rejeu à la première différence.
    pub fn comparant(inner: F, attendu: Vec<Evenement>) -> Enregistreuse<F> {
        Enregistreuse {
            inner,
            etat: Rc::new(RefCell::new(Etat {
                attendu: Some(Rc::from(attendu)),
                ..Etat::default()
            })),
        }
    }

    /// Écrit un événement qui n'est pas d'une instance : le début ou la fin d'une
    /// soirée. Rend `false` si le rejeu doit s'arrêter.
    pub fn noter(&self, e: Evenement) -> bool {
        self.etat.borrow_mut().noter(e)
    }

    /// Les événements écrits jusqu'ici.
    pub fn evenements(&self) -> Vec<Evenement> {
        self.etat.borrow().evenements.clone()
    }

    /// La première différence avec le relevé attendu, s'il y en a un : constatée en
    /// chemin, ou, à la fin, un relevé plus court que l'attendu.
    pub fn ecart(&self) -> Option<Ecart> {
        let e = self.etat.borrow();
        if let Some(x) = &e.ecart {
            return Some(x.clone());
        }
        let attendu = e.attendu.as_ref()?;
        let k = e.evenements.len();
        attendu.get(k).map(|a| {
            let (soiree, apres_l_entree) = contexte(&e.evenements, k);
            Ecart {
                indice: k,
                soiree,
                apres_l_entree,
                attendu: Some(a.clone()),
                obtenu: None,
            }
        })
    }
}

impl<F: Fabrique> Fabrique for Enregistreuse<F> {
    type I = Enregistree<F::I>;

    fn neuve(&mut self) -> Result<Enregistree<F::I>, PanneOutil> {
        if self.etat.borrow().ecart.is_some() {
            return Err(arrete());
        }
        let inner = self.inner.neuve()?;
        let numero = {
            let mut e = self.etat.borrow_mut();
            let n = e.instances;
            e.instances = e.instances.saturating_add(1);
            n
        };
        if !self.noter(Evenement::Neuve(numero)) {
            return Err(arrete());
        }
        Ok(Enregistree {
            inner,
            numero,
            etat: Rc::clone(&self.etat),
        })
    }
}

/// Une instance dont chaque opération s'écrit au relevé.
pub struct Enregistree<I> {
    inner: I,
    numero: u32,
    etat: Rc<RefCell<Etat>>,
}

impl<I: Instance> Enregistree<I> {
    fn noter(&self, e: Evenement) -> bool {
        self.etat.borrow_mut().noter(e)
    }
}

impl<I: Instance> Instance for Enregistree<I> {
    type Instantane = I::Instantane;

    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8], budget: u64) -> Result<Vec<u8>, Echec> {
        if self.etat.borrow().ecart.is_some() {
            return Err(Echec::Panne(arrete()));
        }
        let decode = decoder::<Appel<'_>>(appel).ok();
        let nom = decode.as_ref().map_or("?", nom_de);
        let entree = match decode {
            Some(Appel::Entree { ordre, place, .. }) => Some((ordre, place)),
            Some(_) | None => None,
        };
        let r = self.inner.appeler(sorte, appel, budget);
        let e = Evenement::Appel {
            instance: self.numero,
            nom: String::from(nom),
            budget,
            appel: court(appel),
            entree,
            carburant: self.inner.carburant(),
            issue: issue_de(&r),
        };
        if self.noter(e) {
            r
        } else {
            Err(Echec::Panne(arrete()))
        }
    }

    fn carburant(&self) -> Option<u64> {
        self.inner.carburant()
    }

    fn instantane(&mut self) -> Result<I::Instantane, PanneOutil> {
        let r = self.inner.instantane();
        let e = Evenement::Instantane(self.numero, r.as_ref().err().map(|p| cause(p.cause)));
        if self.noter(e) { r } else { Err(arrete()) }
    }

    fn restaurer(&mut self, i: &I::Instantane) -> Result<(), PanneOutil> {
        let r = self.inner.restaurer(i);
        let e = Evenement::Restaurer(self.numero, r.as_ref().err().map(|p| cause(p.cause)));
        if self.noter(e) { r } else { Err(arrete()) }
    }

    fn empreinte(&mut self) -> Result<[u8; 32], PanneOutil> {
        let r = self.inner.empreinte();
        let e = Evenement::Empreinte(
            self.numero,
            r.as_ref().map(|x| *x).map_err(|p| cause(p.cause)),
        );
        if self.noter(e) { r } else { Err(arrete()) }
    }
}

/// Reprend une soirée depuis sa création, sur des instances de `f` : la partie
/// s'ouvre (`Bonjour`, `Decrire`, `Creer`, l'état, le point de la création), puis
/// [`Partie::reprendre_en_refaisant_les_fautes`] rejoue tout le journal depuis ce point,
/// chaque empreinte notée vérifiée en chemin, et chaque entrée écartée rejouée pour
/// refaire sa faute (sans quoi une faute ne se comparerait pas d'un moteur à l'autre) ;
/// enfin la forme canonique et l'empreinte de la partie reprise.
/// Rend ce que [`Evenement::Fin`] note.
fn rejouer_une<F: Fabrique + Clone>(f: &Enregistreuse<F>, s: &SoireeJouee) -> String {
    let point = match Partie::ouvrir(f.clone(), s.ouverture.clone()) {
        Ok(mut p) => match p.point() {
            Ok(point) => point,
            Err(panne) => return format!("point de la création : panne {}", cause(panne.cause)),
        },
        Err(r) => return classer_ouverture(&r),
    };
    match Partie::reprendre_en_refaisant_les_fautes(
        f.clone(),
        s.ouverture.clone(),
        &point,
        &s.journal,
    ) {
        Ok(mut reprise) => {
            let _: Result<Vec<u8>, Echec> = reprise.canonique();
            let _: Result<[u8; 32], PanneOutil> = reprise.empreinte();
            String::from(REJOUEE)
        }
        Err(r) => classer_reprise(&r),
    }
}

/// Les fautes `Budget` provoquées, avant les soirées (lot 3b, après la vérification) :
/// les journaux des modules d'aujourd'hui ne font refaire que des `Panique`, et sans
/// elles aucune autre sorte de faute ne passerait par E20.
///
/// Sur une instance neuve, `Bonjour` puis `Decrire` au budget de la lecture : le
/// carburant `c` de `Decrire` est relevé. Puis, chaque fois sur une instance neuve,
/// `Bonjour` et `Decrire` au budget `c / 2`, puis `c - 1` : le carburant s'épuise au
/// milieu de l'appel, puis à une unité de sa fin. Les deux moteurs doivent rendre la
/// même faute au même carburant ; l'épreuve vérifie que wasmtime en fait des `Budget`
/// (sinon E20 est une panne de l'outil, [`budgets_provoques`]).
///
/// Ne provoque rien si `Bonjour` ou le premier `Decrire` n'aboutit pas, si l'instance
/// n'a pas de carburant, ou si le relevé a déjà divergé.
fn provoquer_le_budget<F: Fabrique + Clone>(f: &Enregistreuse<F>, lecture: u64) {
    let (mut bonjour, mut decrire) = (Vec::new(), Vec::new());
    if encoder(&Appel::Bonjour, &mut bonjour).is_err()
        || encoder(&Appel::Decrire, &mut decrire).is_err()
    {
        return;
    }
    let mut fabrique = f.clone();
    // Une instance neuve, `Bonjour`, puis `Decrire` au budget donné : ce que `Decrire`
    // rend, et son carburant. `None` si l'instance ou `Bonjour` n'aboutit pas.
    let mut accueillir = |budget: u64| {
        let mut i = fabrique.neuve().ok()?;
        i.appeler(Appel::Bonjour.sorte(), &bonjour, lecture).ok()?;
        let r = i.appeler(Appel::Decrire.sorte(), &decrire, budget);
        Some((r, i.carburant()))
    };
    let Some((Ok(_), Some(c))) = accueillir(lecture) else {
        return;
    };
    let mut budgets = [c / 2, c.saturating_sub(1)].to_vec();
    budgets.dedup();
    for b in budgets {
        if accueillir(b).is_none() {
            return;
        }
    }
}

/// Combien de fautes `Budget` un relevé a avant sa première soirée : celles que
/// [`rejouer`] provoque. Deux sur un module dont `Decrire` coûte au moins deux unités.
pub fn budgets_provoques(evenements: &[Evenement]) -> usize {
    evenements
        .iter()
        .take_while(|e| !matches!(e, Evenement::Soiree(_)))
        .filter(|e| {
            matches!(
                e,
                Evenement::Appel {
                    issue: Issue::Faute(Faute::Budget),
                    ..
                }
            )
        })
        .count()
}

/// Reprend chaque soirée depuis sa création, dans l'ordre, sur l'enregistreuse `f` ; le
/// relevé est dans `f` ([`Enregistreuse::evenements`]). S'arrête à la première
/// différence si `f` compare.
///
/// Avant les soirées, au budget de lecture de la première, les fautes `Budget`
/// provoquées ([`provoquer_le_budget`]) : rien, s'il n'y a aucune soirée.
pub fn rejouer<F: Fabrique + Clone>(f: &Enregistreuse<F>, soirees: &[SoireeJouee]) {
    if let Some(s) = soirees.first() {
        provoquer_le_budget(f, s.ouverture.budgets.lecture);
    }
    for (k, s) in soirees.iter().enumerate() {
        if !f.noter(Evenement::Soiree(u32::try_from(k).unwrap_or(u32::MAX))) {
            return;
        }
        let fin = rejouer_une(f, s);
        if !f.noter(Evenement::Fin(fin)) {
            return;
        }
    }
}

/// Combien d'appels un relevé contient, et combien de fautes, par sorte (`Budget`,
/// `Panique`…) : ce qu'un rapport dit de ce qui a été classé.
pub fn compter(evenements: &[Evenement]) -> (u64, Vec<(String, u64)>) {
    let mut appels = 0u64;
    let mut fautes: Vec<(String, u64)> = Vec::new();
    for e in evenements {
        if let Evenement::Appel { issue, .. } = e {
            appels = appels.saturating_add(1);
            if let Issue::Faute(f) = issue {
                let sorte = format!("{f:?}");
                let sorte = String::from(sorte.split([' ', '{', '(']).next().unwrap_or("?"));
                match fautes.iter_mut().find(|(s, _)| *s == sorte) {
                    Some((_, n)) => *n = n.saturating_add(1),
                    None => fautes.push((sorte, 1)),
                }
            }
        }
    }
    (appels, fautes)
}

/// L'empreinte d'octets (le relevé encodé) : deux relevés égaux ont la même.
pub fn empreinte(octets: &[u8]) -> [u8; 32] {
    *blake3::hash(octets).as_bytes()
}

// L'écriture.

fn u32_le(o: &mut Vec<u8>, x: u32) {
    o.extend_from_slice(&x.to_le_bytes());
}

fn u64_le(o: &mut Vec<u8>, x: u64) {
    o.extend_from_slice(&x.to_le_bytes());
}

fn octets_le(o: &mut Vec<u8>, b: &[u8]) {
    u32_le(o, u32::try_from(b.len()).unwrap_or(u32::MAX));
    o.extend_from_slice(b);
}

fn texte_le(o: &mut Vec<u8>, t: &str) {
    octets_le(o, t.as_bytes());
}

fn option_texte(o: &mut Vec<u8>, t: Option<&String>) {
    match t {
        None => o.push(0),
        Some(t) => {
            o.push(1);
            texte_le(o, t);
        }
    }
}

/// Les soirées, encodées.
pub fn encoder_soirees(soirees: &[SoireeJouee]) -> Vec<u8> {
    let mut o = Vec::new();
    o.extend_from_slice(ENTETE_SOIREES);
    u32_le(&mut o, u32::try_from(soirees.len()).unwrap_or(u32::MAX));
    for s in soirees {
        texte_le(&mut o, &s.nom);
        octets_le(&mut o, &encoder_ouverture(&s.ouverture));
        octets_le(&mut o, &encoder_journal(&s.journal));
    }
    o
}

fn encoder_evenement(o: &mut Vec<u8>, evenement: &Evenement) {
    match evenement {
        Evenement::Soiree(s) => {
            o.push(1);
            u32_le(o, *s);
        }
        Evenement::Neuve(i) => {
            o.push(2);
            u32_le(o, *i);
        }
        Evenement::Appel {
            instance,
            nom,
            budget,
            appel,
            entree,
            carburant,
            issue,
        } => {
            o.push(3);
            u32_le(o, *instance);
            texte_le(o, nom);
            u64_le(o, *budget);
            o.extend_from_slice(appel);
            match entree {
                None => o.push(0),
                Some((ordre, place)) => {
                    o.push(1);
                    u64_le(o, *ordre);
                    u64_le(o, *place);
                }
            }
            match carburant {
                None => o.push(0),
                Some(c) => {
                    o.push(1);
                    u64_le(o, *c);
                }
            }
            match issue {
                Issue::Rendu {
                    longueur,
                    empreinte,
                } => {
                    o.push(0);
                    u32_le(o, *longueur);
                    o.extend_from_slice(empreinte);
                }
                Issue::Faute(f) => {
                    o.push(1);
                    let mut b = Vec::new();
                    // Une faute s'encode toujours : ses champs sont des textes.
                    let _: Result<(), jeu_abi::ErreurEncodage> = encoder(f, &mut b);
                    octets_le(o, &b);
                }
                Issue::Panne(c) => {
                    o.push(2);
                    texte_le(o, c);
                }
            }
        }
        Evenement::Instantane(i, p) => {
            o.push(4);
            u32_le(o, *i);
            option_texte(o, p.as_ref());
        }
        Evenement::Restaurer(i, p) => {
            o.push(5);
            u32_le(o, *i);
            option_texte(o, p.as_ref());
        }
        Evenement::Empreinte(instance, resultat) => {
            o.push(6);
            u32_le(o, *instance);
            match resultat {
                Ok(empreinte) => {
                    o.push(0);
                    o.extend_from_slice(empreinte);
                }
                Err(panne) => {
                    o.push(1);
                    texte_le(o, panne);
                }
            }
        }
        Evenement::Fin(t) => {
            o.push(7);
            texte_le(o, t);
        }
    }
}

/// Le relevé, encodé.
pub fn encoder_releve(r: &Releve) -> Vec<u8> {
    let mut o = Vec::new();
    o.extend_from_slice(ENTETE_RELEVE);
    match &r.chargement {
        Chargement::Accepte {
            empreinte,
            carburant,
        } => {
            o.push(0);
            o.extend_from_slice(empreinte);
            u64_le(&mut o, carburant.0);
            u64_le(&mut o, carburant.1);
        }
        Chargement::Refuse(t) => {
            o.push(1);
            texte_le(&mut o, t);
        }
    }
    u32_le(
        &mut o,
        u32::try_from(r.evenements.len()).unwrap_or(u32::MAX),
    );
    for e in &r.evenements {
        encoder_evenement(&mut o, e);
    }
    o
}

// La lecture.

/// Pourquoi des octets ne se relisent pas en soirées ou en relevé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Illisible(pub String);

impl fmt::Display for Illisible {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

struct Lecteur<'a> {
    reste: &'a [u8],
}

impl<'a> Lecteur<'a> {
    fn prendre(&mut self, n: usize) -> Result<&'a [u8], Illisible> {
        if self.reste.len() < n {
            return Err(Illisible(String::from("coupé")));
        }
        let (a, b) = self.reste.split_at(n);
        self.reste = b;
        Ok(a)
    }

    fn octet(&mut self) -> Result<u8, Illisible> {
        self.prendre(1)?
            .first()
            .copied()
            .ok_or_else(|| Illisible(String::from("coupé")))
    }

    fn tableau<const N: usize>(&mut self) -> Result<[u8; N], Illisible> {
        self.prendre(N)?
            .first_chunk::<N>()
            .copied()
            .ok_or_else(|| Illisible(String::from("coupé")))
    }

    fn u32(&mut self) -> Result<u32, Illisible> {
        Ok(u32::from_le_bytes(self.tableau::<4>()?))
    }

    fn u64(&mut self) -> Result<u64, Illisible> {
        Ok(u64::from_le_bytes(self.tableau::<8>()?))
    }

    fn octets(&mut self) -> Result<&'a [u8], Illisible> {
        let n = usize::try_from(self.u32()?).map_err(|_| Illisible(String::from("trop long")))?;
        self.prendre(n)
    }

    fn texte(&mut self) -> Result<String, Illisible> {
        let b = self.octets()?;
        core::str::from_utf8(b)
            .map(String::from)
            .map_err(|_| Illisible(String::from("texte non UTF-8")))
    }

    fn option_texte(&mut self) -> Result<Option<String>, Illisible> {
        match self.octet()? {
            0 => Ok(None),
            1 => self.texte().map(Some),
            x => Err(Illisible(format!("option {x}"))),
        }
    }

    fn fini(&self) -> Result<(), Illisible> {
        if self.reste.is_empty() {
            Ok(())
        } else {
            Err(Illisible(format!("{} octets de trop", self.reste.len())))
        }
    }
}

/// Relit des soirées.
pub fn decoder_soirees(octets: &[u8]) -> Result<Vec<SoireeJouee>, Illisible> {
    let mut l = Lecteur { reste: octets };
    if l.prendre(8)? != ENTETE_SOIREES {
        return Err(Illisible(String::from("ce ne sont pas des soirées")));
    }
    let n = l.u32()?;
    let mut v = Vec::new();
    for _ in 0..n {
        let nom = l.texte()?;
        let ouverture = decoder_ouverture(l.octets()?)
            .map_err(|e| Illisible(format!("ouverture de « {nom} » : {e:?}")))?;
        let journal = decoder_journal(l.octets()?)
            .map_err(|e| Illisible(format!("journal de « {nom} » : {e:?}")))?;
        v.push(SoireeJouee {
            nom,
            ouverture,
            journal,
        });
    }
    l.fini()?;
    Ok(v)
}

fn decoder_evenement(l: &mut Lecteur<'_>) -> Result<Evenement, Illisible> {
    Ok(match l.octet()? {
        1 => Evenement::Soiree(l.u32()?),
        2 => Evenement::Neuve(l.u32()?),
        3 => {
            let instance = l.u32()?;
            let nom = l.texte()?;
            let budget = l.u64()?;
            let appel = l.tableau::<8>()?;
            let entree = match l.octet()? {
                0 => None,
                1 => Some((l.u64()?, l.u64()?)),
                x => return Err(Illisible(format!("entrée {x}"))),
            };
            let carburant = match l.octet()? {
                0 => None,
                1 => Some(l.u64()?),
                x => return Err(Illisible(format!("carburant {x}"))),
            };
            let issue = match l.octet()? {
                0 => Issue::Rendu {
                    longueur: l.u32()?,
                    empreinte: l.tableau::<8>()?,
                },
                1 => Issue::Faute(
                    decoder::<Faute>(l.octets()?)
                        .map_err(|e| Illisible(format!("faute : {e:?}")))?,
                ),
                2 => Issue::Panne(l.texte()?),
                x => return Err(Illisible(format!("issue {x}"))),
            };
            Evenement::Appel {
                instance,
                nom,
                budget,
                appel,
                entree,
                carburant,
                issue,
            }
        }
        4 => Evenement::Instantane(l.u32()?, l.option_texte()?),
        5 => Evenement::Restaurer(l.u32()?, l.option_texte()?),
        6 => {
            let i = l.u32()?;
            match l.octet()? {
                0 => Evenement::Empreinte(i, Ok(l.tableau::<32>()?)),
                1 => Evenement::Empreinte(i, Err(l.texte()?)),
                x => return Err(Illisible(format!("empreinte {x}"))),
            }
        }
        7 => Evenement::Fin(l.texte()?),
        x => return Err(Illisible(format!("événement {x}"))),
    })
}

/// Relit un relevé.
pub fn decoder_releve(octets: &[u8]) -> Result<Releve, Illisible> {
    let mut l = Lecteur { reste: octets };
    if l.prendre(8)? != ENTETE_RELEVE {
        return Err(Illisible(String::from("ce n'est pas un relevé")));
    }
    let chargement = match l.octet()? {
        0 => Chargement::Accepte {
            empreinte: l.tableau::<32>()?,
            carburant: (l.u64()?, l.u64()?),
        },
        1 => Chargement::Refuse(l.texte()?),
        x => return Err(Illisible(format!("chargement {x}"))),
    };
    let n = l.u32()?;
    let mut evenements = Vec::new();
    for _ in 0..n {
        evenements.push(decoder_evenement(&mut l)?);
    }
    l.fini()?;
    Ok(Releve {
        chargement,
        evenements,
    })
}
