//! Valider un module et le réécrire (INTERFACE.md, § 3.1, § 3.6 et § 8.5).
//!
//! La réécriture est une fonction pure des octets du module, de ses réglages et de
//! [`VERSION`] : les mêmes octets en entrée donnent les mêmes octets en sortie, sur
//! toute machine. Elle ne dépend pas de wasmtime : le moteur du navigateur la
//! reprendra telle quelle en phase 2. Le module réécrit porte lui-même son budget,
//! sa profondeur, ses NaN canoniques et sa croissance bornée (D32) : il se comporte
//! pareil dans tout moteur qui l'exécute.
//!
//! # Les passes
//!
//! | Passe | Ce qu'elle fait |
//! |---|---|
//! | [`passes::valider`] | § 3.1, exactement les exports de § 3.3, un maximum de mémoire, la pile en bas |
//! | [`passes::globales`] | exporte chaque globale mutable d'origine sous `soiree_globale_<n>` ; `data.drop` devient `nop` |
//! | [`passes::carburant`] | `soiree_carburant`, décrémenté au début de chaque segment : entrée de fonction, tête de boucle, branche, jonction |
//! | [`passes::profondeur`] | `soiree_profondeur`, compté autour de chaque appel (et, pour les seules mesures, [`passes::profondeur_mesuree`]) |
//! | [`passes::nan`] | un NaN calculé devient le NaN canonique |
//! | [`passes::croissance`] | `memory.grow` borné par `soiree_memoire_max` |
//! | [`passes::pile`] | la pile remise à zéro au début de chaque écriture |
//!
//! [`reecrire`] les enchaîne dans cet ordre, puis revalide le résultat. Le carburant
//! passe avant les autres passes de code : il compte les instructions du module
//! d'origine, et non celles que les passes ajoutent.
//!
//! # Ce qui se lit après un piège
//!
//! Un piège du module réécrit se classe par ses globales (§ 4.6) : `soiree_carburant`
//! négatif, c'est le budget ; `soiree_profondeur` au-delà du plafond, la profondeur ;
//! `soiree_croissance_bornee` à 1, la mémoire refusée par le plafond ;
//! `soiree_croissance_refusee` à 1, la machine qui a refusé une croissance permise.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod analyse;
mod ecriture;
mod operateurs;
pub mod passes;
mod valider;

use core::fmt;

/// La version de la réécriture. Elle monte à chaque changement de ce que produit une
/// passe : deux moteurs ne comparent que des modules réécrits par la même version
/// (§ 3.8).
///
/// Version 3 (2026-11-04) : `memory.init` paie sa longueur divisée par 64, comme
/// `memory.copy` et `memory.fill` ; un appel compte autant d'étages de profondeur
/// que le cadre de la fonction appelée en demande.
///
/// Version 2 (2026-11-04) : la passe [`passes::pile`], et le carburant compté par
/// segment plutôt que par fonction entière (`conception/BESOINS-DES-JEUX.md`, besoins 1
/// et 13).
pub const VERSION: u32 = 3;

/// Le nombre maximal de paramètres et de locaux d'une fonction (§ 3.1) : le plafond
/// de `wasmparser` (50 000), moins les locaux que les passes peuvent ajouter (cinq
/// aujourd'hui, seize réservés). Au-delà, la validation refuse la fonction, à sa
/// position dans l'original, plutôt que de laisser la réécriture produire un module
/// invalide.
pub const LOCAUX_MAX: u32 = 50_000 - 16;

/// La plus grande hauteur permise de la pile d'opérandes d'une fonction (§ 3.1).
/// Des valeurs empilées par milliers et vivantes à travers un appel font croître la
/// compilation plus que linéairement (2026-11-04, Apple M5 : 20 000 valeurs, 5,2 s
/// pour une seule fonction) ; sous ce plafond, le temps de compilation reste
/// proportionnel à la taille du module. rustc n'empile que quelques dizaines de
/// valeurs. Point de départ, non mesuré sur des modules réels.
pub const HAUTEUR_MAX: u32 = 1024;

/// Le nom de la globale de carburant, exportée.
pub const CARBURANT: &str = "soiree_carburant";
/// Le nom de la globale de profondeur d'appel, exportée.
pub const PROFONDEUR: &str = "soiree_profondeur";
/// Le nom de la globale de la plus grande profondeur atteinte, exportée par la seule
/// variante de mesure ([`reecrire_en_mesurant_la_profondeur`], M9) : jamais dans un
/// module de partie.
pub const PROFONDEUR_ATTEINTE: &str = "soiree_profondeur_atteinte";
/// Le nom de la globale du maximum de mémoire, en pages, exportée.
pub const MEMOIRE_MAX: &str = "soiree_memoire_max";
/// Le nom de la globale posée quand un `memory.grow` permis a échoué : la machine.
pub const CROISSANCE_REFUSEE: &str = "soiree_croissance_refusee";
/// Le nom de la globale posée quand un `memory.grow` dépasse `soiree_memoire_max` :
/// le jeu.
pub const CROISSANCE_BORNEE: &str = "soiree_croissance_bornee";
/// Le préfixe des globales mutables d'origine, exportées sous `soiree_globale_<n>`,
/// où `n` est l'indice de la globale dans le module.
pub const PREFIXE_GLOBALE: &str = "soiree_globale_";

/// Le nom d'export d'une globale mutable d'origine.
pub fn nom_de_globale(indice: u32) -> String {
    format!("{PREFIXE_GLOBALE}{indice}")
}

/// Les réglages de la réécriture. Ils entrent dans les octets du module réécrit, donc
/// dans son empreinte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reglages {
    /// Les étages d'appels imbriqués permis sous l'appel du moteur
    /// ([`passes::etages`] : un par appel, davantage pour une fonction au cadre
    /// épais). Au-delà, le module exécute `unreachable`, et `soiree_profondeur`
    /// dépasse `profondeur_max`.
    pub profondeur_max: u32,
    /// La mémoire que le moteur accorde au plus, en pages de 64 Kio. La valeur
    /// initiale de `soiree_memoire_max` est le plus petit de ce plafond et du maximum
    /// déclaré par le module ; le moteur l'abaisse ensuite à `limites.memoire`.
    pub memoire_plafond_pages: u32,
}

impl Reglages {
    /// Des points de départ pour la phase 1, **non mesurés** (INTERFACE.md, § 13,
    /// question 12) : 4 096 appels imbriqués, 1 Gio de mémoire.
    pub const PHASE_1: Reglages = Reglages {
        profondeur_max: 4096,
        memoire_plafond_pages: 16_384,
    };

    /// Les réglages de la partie depuis la phase 3, les mêmes pour wasmtime et pour
    /// chaque navigateur (PHASE-3, P4) : le module réécrit est le même partout, et ses
    /// empreintes changent avec eux.
    ///
    /// La profondeur, 64 étages, est réglée entre les deux moitiés de M9, mesurées le
    /// 2026-11-05 (`conception/MESURES-PHASE-3.md`, « Lot 3c ») :
    ///
    /// - **ce que les jeux atteignent**, sur toute leur épreuve au plan standard
    ///   (`epreuve/examples/profondeur.rs`, la variante de mesure de la passe) : `devine`
    ///   10 étages, `ardoise` 9, Punchline 15, Sidequest 13, Tier list 18 ;
    /// - **ce que tient la pile d'un Worker**, sur la récursion la plus épaisse par
    ///   étage (250 valeurs `i64` vivantes, un étage par appel) : 247 étages dans Chrome
    ///   156, 492 dans Firefox 158 ; deux fois plus sur le fil d'une page. Safari n'est
    ///   pas mesuré (`safaridriver` refuse la session).
    ///
    /// 64 laisse un facteur 3,5 au jeu le plus profond, et 3,9 à la pile du Worker le
    /// plus court. Les 4 096 étages de [`Reglages::PHASE_1`] auraient laissé un Worker
    /// de Chrome épuiser sa pile native (une panne de l'outil) avant la faute
    /// `Profondeur` du jeu.
    pub const PHASE_3: Reglages = Reglages {
        profondeur_max: 64,
        memoire_plafond_pages: 16_384,
    };
}

/// Un module validé et réécrit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reecrit {
    /// Les octets du module réécrit.
    pub octets: Vec<u8>,
    /// BLAKE3 des octets livrés, avant réécriture : la version du jeu (§ 3.8, OUT-37).
    pub empreinte_originale: [u8; 32],
    /// BLAKE3 des octets réécrits. Avec [`VERSION`], l'identité du module réécrit.
    pub empreinte: [u8; 32],
    /// Ce que le module peut faire, constaté sur ses octets.
    pub constat: Constat,
    /// La valeur initiale de `__stack_pointer` : la pile occupe `[0, pile_initiale)`.
    /// 0 pour un module sans pile.
    pub pile_initiale: u32,
    /// L'indice de la globale `__stack_pointer`, exportée sous `soiree_globale_<n>` ;
    /// `None` pour un module sans pile.
    pub pile_globale: Option<u32>,
}

/// Ce que le module peut faire, constaté sur ses octets d'origine (§ 3.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constat {
    /// Les imports : toujours vide si le module est accepté.
    pub imports: Vec<(String, String)>,
    /// Les exports d'origine, dans l'ordre du module.
    pub exports: Vec<String>,
    /// La mémoire minimale et maximale, en pages.
    pub memoire: (u32, Option<u32>),
    /// Les familles d'instructions présentes, avec leur nombre, triées par nom.
    pub familles: Vec<(&'static str, u32)>,
    /// Le nombre de globales mutables d'origine.
    pub globales_mutables: u32,
}

/// Pourquoi un module est refusé. Chaque refus nomme ce qu'il refuse, et sa
/// position en octets depuis le début du module quand elle a un sens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refus {
    /// Les octets ne sont pas un module WebAssembly valide.
    Illisible {
        /// Où la lecture s'est arrêtée.
        position: usize,
        /// Ce que le lecteur en dit.
        quoi: String,
    },
    /// Le module importe quelque chose : il n'a droit à rien (§ 3.1, D5).
    Import {
        /// Le module de l'import.
        module: String,
        /// Son nom.
        nom: String,
    },
    /// Un export hors de § 3.3, ou un export de § 3.3 du mauvais type.
    Export {
        /// Son nom.
        nom: String,
    },
    /// Un export de § 3.3 manque.
    ExportManquant {
        /// Son nom.
        nom: &'static str,
    },
    /// Une fonctionnalité refusée par § 3.1, ailleurs que dans une instruction : un
    /// type, une table, une globale, une section.
    Fonctionnalite {
        /// Son nom.
        nom: &'static str,
        /// Sa position.
        position: usize,
    },
    /// Une instruction hors de la liste de § 3.1.
    Instruction {
        /// Son nom, celui de `wasmparser::Operator`.
        nom: &'static str,
        /// Sa position.
        position: usize,
    },
    /// Le module a une fonction de départ.
    Depart,
    /// La mémoire n'a pas la forme de § 3.1.
    Memoire(RefusMemoire),
    /// La pile n'est pas en bas de la mémoire : une donnée est sous la valeur
    /// initiale de `__stack_pointer`, ou cette valeur ne se lit pas (§ 3.1).
    PileNonEnBas,
    /// La réécriture a produit un module invalide : une panne de l'outil, jamais une
    /// faute du jeu (HIS-06).
    Reecriture {
        /// Ce que le lecteur en dit.
        quoi: String,
    },
}

/// Ce qui ne va pas dans la mémoire d'un module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusMemoire {
    /// Aucune mémoire.
    Absente,
    /// Plusieurs mémoires.
    Plusieurs,
    /// Une mémoire partagée.
    Partagee,
    /// Une mémoire 64 bits.
    Bits64,
    /// Une mémoire sans maximum.
    SansMaximum,
    /// Une mémoire à pages de taille choisie.
    PagesChoisies,
}

impl fmt::Display for Refus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refus::Illisible { position, quoi } => {
                write!(f, "module illisible à la position {position} : {quoi}")
            }
            Refus::Import { module, nom } => write!(f, "import refusé : {module}::{nom}"),
            Refus::Export { nom } => write!(f, "export refusé : {nom}"),
            Refus::ExportManquant { nom } => write!(f, "export manquant : {nom}"),
            Refus::Fonctionnalite { nom, position } => {
                write!(f, "fonctionnalité refusée à la position {position} : {nom}")
            }
            Refus::Instruction { nom, position } => {
                write!(f, "instruction refusée à la position {position} : {nom}")
            }
            Refus::Depart => f.write_str("fonction de départ refusée"),
            Refus::Memoire(m) => write!(f, "mémoire refusée : {m:?}"),
            Refus::PileNonEnBas => f.write_str("la pile n'est pas en bas de la mémoire"),
            Refus::Reecriture { quoi } => write!(f, "réécriture invalide : {quoi}"),
        }
    }
}

impl std::error::Error for Refus {}

/// Valide un module et le réécrit (§ 3.6) : valider, puis globales, carburant,
/// profondeur, NaN, croissance et pile, puis revalider le résultat.
///
/// Fonction pure : les mêmes octets et les mêmes réglages donnent les mêmes octets.
pub fn reecrire(original: &[u8], r: &Reglages) -> Result<Reecrit, Refus> {
    reecrire_et_mesurer(original, *r, passes::Variante::Partie)
}

/// [`reecrire`], avec la variante de mesure de la passe de profondeur
/// ([`passes::profondeur_mesuree`]) : le module garde aussi la plus grande profondeur
/// atteinte dans [`PROFONDEUR_ATTEINTE`]. **Réservée au banc et aux mesures** (M9,
/// PHASE-3, § 2.3) : le module rendu brûle le même carburant et rend les mêmes octets
/// que celui de [`reecrire`], mais ses octets, donc son empreinte, diffèrent. Il ne sert
/// jamais à une partie, ni à un rapport de l'épreuve.
pub fn reecrire_en_mesurant_la_profondeur(original: &[u8], r: &Reglages) -> Result<Reecrit, Refus> {
    reecrire_et_mesurer(original, *r, passes::Variante::Mesure)
}

fn reecrire_et_mesurer(
    original: &[u8],
    r: Reglages,
    variante: passes::Variante,
) -> Result<Reecrit, Refus> {
    let examen = valider::examiner(original)?;
    let m = passes::globales(original)?;
    let m = passes::carburant(&m)?;
    let m = match variante {
        passes::Variante::Partie => passes::profondeur(&m, r.profondeur_max)?,
        passes::Variante::Mesure => passes::profondeur_mesuree(&m, r.profondeur_max)?,
    };
    let m = passes::nan(&m)?;
    let m = passes::croissance(&m, r.memoire_plafond_pages)?;
    let m = passes::pile(&m, examen.pile_globale)?;
    // L'original a passé la même validation : un refus ici vient des passes, une
    // panne de l'outil et non une faute du jeu (HIS-06).
    valider::revalider(&m).map_err(|r| Refus::Reecriture {
        quoi: format!("le module réécrit ne se valide plus : {r}"),
    })?;
    Ok(Reecrit {
        empreinte_originale: *blake3::hash(original).as_bytes(),
        empreinte: *blake3::hash(&m).as_bytes(),
        octets: m,
        constat: examen.constat,
        pile_initiale: examen.pile_initiale,
        pile_globale: examen.pile_globale,
    })
}
