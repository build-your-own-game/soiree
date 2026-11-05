//! E9, entrée par entrée : le journal d'une soirée rejoué sans le cœur, sur une
//! instance jumelle du module et, si elle est là, sur la copie native (§ 7.2 ; § 8.3,
//! `colle_natif_et_module_identiques`).
//!
//! Pour chaque partie du journal : `Bonjour`, `Decrire`, le `Creer` que la sonde a
//! gardé, puis chaque entrée que la partie n'a pas écartée. Après chacune :
//!
//! - la jumelle rend les mêmes octets que la définitive, et a la même empreinte
//!   qu'elle après cette entrée (deux exécutions du module) ;
//! - la copie native rend les mêmes octets que la jumelle, la même forme canonique,
//!   le même état et les mêmes vues.

use jeu_abi::{Appel, RegardFil, SorteAppel, encoder};
use moteur_wasmtime::ModuleCharge;
use partie::{Echec, Fabrique, Instance, IssueJournal, Ligne};

use crate::dynamique::{FabriqueDynamique, InstanceDynamique};
use crate::soiree::{BUDGETS, Bilan};
use crate::sonde::{Compte, hacher, recoller};

/// Les regards dont les vues se comparent, au plus.
const REGARDS_MAX: usize = 10;

fn encode(a: &Appel<'_>) -> Vec<u8> {
    let mut o = Vec::new();
    match encoder(a, &mut o) {
        Ok(()) => o,
        Err(_) => Vec::new(),
    }
}

/// Une instance nue, quelle qu'elle soit.
trait Nue {
    fn appel(&mut self, a: &Appel<'_>) -> Result<Vec<u8>, Echec>;
    fn empreinte_nue(&mut self) -> Option<[u8; 32]>;
}

impl Nue for moteur_wasmtime::InstanceWasm {
    fn appel(&mut self, a: &Appel<'_>) -> Result<Vec<u8>, Echec> {
        Instance::appeler(self, a.sorte(), &encode(a), budget(a.sorte()))
    }
    fn empreinte_nue(&mut self) -> Option<[u8; 32]> {
        Instance::empreinte(self).ok()
    }
}

impl Nue for Box<dyn InstanceDynamique> {
    fn appel(&mut self, a: &Appel<'_>) -> Result<Vec<u8>, Echec> {
        self.as_mut()
            .appeler(a.sorte(), &encode(a), budget(a.sorte()))
    }
    fn empreinte_nue(&mut self) -> Option<[u8; 32]> {
        None
    }
}

const fn budget(s: SorteAppel) -> u64 {
    match s {
        SorteAppel::Lecture => BUDGETS.lecture,
        SorteAppel::Ecriture => BUDGETS.entree,
    }
}

fn canonique(n: &mut dyn Nue) -> Result<Vec<u8>, Echec> {
    recoller(|morceau| n.appel(&Appel::Canonique { morceau }))
}

/// Ce que le rejeu a vu.
#[derive(Debug, Default)]
pub(crate) struct Constat {
    pub(crate) e9: Compte,
    pub(crate) pannes: Vec<String>,
    pub(crate) entrees: u64,
}

/// Rejoue le journal de la soirée entrée par entrée.
#[expect(
    clippy::too_many_lines,
    reason = "le rejeu se lit d'un trait : accueil, création, puis chaque entrée et ses comparaisons"
)]
pub(crate) fn rejouer(
    module: &ModuleCharge,
    mut natif: Option<&mut dyn FabriqueDynamique>,
    bilan: &Bilan,
) -> Constat {
    let mut c = Constat::default();
    let obs = bilan.obs.borrow();
    let regards: Vec<RegardFil> = bilan.regards.iter().copied().take(REGARDS_MAX).collect();
    let lignes = bilan.journal.lignes();
    for (partie, creer) in &obs.creations {
        let mut jumelle = match Fabrique::neuve(&mut module.fabrique()) {
            Ok(i) => i,
            Err(p) => {
                c.pannes.push(format!("E9, instance jumelle : {p}"));
                return c;
            }
        };
        let mut copie: Option<Box<dyn InstanceDynamique>> = match natif.as_deref_mut() {
            Some(f) => match FabriqueDynamique::neuve(f) {
                Ok(i) => Some(i),
                Err(p) => {
                    c.pannes.push(format!("E9, copie native : {p}"));
                    return c;
                }
            },
            None => None,
        };
        for a in [Appel::Bonjour, Appel::Decrire] {
            let rj = jumelle.appel(&a);
            let rn = copie.as_mut().map(|n| n.appel(&a));
            if let Some(rn) = rn
                && matches!(a, Appel::Decrire)
            {
                c.e9.evaluer();
                if rn.as_ref().ok() != rj.as_ref().ok() {
                    c.e9.echouer(String::from(
                        "la copie native ne décrit pas le jeu comme le module",
                    ));
                }
            }
        }
        let rj = Instance::appeler(&mut jumelle, SorteAppel::Ecriture, creer, BUDGETS.creation);
        if let Some(n) = copie.as_mut() {
            let rn = n
                .as_mut()
                .appeler(SorteAppel::Ecriture, creer, BUDGETS.creation);
            c.e9.evaluer();
            if rn.as_ref().ok() != rj.as_ref().ok() {
                c.e9.echouer(format!(
                    "partie {partie} : la copie native ne crée pas l'état comme le module"
                ));
                copie = None;
            }
        }
        // Les entrées appliquées aux deux instances, par leur ligne du journal ; la
        // partie de la dernière entrée vue.
        let mut appliquees: Vec<usize> = Vec::new();
        let mut courante = None;
        for (rang, l) in lignes.iter().enumerate() {
            let e = match l {
                Ligne::Entree(e) => e,
                // Une reprise (§ 4.6) de cette partie : la définitive est revenue à son
                // point sain ; les deux instances y reviennent, en rejouant depuis la
                // création les entrées qui le précèdent.
                Ligne::Reprise { depuis, .. } if courante == Some(*partie) => {
                    appliquees.retain(|k| k < depuis);
                    match relancer(module, natif.as_deref_mut(), creer, lignes, &appliquees) {
                        Ok((j, n)) => {
                            jumelle = j;
                            if copie.is_some() {
                                copie = n;
                            }
                        }
                        Err(p) => {
                            c.pannes
                                .push(format!("E9, partie {partie}, après une reprise : {p}"));
                            break;
                        }
                    }
                    continue;
                }
                Ligne::Reprise { .. }
                | Ligne::Arrivees { .. }
                | Ligne::Empreinte { .. }
                | Ligne::Saut { .. }
                | Ligne::Pause { .. }
                | Ligne::Vue { .. }
                | Ligne::Partie { .. } => continue,
            };
            courante = Some(e.partie);
            if e.partie != *partie || matches!(e.issue, IssueJournal::Ecartee { .. }) {
                continue;
            }
            appliquees.push(rang);
            let appel = Appel::Entree {
                ordre: e.ordre,
                place: e.place.numero(),
                entree: &e.octets,
            };
            let ou = format!(
                "partie {partie}, entrée d'ordre {} au pas {}",
                e.ordre,
                e.place.numero()
            );
            c.entrees = c.entrees.saturating_add(1);
            c.e9.evaluer();
            let rj = match jumelle.appel(&appel) {
                Ok(r) => r,
                Err(err) => {
                    c.e9.echouer(format!(
                        "{ou} : la jumelle échoue ({err}) là où la partie a réussi"
                    ));
                    break;
                }
            };
            if let Some(attendu) = obs.entrees.get(&(*partie, e.ordre)) {
                if attendu.retour != rj {
                    c.e9.echouer(format!(
                        "{ou} : deux exécutions du module ne rendent pas les mêmes octets"
                    ));
                    break;
                }
                if jumelle.empreinte_nue() != Some(attendu.empreinte) {
                    c.e9.echouer(format!(
                        "{ou} : deux exécutions du module n'ont pas la même empreinte"
                    ));
                    break;
                }
            }
            let Some(n) = copie.as_mut() else {
                continue;
            };
            let diverge = match n.appel(&appel) {
                Err(err) => Some(format!("la copie native échoue ({err})")),
                Ok(rn) if rn != rj => Some(String::from(
                    "la copie native ne rend pas les mêmes octets que le module",
                )),
                Ok(_) => comparer_lectures(&mut jumelle, n.as_mut(), &regards),
            };
            if let Some(d) = diverge {
                c.e9.echouer(format!("{ou} : {d}"));
                copie = None;
            }
        }
    }
    c
}

/// Deux instances neuves, la jumelle et la copie native, accueillies, créées par
/// `creer`, puis menées par les entrées de ces lignes du journal, sans comparaison :
/// celles que la partie avait jouées avant le point sain d'une reprise.
fn relancer<'n>(
    module: &ModuleCharge,
    natif: Option<&mut (dyn FabriqueDynamique + 'n)>,
    creer: &[u8],
    lignes: &[Ligne],
    entrees: &[usize],
) -> Result<
    (
        moteur_wasmtime::InstanceWasm,
        Option<Box<dyn InstanceDynamique>>,
    ),
    String,
> {
    let mut jumelle = Fabrique::neuve(&mut module.fabrique()).map_err(|p| p.to_string())?;
    let mut copie = match natif {
        Some(f) => Some(FabriqueDynamique::neuve(f).map_err(|p| p.to_string())?),
        None => None,
    };
    let mut appels: Vec<(SorteAppel, Vec<u8>, u64)> = vec![
        (
            SorteAppel::Lecture,
            encode(&Appel::Bonjour),
            BUDGETS.lecture,
        ),
        (
            SorteAppel::Lecture,
            encode(&Appel::Decrire),
            BUDGETS.lecture,
        ),
        (SorteAppel::Ecriture, creer.to_vec(), BUDGETS.creation),
    ];
    for k in entrees {
        if let Some(Ligne::Entree(e)) = lignes.get(*k) {
            appels.push((
                SorteAppel::Ecriture,
                encode(&Appel::Entree {
                    ordre: e.ordre,
                    place: e.place.numero(),
                    entree: &e.octets,
                }),
                BUDGETS.entree,
            ));
        }
    }
    for (sorte, octets, budget) in &appels {
        let _: Vec<u8> = Instance::appeler(&mut jumelle, *sorte, octets, *budget).map_err(|e| {
            format!("la jumelle ne refait pas une entrée d'avant le point sain : {e}")
        })?;
        if let Some(n) = copie.as_mut() {
            let _: Vec<u8> = n.as_mut().appeler(*sorte, octets, *budget).map_err(|e| {
                format!("la copie native ne refait pas une entrée d'avant le point sain : {e}")
            })?;
        }
    }
    Ok((jumelle, copie))
}

/// Après une entrée : même forme canonique, même état, mêmes vues.
fn comparer_lectures(
    jumelle: &mut moteur_wasmtime::InstanceWasm,
    natif: &mut dyn InstanceDynamique,
    regards: &[RegardFil],
) -> Option<String> {
    let mut n = NueDyn(natif);
    match (canonique(jumelle), canonique(&mut n)) {
        (Ok(a), Ok(b)) if hacher(&a) == hacher(&b) => {}
        (Ok(_), Ok(_)) => {
            return Some(String::from(
                "la forme canonique de la copie native diffère",
            ));
        }
        (a, b) => {
            return Some(format!(
                "forme canonique illisible : module {:?}, copie native {:?}",
                a.err(),
                b.err()
            ));
        }
    }
    for a in [
        Appel::Etat {
            connues: Vec::new(),
        },
        Appel::Regarder {
            regards: regards.to_vec(),
        },
    ] {
        if jumelle.appel(&a).ok() != n.appel(&a).ok() {
            return Some(format!(
                "{} : la copie native ne rend pas ce que rend le module",
                crate::sonde::nom_appel(&a)
            ));
        }
    }
    None
}

struct NueDyn<'a>(&'a mut dyn InstanceDynamique);

impl Nue for NueDyn<'_> {
    fn appel(&mut self, a: &Appel<'_>) -> Result<Vec<u8>, Echec> {
        self.0.appeler(a.sorte(), &encode(a), budget(a.sorte()))
    }
    fn empreinte_nue(&mut self) -> Option<[u8; 32]> {
        None
    }
}
