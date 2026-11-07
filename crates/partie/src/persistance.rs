//! Ce que la garde tient, en octets (PHASE-3, § 2.4) : un point de reprise, l'état du
//! cœur, l'ouverture et les lignes du journal, pour qu'une partie reprenne après un
//! onglet rechargé, un Worker tué ou un relais relancé (CONTRAT, « La reprise survit à
//! un onglet qui se recharge »).
//!
//! # La forme
//!
//! Chaque message commence par un en-tête de cinq octets : [`VERSION_GARDE`] en `u32`
//! petit-boutiste, puis la sorte du message (un octet : 1 un point, 2 un journal ou un
//! morceau de journal, 3 une ouverture, 4 l'état du cœur). Le reste est le contenu, en
//! postcard (`jeu_abi::encoder`), relu strictement : un octet de trop, un message
//! coupé, une valeur hors de son domaine sont refusés, jamais acceptés à moitié.
//!
//! Les octets sont figés par des tests (`tests/coeur/garde.rs` et
//! `tests/coeur/forme_garde.rs`, comme `forme_figee` de `jeu-abi`) : chaque variant de
//! chaque enum gardée, un état du cœur qui a de chaque chose, un point dont la chaîne
//! a trois points ; une seconde lecture indépendante, qui n'utilise ni serde ni postcard
//! (SAL-90), les relit champ à champ. Changer la forme d'un type gardé fait rougir l'un
//! et l'autre, et demande de monter [`VERSION_GARDE`]. L'instantané d'un module a les
//! siens (`moteur-commun`, `instantane.rs`). Une garde d'une autre version est refusée,
//! typée ([`RefusGarde::Version`]) : on ne relit pas des octets qu'on ne sait plus lire.
//!
//! # Un point de reprise
//!
//! Un point garde, à côté de son instantané et de l'état du cœur, le point sain et le
//! dernier point à intervalle que la partie avait (`sain`, `periodique`) : une reprise
//! depuis lui retrouve [`crate::Partie::repartir`]. Cette chaîne est **aplatie** : une
//! table des points, le premier étant le point lui-même, chacun ne citant que des points
//! plus loin dans la table ; et **dédoublonnée** : un point cité deux fois (le point sain
//! est souvent le dernier point à intervalle) ne s'écrit qu'une fois, comme un
//! instantané que deux points partagent. L'instantané s'écrit par l'instance qui le
//! connaît ([`InstantaneGarde`]) : le cœur ne lit pas la mémoire d'un module.
//!
//! # Ce qui n'est pas ici
//!
//! Le chiffrement, les cases et le relais sont à `salle` (lot 4). Ce module ne fait
//! aucune entrée-sortie.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use jeu_abi::{ErreurFil, Pas, decoder};
use serde::{Deserialize, Serialize};

use crate::coeur::EtatCoeur;
use crate::journal::{Journal, Ligne};
use crate::partie::{Ouverture, PointDeReprise};

/// La version de la forme de la garde. Elle monte à chaque changement de la forme d'un
/// type gardé (un champ, un variant, leur ordre) : les octets figés de
/// `tests/coeur/garde.rs`, de `tests/coeur/forme_garde.rs` et de l'instantané d'un
/// module (`moteur-commun`, `instantane.rs`) se réécrivent alors à la main.
///
/// Version 2 (PHASE-3, lot 2) : l'instantané d'un module ne garde plus que la mémoire
/// hors des quatre zones (P3). Les types de `partie` n'ont pas changé : leurs octets
/// figés sont ceux de la version 1, sous un autre en-tête.
///
/// Version 3 (PHASE-3, lot 6, l'ABI 4) : une arrivée de fichiers dit son sélecteur
/// (`ComposantArrivee::Fichiers::selecteur`), et un rendu réussi le nombre d'images
/// qu'il n'a pas dessinées (`ReponseArrivee::Rendu`). Les octets des autres types ne
/// bougent pas.
pub const VERSION_GARDE: u32 = 3;

/// La sorte d'un message de la garde : le cinquième octet de l'en-tête.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SorteGarde {
    /// Un point de reprise.
    Point = 1,
    /// Un journal, ou un morceau de journal.
    Journal = 2,
    /// L'ouverture de la soirée.
    Ouverture = 3,
    /// L'état du cœur seul.
    Coeur = 4,
}

/// Pourquoi des octets de la garde ne se relisent pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusGarde {
    /// L'en-tête manque : moins de cinq octets.
    Court,
    /// La garde est d'une autre version.
    Version {
        /// Celle des octets.
        lue: u32,
        /// Celle de ce cœur, [`VERSION_GARDE`].
        attendue: u32,
    },
    /// Le message n'est pas de la sorte attendue.
    Sorte {
        /// L'octet lu.
        lue: u8,
        /// La sorte attendue.
        attendue: SorteGarde,
    },
    /// Le contenu ne se lit pas : forme, longueur, valeur hors de son domaine.
    Illisible(ErreurFil),
    /// Un instantané ne se relit pas, d'après l'instance qui le connaît.
    Instantane(String),
    /// Les octets se lisent, mais ne forment pas une garde : un point qui en cite un
    /// autre hors de la table, ou plus haut que lui ; un morceau de journal qui ne suit
    /// pas le journal qu'il prolonge.
    Incoherente(&'static str),
}

impl From<ErreurFil> for RefusGarde {
    fn from(e: ErreurFil) -> RefusGarde {
        RefusGarde::Illisible(e)
    }
}

/// Un instantané d'instance que la garde sait écrire et relire : la mémoire, les
/// globales et la taille d'un module (`moteur-commun`), l'état du modèle des tests.
/// Ses octets sont les siens ; la garde les range sans les lire.
pub trait InstantaneGarde: Sized {
    /// Écrit l'instantané à la fin de `dans`.
    fn ecrire(&self, dans: &mut Vec<u8>);

    /// Relit un instantané écrit par [`InstantaneGarde::ecrire`], strictement.
    fn lire(octets: &[u8]) -> Result<Self, RefusGarde>;
}

// ---------------------------------------------------------------------------
// L'en-tête
// ---------------------------------------------------------------------------

fn entete(sorte: SorteGarde) -> Vec<u8> {
    let mut o = Vec::new();
    o.extend_from_slice(&VERSION_GARDE.to_le_bytes());
    o.push(sorte as u8);
    o
}

/// Le contenu après l'en-tête, si l'en-tête est de cette version et de cette sorte.
fn contenu(octets: &[u8], attendue: SorteGarde) -> Result<&[u8], RefusGarde> {
    let Some((tete, reste)) = octets.split_first_chunk::<5>() else {
        return Err(RefusGarde::Court);
    };
    let [a, b, c, d, sorte] = *tete;
    let lue = u32::from_le_bytes([a, b, c, d]);
    if lue != VERSION_GARDE {
        return Err(RefusGarde::Version {
            lue,
            attendue: VERSION_GARDE,
        });
    }
    if sorte != attendue as u8 {
        return Err(RefusGarde::Sorte {
            lue: sorte,
            attendue,
        });
    }
    Ok(reste)
}

/// L'en-tête puis le contenu en postcard. Une valeur que postcard ne sait pas écrire
/// n'existe pas parmi les types gardés : l'encodage n'échoue pas.
fn ecrire<T: Serialize + ?Sized>(sorte: SorteGarde, v: &T) -> Vec<u8> {
    let mut o = entete(sorte);
    let debut = o.len();
    if jeu_abi::encoder(v, &mut o).is_err() {
        // Inatteignable pour les types de ce module ; un message vide d'après
        // l'en-tête se refuse à la relecture plutôt que de paniquer ici.
        o.truncate(debut);
    }
    o
}

// ---------------------------------------------------------------------------
// Le journal
// ---------------------------------------------------------------------------

/// Un journal sur le fil : sa base, son dernier pas définitif, ses lignes.
#[derive(Serialize, Deserialize)]
struct JournalFil {
    base: u64,
    pas: Pas,
    lignes: Vec<Ligne>,
}

/// Les lignes d'un journal, de sa base à sa fin, avec son dernier pas définitif.
pub fn encoder_journal(j: &Journal) -> Vec<u8> {
    encoder_lignes(j.base(), j.pas(), j.lignes())
}

/// Un morceau de journal : ces lignes, la première d'indice absolu `base`, écrites
/// quand le dernier pas définitif était `pas`. Une case de journal de la garde en est
/// un (PHASE-3, § 2.4) ; [`raccorder`] les remet bout à bout.
pub fn encoder_lignes(base: usize, pas: Pas, lignes: &[Ligne]) -> Vec<u8> {
    #[derive(Serialize)]
    struct Emprunte<'a> {
        base: u64,
        pas: Pas,
        lignes: &'a [Ligne],
    }
    ecrire(
        SorteGarde::Journal,
        &Emprunte {
            base: u64::try_from(base).unwrap_or(u64::MAX),
            pas,
            lignes,
        },
    )
}

/// Relit un journal, ou un morceau de journal.
pub fn decoder_journal(octets: &[u8]) -> Result<Journal, RefusGarde> {
    let f: JournalFil = decoder(contenu(octets, SorteGarde::Journal)?)?;
    let base = usize::try_from(f.base).map_err(|_| RefusGarde::Incoherente("base trop grande"))?;
    Ok(Journal::depuis(base, f.lignes, f.pas))
}

/// Prolonge `journal` par `suite`, le morceau qui commence là où il finit : ses lignes
/// s'ajoutent, et le dernier pas définitif devient le plus grand des deux. Un morceau
/// qui ne commence pas à la fin du journal est refusé, sans rien changer.
pub fn raccorder(journal: &mut Journal, suite: Journal) -> Result<(), RefusGarde> {
    if suite.base() != journal.fin() {
        return Err(RefusGarde::Incoherente(
            "le morceau ne commence pas à la fin du journal",
        ));
    }
    journal.pas = journal.pas.max(suite.pas());
    journal.lignes.extend(suite.lignes);
    Ok(())
}

// ---------------------------------------------------------------------------
// L'ouverture et l'état du cœur
// ---------------------------------------------------------------------------

/// L'ouverture de la soirée.
pub fn encoder_ouverture(o: &Ouverture) -> Vec<u8> {
    ecrire(SorteGarde::Ouverture, o)
}

/// Relit une ouverture.
pub fn decoder_ouverture(octets: &[u8]) -> Result<Ouverture, RefusGarde> {
    Ok(decoder(contenu(octets, SorteGarde::Ouverture)?)?)
}

/// L'état du cœur seul.
pub fn encoder_coeur(c: &EtatCoeur) -> Vec<u8> {
    ecrire(SorteGarde::Coeur, c)
}

/// Relit un état du cœur.
pub fn decoder_coeur(octets: &[u8]) -> Result<EtatCoeur, RefusGarde> {
    Ok(decoder(contenu(octets, SorteGarde::Coeur)?)?)
}

// ---------------------------------------------------------------------------
// Le point de reprise
// ---------------------------------------------------------------------------

/// Un point de la table : son instantané par son rang, son cœur, son indice au journal,
/// et ses points cités par leur rang dans la table.
#[derive(Serialize, Deserialize)]
struct PointPlat {
    instantane: u32,
    coeur: EtatCoeur,
    indice: u64,
    sain: Option<u32>,
    periodique: Option<u32>,
}

/// Un point de reprise sur le fil : les instantanés distincts, puis les points, le
/// premier étant celui qu'on garde.
#[derive(Serialize, Deserialize)]
struct PointFil {
    instantanes: Vec<Vec<u8>>,
    points: Vec<PointPlat>,
}

/// Le rang de `v` dans `table`, ajouté s'il n'y est pas.
fn rang_de<T>(table: &mut Vec<T>, v: T, meme: impl Fn(&T, &T) -> bool) -> u32 {
    let i = table.iter().position(|t| meme(t, &v)).unwrap_or_else(|| {
        table.push(v);
        table.len().saturating_sub(1)
    });
    u32::try_from(i).unwrap_or(u32::MAX)
}

/// Un point de reprise, sa chaîne aplatie et dédoublonnée.
///
/// Deux instantanés égaux ne s'écrivent qu'une fois. L'égalité de l'instantané d'un
/// module compare d'abord les adresses de sa mémoire partagée (`Arc`) : c'est le cas
/// courant, un point et le dernier point à intervalle pris au même pas.
pub fn encoder_point<S: InstantaneGarde + PartialEq>(p: &PointDeReprise<S>) -> Vec<u8> {
    // Les points à écrire, dans l'ordre de la table : l'ordre postfixe d'un parcours
    // en profondeur, retourné, met chaque point avant ceux qu'il cite, et le point
    // lui-même en tête.
    let mut points: Vec<&PointDeReprise<S>> = Vec::new();
    visiter(p, &mut points);
    points.reverse();
    let rang_du_point = |q: &PointDeReprise<S>| {
        points
            .iter()
            .position(|r| core::ptr::eq(*r, q))
            .and_then(|i| u32::try_from(i).ok())
    };
    let mut instantanes: Vec<&S> = Vec::new();
    let mut plats = Vec::with_capacity(points.len());
    for q in &points {
        let instantane = rang_de(&mut instantanes, &q.instantane, |a, b| *a == *b);
        plats.push(PointPlat {
            instantane,
            coeur: q.coeur.clone(),
            indice: u64::try_from(q.indice).unwrap_or(u64::MAX),
            sain: q.sain.as_deref().and_then(rang_du_point),
            periodique: q.periodique.as_deref().and_then(rang_du_point),
        });
    }
    let instantanes = instantanes
        .into_iter()
        .map(|i| {
            let mut o = Vec::new();
            i.ecrire(&mut o);
            o
        })
        .collect();
    ecrire(
        SorteGarde::Point,
        &PointFil {
            instantanes,
            points: plats,
        },
    )
}

/// Ajoute à `vus` les points que `q` cite, puis `q`, chacun une fois.
fn visiter<'a, S>(q: &'a PointDeReprise<S>, vus: &mut Vec<&'a PointDeReprise<S>>) {
    if vus.iter().any(|r| core::ptr::eq(*r, q)) {
        return;
    }
    for cite in [&q.sain, &q.periodique].into_iter().flatten() {
        visiter(cite, vus);
    }
    vus.push(q);
}

/// Relit un point de reprise, sa chaîne reconstruite : un point cité deux fois est le
/// même `Arc`.
pub fn decoder_point<S: InstantaneGarde + Clone>(
    octets: &[u8],
) -> Result<PointDeReprise<S>, RefusGarde> {
    let f: PointFil = decoder(contenu(octets, SorteGarde::Point)?)?;
    let instantanes: Vec<S> = f
        .instantanes
        .iter()
        .map(|o| S::lire(o))
        .collect::<Result<_, _>>()?;
    if f.points.is_empty() {
        return Err(RefusGarde::Incoherente("un point sans point"));
    }
    // Du dernier au premier : un point ne cite que des points plus loin dans la table,
    // déjà construits.
    let n = f.points.len();
    let mut construits: Vec<Option<Arc<PointDeReprise<S>>>> = alloc::vec![None; n];
    let mut premier = None;
    for (i, plat) in f.points.into_iter().enumerate().rev() {
        let cite = |r: Option<u32>| -> Result<Option<Arc<PointDeReprise<S>>>, RefusGarde> {
            let Some(r) = r else {
                return Ok(None);
            };
            let r = usize::try_from(r).map_err(|_| RefusGarde::Incoherente("rang illisible"))?;
            if r <= i {
                return Err(RefusGarde::Incoherente(
                    "un point cite un point qui n'est pas plus loin que lui",
                ));
            }
            construits
                .get(r)
                .cloned()
                .flatten()
                .map(Some)
                .ok_or(RefusGarde::Incoherente(
                    "un point cite un point hors de la table",
                ))
        };
        let instantane = usize::try_from(plat.instantane)
            .ok()
            .and_then(|k| instantanes.get(k))
            .cloned()
            .ok_or(RefusGarde::Incoherente(
                "un point cite un instantané hors de la table",
            ))?;
        let point = PointDeReprise {
            instantane,
            coeur: plat.coeur,
            indice: usize::try_from(plat.indice)
                .map_err(|_| RefusGarde::Incoherente("indice trop grand"))?,
            sain: cite(plat.sain)?,
            periodique: cite(plat.periodique)?,
        };
        if i == 0 {
            premier = Some(point);
        } else if let Some(case) = construits.get_mut(i) {
            *case = Some(Arc::new(point));
        }
    }
    premier.ok_or(RefusGarde::Incoherente("un point sans point"))
}
