//! La sonde : une fabrique et une instance qui en enveloppent d'autres, et regardent
//! chaque appel que le cœur fait sur la définitive.
//!
//! Le cœur (`partie`) ne sait rien de l'épreuve. La sonde se glisse entre lui et
//! l'instance, sans rien changer à ce qu'il voit, et constate :
//!
//! - **E4** : après chaque entrée acceptée, `Transition` entre les résumés d'avant et
//!   d'après, puis `Verifier` ;
//! - **E6** : avant chaque entrée, la forme canonique et les vues de tous les
//!   regards ; après un refus ou un `ignorer`, les mêmes, octet pour octet ;
//! - **E7** : avant et après chaque lecture, l'empreinte des zones d'état (§ 3.2) ;
//! - pour E9 et E14 : l'appel `Creer` de chaque partie, et, après chaque entrée, le
//!   retour et l'empreinte de l'instance.
//!
//! - **E18** : après chaque entrée acceptée, la phase, et si l'entrée était un `Temps` ;
//! - **E19** : le carburant de chaque appel du cœur, par sorte, et celui de chaque pas.
//!
//! L'épreuve appelle aussi la définitive elle-même ([`Sondee::lire`]) : bots, état,
//! projections. Ces lectures passent par les mêmes constats.
//!
//! La tête (§ 4.2), qu'une fenêtre non nulle fait naître, passe par la sonde sans
//! constat : elle rejoue ce que la définitive jouera, et c'est la définitive que les
//! contrôles jugent.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use jeu_abi::{Appel, ConstatFil, IssueFil, Oui, RegardFil, Retour, SorteAppel, decoder, encoder};
use partie::{CausePanne, Echec, Fabrique, Faute, Instance, PanneOutil};

/// Le plus de constats détaillés gardés par contrôle : le reste se compte.
const DETAILS_MAX: usize = 8;

/// Ce que la sonde vérifie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Verifs {
    pub(crate) e4: Oui,
    pub(crate) e6: Oui,
    pub(crate) e7: Oui,
    /// Garder le `Creer` de chaque partie, et le retour et l'empreinte de chaque
    /// entrée.
    pub(crate) releve: Oui,
    /// E18 : la phase après chaque entrée acceptée.
    pub(crate) phases: Oui,
    /// E19 : le carburant de chaque appel du cœur.
    pub(crate) budgets: Oui,
}

impl Verifs {
    pub(crate) const RIEN: Verifs = Verifs {
        e4: Oui::Non,
        e6: Oui::Non,
        e7: Oui::Non,
        releve: Oui::Non,
        phases: Oui::Non,
        budgets: Oui::Non,
    };
}

/// La phase après une entrée acceptée (E18).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PhaseVue {
    /// La partie.
    pub(crate) partie: u32,
    /// Le pas de l'entrée.
    pub(crate) place: u64,
    /// L'entrée était-elle un `Temps` ?
    pub(crate) temps: Oui,
    /// Le temps jeté que ce `Temps` portait, en pas.
    pub(crate) saut: u64,
    /// L'indice de la phase après l'entrée.
    pub(crate) phase: u16,
}

/// Les constats d'un contrôle : combien d'évaluations, et ce qui a échoué.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Compte {
    pub(crate) evaluations: u64,
    pub(crate) echecs: u64,
    pub(crate) details: Vec<String>,
}

impl Compte {
    pub(crate) fn evaluer(&mut self) {
        self.evaluations = self.evaluations.saturating_add(1);
    }

    pub(crate) fn echouer(&mut self, detail: String) {
        self.echecs = self.echecs.saturating_add(1);
        if self.details.len() < DETAILS_MAX {
            self.details.push(detail);
        }
    }

    /// Ajoute les constats d'un autre compte.
    pub(crate) fn ajouter(&mut self, autre: &Compte) {
        self.evaluations = self.evaluations.saturating_add(autre.evaluations);
        self.echecs = self.echecs.saturating_add(autre.echecs);
        for d in &autre.details {
            if self.details.len() < DETAILS_MAX {
                self.details.push(d.clone());
            }
        }
    }
}

/// Ce que la sonde a relevé d'une entrée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Releve {
    /// Le retour de l'instance, octet pour octet.
    pub(crate) retour: Vec<u8>,
    /// L'empreinte de l'instance juste après.
    pub(crate) empreinte: [u8; 32],
}

/// La première entrée qu'un rejeu ne retrouve pas : même partie, même ordre, mais
/// un autre retour ou une autre empreinte. Seules les entrées des deux relevés se
/// comparent.
pub(crate) fn premiere_entree_differente(
    reference: &BTreeMap<(u32, u64), Releve>,
    rejoue: &BTreeMap<(u32, u64), Releve>,
) -> Option<(u32, u64)> {
    rejoue
        .iter()
        .find(|(cle, r)| reference.get(cle).is_some_and(|a| a != *r))
        .map(|(cle, _)| *cle)
}

/// Ce que les instances d'une sonde partagent.
#[derive(Debug)]
pub(crate) struct Obs {
    pub(crate) verifs: Verifs,
    /// Les regards dont E6 compare les vues.
    pub(crate) regards: Vec<RegardFil>,
    /// Le carburant des lectures de la sonde.
    pub(crate) budget: u64,
    pub(crate) e4: Compte,
    /// Le nombre de fois que chaque invariant a été évalué, par nom (OUT-01).
    pub(crate) invariants: BTreeMap<String, u64>,
    pub(crate) e6: Compte,
    pub(crate) e7: Compte,
    /// Les fautes du jeu pendant les lectures de l'épreuve elle-même : la
    /// définitive n'est plus sûre, et la soirée s'arrête.
    pub(crate) fautes: Vec<(String, Faute)>,
    /// Les pannes de l'outil vues par la sonde.
    pub(crate) pannes: Vec<PanneOutil>,
    /// Les entrées acceptées.
    pub(crate) acceptees: u64,
    /// Le retour et l'empreinte de chaque entrée réussie, par partie et par ordre.
    pub(crate) entrees: BTreeMap<(u32, u64), Releve>,
    /// L'appel `Creer` encodé de chaque partie.
    pub(crate) creations: BTreeMap<u32, Vec<u8>>,
    /// Le numéro de la prochaine partie créée.
    pub(crate) prochaine_partie: u32,
    /// La partie d'une instance restaurée sans `Creer` (une reprise).
    pub(crate) partie_reprise: u32,
    /// L'empreinte de la définitive après sa dernière écriture réussie.
    pub(crate) derniere_ecriture: Option<[u8; 32]>,
    /// E18 : la phase après chaque entrée acceptée, dans l'ordre.
    pub(crate) phases: Vec<PhaseVue>,
    /// E19 : le carburant de chaque appel du cœur, par nom d'appel.
    pub(crate) carburant: BTreeMap<&'static str, Vec<u64>>,
    /// E19 : le pas et le carburant de chaque entrée, par partie et par ordre. Une
    /// entrée rejouée (une reprise, une faute reconstruite) remplace la sienne : elle
    /// ne compte qu'une fois.
    pub(crate) carburant_entrees: BTreeMap<(u32, u64), (u64, u64)>,
}

impl Obs {
    pub(crate) fn new(verifs: Verifs, budget: u64) -> Rc<RefCell<Obs>> {
        Rc::new(RefCell::new(Obs {
            verifs,
            regards: vec![RegardFil::Ecran],
            budget,
            e4: Compte::default(),
            invariants: BTreeMap::new(),
            e6: Compte::default(),
            e7: Compte::default(),
            fautes: Vec::new(),
            pannes: Vec::new(),
            acceptees: 0,
            entrees: BTreeMap::new(),
            creations: BTreeMap::new(),
            prochaine_partie: 0,
            partie_reprise: 0,
            derniere_ecriture: None,
            phases: Vec::new(),
            carburant: BTreeMap::new(),
            carburant_entrees: BTreeMap::new(),
        }))
    }
}

/// Une fabrique sondée.
pub(crate) struct Sonde<F> {
    fabrique: F,
    obs: Rc<RefCell<Obs>>,
}

impl<F> Sonde<F> {
    pub(crate) fn new(fabrique: F, obs: &Rc<RefCell<Obs>>) -> Sonde<F> {
        Sonde {
            fabrique,
            obs: Rc::clone(obs),
        }
    }
}

impl<F: Fabrique> Fabrique for Sonde<F> {
    type I = Sondee<F::I>;

    fn neuve(&mut self) -> Result<Sondee<F::I>, PanneOutil> {
        Ok(Sondee {
            inner: RefCell::new(self.fabrique.neuve()?),
            obs: Rc::clone(&self.obs),
            partie: Cell::new(None),
            connue: Cell::new(None),
            officielle: Cell::new(None),
            tete: Oui::Non,
        })
    }

    fn neuve_tete(&mut self) -> Result<Sondee<F::I>, PanneOutil> {
        let mut i = self.neuve()?;
        i.tete = Oui::Oui;
        Ok(i)
    }
}

/// Une instance sondée.
pub(crate) struct Sondee<I> {
    inner: RefCell<I>,
    obs: Rc<RefCell<Obs>>,
    /// La partie de cette instance, depuis son `Creer`.
    partie: Cell<Option<u32>>,
    /// L'empreinte de l'instance, tant qu'aucune écriture n'a eu lieu depuis.
    connue: Cell<Option<[u8; 32]>>,
    /// L'empreinte juste après la dernière écriture, avant les lectures de la sonde :
    /// celle que le cœur lit et note, jusqu'à son prochain appel. Une instance que la
    /// sonde ne regarde pas aurait celle-là ; une lecture qui écrit (E7) ne déborde
    /// ainsi pas sur la reprise (E14) ni sur le déterminisme (E9).
    officielle: Cell<Option<[u8; 32]>>,
    /// La tête de la partie : ses appels passent sans constat.
    tete: Oui,
}

fn encode(a: &Appel<'_>) -> Result<Vec<u8>, Echec> {
    let mut o = Vec::new();
    encoder(a, &mut o)
        .map_err(|e| Echec::Panne(PanneOutil::new(CausePanne::Interne, e.to_string())))?;
    Ok(o)
}

/// Le nom d'un appel, pour les rapports.
pub(crate) const fn nom_appel(a: &Appel<'_>) -> &'static str {
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

pub(crate) fn hacher(o: &[u8]) -> [u8; 32] {
    *blake3::hash(o).as_bytes()
}

/// Les morceaux d'une forme canonique ou d'une projection, recollés.
pub(crate) fn recoller(
    mut lire: impl FnMut(u32) -> Result<Vec<u8>, Echec>,
) -> Result<Vec<u8>, Echec> {
    let mut tout = Vec::new();
    let mut morceau = 0;
    loop {
        let r = lire(morceau)?;
        match decoder::<Retour<'_>>(&r) {
            Ok(Retour::Morceau { octets, suivant }) => {
                tout.extend_from_slice(octets);
                match suivant {
                    Some(s) if s > morceau => morceau = s,
                    Some(_) => return Err(Echec::Faute(Faute::SortieInvalide)),
                    None => return Ok(tout),
                }
            }
            Ok(Retour::Illisible(e)) => {
                return Err(Echec::Panne(PanneOutil::new(
                    CausePanne::AppelIllisible,
                    e.to_string(),
                )));
            }
            Ok(_) | Err(_) => return Err(Echec::Faute(Faute::SortieInvalide)),
        }
    }
}

impl<I: Instance> Sondee<I> {
    /// L'appel, sans constat.
    fn brut(&self, sorte: SorteAppel, octets: &[u8], budget: u64) -> Result<Vec<u8>, Echec> {
        self.inner.borrow_mut().appeler(sorte, octets, budget)
    }

    /// L'empreinte de l'instance, connue ou calculée.
    fn empreinte_connue(&self) -> Result<[u8; 32], PanneOutil> {
        if let Some(e) = self.connue.get() {
            return Ok(e);
        }
        let e = self.inner.borrow_mut().empreinte()?;
        self.connue.set(Some(e));
        Ok(e)
    }

    /// Une lecture, sous le constat de E7.
    fn lecture(&self, nom: &'static str, octets: &[u8], budget: u64) -> Result<Vec<u8>, Echec> {
        let e7 = self.obs.borrow().verifs.e7 == Oui::Oui;
        let avant = if e7 {
            Some(self.empreinte_connue()?)
        } else {
            None
        };
        let r = self.brut(SorteAppel::Lecture, octets, budget);
        if r.is_err() {
            self.connue.set(None);
            return r;
        }
        if let Some(avant) = avant {
            let apres = self.inner.borrow_mut().empreinte()?;
            self.connue.set(Some(apres));
            let mut obs = self.obs.borrow_mut();
            obs.e7.evaluer();
            if apres != avant {
                obs.e7.echouer(format!(
                    "la lecture {nom} a changé les zones d'état de la mémoire"
                ));
            }
        }
        r
    }

    /// Une lecture de l'épreuve sur cette instance, sous les mêmes constats que celles
    /// du cœur.
    pub(crate) fn lire(&self, a: &Appel<'_>) -> Result<Vec<u8>, Echec> {
        let budget = self.obs.borrow().budget;
        let o = encode(a)?;
        self.lecture(nom_appel(a), &o, budget)
    }

    /// Un instantané de l'instance, pour une copie (E5).
    pub(crate) fn instantane_epreuve(&self) -> Result<I::Instantane, PanneOutil> {
        self.inner.borrow_mut().instantane()
    }

    /// La forme canonique, recollée.
    pub(crate) fn canonique(&self) -> Result<Vec<u8>, Echec> {
        recoller(|morceau| self.lire(&Appel::Canonique { morceau }))
    }

    /// La partie de cette instance.
    pub(crate) fn partie(&self) -> u32 {
        self.partie
            .get()
            .unwrap_or_else(|| self.obs.borrow().partie_reprise)
    }

    /// Ce qu'E6 compare : la forme canonique et les vues de tous les regards.
    fn trace(&self) -> Result<([u8; 32], [u8; 32]), Echec> {
        let regards = self.obs.borrow().regards.clone();
        let c = self.canonique()?;
        let v = self.lire(&Appel::Regarder { regards })?;
        Ok((hacher(&c), hacher(&v)))
    }

    /// Un résumé, pour la transition.
    fn resumer(&self) -> Result<Vec<u8>, Echec> {
        let r = self.lire(&Appel::Resumer)?;
        match decoder::<Retour<'_>>(&r) {
            Ok(Retour::Octets(o)) => Ok(o.to_vec()),
            Ok(_) | Err(_) => Err(Echec::Faute(Faute::SortieInvalide)),
        }
    }

    /// Les constats d'une vérification.
    fn constats(&self, quoi: &str, r: &[u8], place: u64) {
        let mut obs = self.obs.borrow_mut();
        match decoder::<Retour<'_>>(r) {
            Ok(Retour::Verif(v)) => {
                for (nom, c) in v {
                    obs.e4.evaluer();
                    let n = obs.invariants.entry(String::from(nom)).or_insert(0);
                    *n = n.saturating_add(1);
                    if c == ConstatFil::Viole {
                        obs.e4
                            .echouer(format!("{quoi} « {nom} » violé au pas {place}"));
                    }
                }
            }
            Ok(_) | Err(_) => obs.e4.echouer(format!(
                "{quoi} : le retour n'est pas une vérification, au pas {place}"
            )),
        }
    }

    fn noter_faute(&self, ou: &str, e: Echec) {
        let mut obs = self.obs.borrow_mut();
        match e {
            Echec::Faute(f) => obs.fautes.push((String::from(ou), f)),
            Echec::Panne(p) => obs.pannes.push(p),
        }
    }

    /// Une entrée sur la définitive, avec ses constats.
    fn entree(
        &self,
        sorte: SorteAppel,
        octets: &[u8],
        budget: u64,
        (ordre, place): (u64, u64),
    ) -> Result<Vec<u8>, Echec> {
        let verifs = self.obs.borrow().verifs;
        let trace_avant = if verifs.e6 == Oui::Oui {
            match self.trace() {
                Ok(t) => Some(t),
                Err(e) => {
                    self.noter_faute("la trace d'avant une entrée (E6)", e);
                    None
                }
            }
        } else {
            None
        };
        let resume_avant = if verifs.e4 == Oui::Oui {
            match self.resumer() {
                Ok(r) => Some(r),
                Err(e) => {
                    self.noter_faute("le résumé d'avant une entrée (E4)", e);
                    None
                }
            }
        } else {
            None
        };
        self.connue.set(None);
        let r = self.brut(sorte, octets, budget);
        self.noter_carburant_de_l_entree(ordre, place);
        let Ok(retour) = &r else {
            return r;
        };
        let apres_ecriture = self.empreinte_connue().map_err(Echec::Panne)?;
        self.officielle.set(Some(apres_ecriture));
        let issue = match decoder::<Retour<'_>>(retour) {
            Ok(Retour::Issue(IssueFil::Acceptee { .. })) => Some(true),
            Ok(Retour::Issue(IssueFil::Refusee { .. } | IssueFil::Ignoree)) => Some(false),
            Ok(_) | Err(_) => None,
        };
        match issue {
            Some(true) => {
                {
                    let mut obs = self.obs.borrow_mut();
                    obs.acceptees = obs.acceptees.saturating_add(1);
                }
                if verifs.phases == Oui::Oui {
                    self.noter_phase(octets, place);
                }
                if let Some(avant) = resume_avant {
                    match self.resumer() {
                        Ok(apres) => {
                            let t = Appel::Transition {
                                avant: &avant,
                                apres: &apres,
                            };
                            match self.lire(&t) {
                                Ok(v) => self.constats("la transition", &v, place),
                                Err(e) => self.noter_faute("la transition (E4)", e),
                            }
                        }
                        Err(e) => self.noter_faute("le résumé d'après une entrée (E4)", e),
                    }
                    match self.lire(&Appel::Verifier) {
                        Ok(v) => self.constats("l'invariant", &v, place),
                        Err(e) => self.noter_faute("les invariants (E4)", e),
                    }
                }
            }
            Some(false) => {
                if let Some(avant) = trace_avant {
                    match self.trace() {
                        Ok(apres) => {
                            let mut obs = self.obs.borrow_mut();
                            obs.e6.evaluer();
                            if apres.0 != avant.0 {
                                obs.e6.echouer(format!(
                                    "un refus a changé la forme canonique, au pas {place} (ordre {ordre})"
                                ));
                            } else if apres.1 != avant.1 {
                                obs.e6.echouer(format!(
                                    "un refus a changé une vue, au pas {place} (ordre {ordre})"
                                ));
                            }
                        }
                        Err(e) => self.noter_faute("la trace d'après un refus (E6)", e),
                    }
                }
            }
            None => {}
        }
        if verifs.releve == Oui::Oui {
            let partie = self.partie();
            let mut obs = self.obs.borrow_mut();
            obs.derniere_ecriture = Some(apres_ecriture);
            let _: Option<Releve> = obs.entrees.insert(
                (partie, ordre),
                Releve {
                    retour: retour.clone(),
                    empreinte: apres_ecriture,
                },
            );
        }
        r
    }

    /// E19 : le carburant du dernier appel brut, sous son nom.
    fn noter_carburant(&self, nom: &'static str) {
        let carburant = self.inner.borrow().carburant();
        let mut obs = self.obs.borrow_mut();
        if obs.verifs.budgets == Oui::Oui
            && let Some(c) = carburant
        {
            obs.carburant.entry(nom).or_default().push(c);
        }
    }

    /// E19 : le carburant d'une entrée, avec son pas.
    fn noter_carburant_de_l_entree(&self, ordre: u64, place: u64) {
        let carburant = self.inner.borrow().carburant();
        let partie = self.partie();
        let mut obs = self.obs.borrow_mut();
        if obs.verifs.budgets == Oui::Oui
            && let Some(c) = carburant
        {
            let _: Option<(u64, u64)> = obs.carburant_entrees.insert((partie, ordre), (place, c));
        }
    }

    /// E18 : la phase après une entrée acceptée, et ce qu'était l'entrée.
    fn noter_phase(&self, octets: &[u8], place: u64) {
        let (temps, saut) = match decoder::<Appel<'_>>(octets) {
            Ok(Appel::Entree { entree, .. }) => match decoder::<jeu_abi::EntreeFil<'_>>(entree) {
                Ok(jeu_abi::EntreeFil::Temps { saut, .. }) => (Oui::Oui, saut),
                Ok(_) | Err(_) => (Oui::Non, 0),
            },
            Ok(_) | Err(_) => (Oui::Non, 0),
        };
        let etat = self.lire(&Appel::Etat {
            connues: Vec::new(),
        });
        let phase = match etat.as_deref().map(decoder::<Retour<'_>>) {
            Ok(Ok(Retour::Etat(e))) => e.phase_indice,
            Ok(Ok(_) | Err(_)) | Err(_) => return,
        };
        let partie = self.partie();
        self.obs.borrow_mut().phases.push(PhaseVue {
            partie,
            place,
            temps,
            saut,
            phase,
        });
    }

    /// `Creer` : la partie de l'instance, et son appel gardé.
    fn creer(&self, octets: &[u8], budget: u64) -> Result<Vec<u8>, Echec> {
        self.connue.set(None);
        let r = self.brut(SorteAppel::Ecriture, octets, budget);
        if let Ok(retour) = &r
            && matches!(
                decoder::<Retour<'_>>(retour),
                Ok(Retour::Issue(IssueFil::Acceptee { .. }))
            )
        {
            let releve = self.obs.borrow().verifs.releve == Oui::Oui;
            let partie = {
                let mut obs = self.obs.borrow_mut();
                let p = obs.prochaine_partie;
                obs.prochaine_partie = p.saturating_add(1);
                if releve {
                    let _: Option<Vec<u8>> = obs.creations.insert(p, octets.to_vec());
                }
                p
            };
            self.partie.set(Some(partie));
            let e = self.empreinte_connue().map_err(Echec::Panne)?;
            self.officielle.set(Some(e));
            if releve {
                self.obs.borrow_mut().derniere_ecriture = Some(e);
            }
        }
        r
    }
}

impl<I: Instance> Instance for Sondee<I> {
    type Instantane = I::Instantane;

    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8], budget: u64) -> Result<Vec<u8>, Echec> {
        self.officielle.set(None);
        if self.tete == Oui::Oui {
            self.connue.set(None);
            return self.brut(sorte, appel, budget);
        }
        let decode = decoder::<Appel<'_>>(appel);
        let nom = decode.as_ref().map_or("illisible", nom_appel);
        let r = match decode {
            // L'entrée note son carburant elle-même, avant les lectures de ses constats.
            Ok(Appel::Entree { ordre, place, .. }) => {
                return self.entree(sorte, appel, budget, (ordre, place));
            }
            Ok(Appel::Creer { .. }) => self.creer(appel, budget),
            Ok(a) if a.sorte() == SorteAppel::Lecture => self.lecture(nom_appel(&a), appel, budget),
            Ok(_) | Err(_) => {
                self.connue.set(None);
                self.brut(sorte, appel, budget)
            }
        };
        self.noter_carburant(nom);
        r
    }

    fn carburant(&self) -> Option<u64> {
        self.inner.borrow().carburant()
    }

    fn instantane(&mut self) -> Result<I::Instantane, PanneOutil> {
        self.inner.borrow_mut().instantane()
    }

    fn restaurer(&mut self, i: &I::Instantane) -> Result<(), PanneOutil> {
        self.connue.set(None);
        self.officielle.set(None);
        self.inner.borrow_mut().restaurer(i)
    }

    fn empreinte(&mut self) -> Result<[u8; 32], PanneOutil> {
        match self.officielle.get() {
            Some(e) => Ok(e),
            None => self.empreinte_connue(),
        }
    }
}
