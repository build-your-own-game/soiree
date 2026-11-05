//! La sonde : une fabrique et une instance qui en enveloppent d'autres, et regardent
//! chaque appel que le cœur fait sur la définitive.
//!
//! Le cœur (`partie`) ne sait rien de l'épreuve. La sonde se glisse entre lui et
//! l'instance, sans rien changer à ce qu'il voit, et constate :
//!
//! - **E4** : après chaque entrée acceptée, `Transition` entre les résumés d'avant et
//!   d'après, puis `Verifier` ;
//! - **E6** : avant chaque entrée, la forme canonique et les vues de tous les
//!   regards ; après un refus ou un `ignorer`, les mêmes, octet pour octet. Puis
//!   « même avenir » : l'entrée acceptée qui suit des refus se rejoue sur
//!   l'instantané pris avant le premier d'entre eux, et doit rendre le même retour,
//!   la même forme canonique et les mêmes vues. Un `static` écrit dans `Avant`, que ni
//!   la forme canonique ni les vues ne montrent au refus, se voit là dès qu'il change
//!   ce qu'une entrée acceptée fait (RESTE-PHASE-1, point 11) ;
//! - **E7** : avant et après chaque lecture, l'empreinte des zones d'état (§ 3.2) ;
//! - **E8**, la variation : chaque geste ou tenu accepté d'un téléphone se rejoue sur
//!   l'instantané d'avant lui, avec un autre dernier octet. Quand ce seul octet change
//!   un secret que ne voit pas un appareil étranger à la partie, la projection de cet
//!   appareil (ce que tout le monde voit) doit rester la même, octet pour octet, et la
//!   vue de chaque regard dont la projection ne change pas aussi. Un secret d'un octet
//!   (un vote, un booléen, un chiffre), qu'aucune recherche d'octets ne distingue du
//!   hasard, se voit là s'il fuit dans l'état public ou dans une vue (RESTE-PHASE-1,
//!   point 16) ;
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

use jeu_abi::{
    Appel, ConstatFil, EntreeFil, IssueFil, Oui, RegardFil, Retour, SorteAppel, decoder, encoder,
};
use partie::{CausePanne, Echec, Fabrique, Faute, Instance, PanneOutil};

/// Le plus de constats détaillés gardés par contrôle : le reste se compte.
const DETAILS_MAX: usize = 8;

/// Ce que la sonde vérifie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Verifs {
    pub(crate) e4: Oui,
    pub(crate) e6: Oui,
    pub(crate) e7: Oui,
    /// E8, la variation : chaque geste ou tenu accepté, rejoué avec un autre octet.
    pub(crate) e8: Oui,
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
        e8: Oui::Non,
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
    /// E8, la variation des gestes.
    pub(crate) e8: Compte,
    /// E8 : le pas de la dernière variation ; une par pas au plus.
    pub(crate) derniere_variation: Option<u64>,
    /// E8 : les entrées rejouées avec un autre octet, et celles dont une variante a
    /// été acceptée. Zéro jugement n'est pas un succès (OUT-07) : le rapport les dit.
    pub(crate) variations: (u64, u64),
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
    /// E19 : le pas, le carburant et le tour de chaque entrée, par partie et par
    /// ordre. Une entrée rejouée (une reprise, une faute reconstruite) ne compte qu'une
    /// fois, au tour où elle a été jouée d'abord.
    pub(crate) carburant_entrees: BTreeMap<(u32, u64), CarburantEntree>,
    /// E19 : le tour de la boucle en cours, que la soirée avance à chaque
    /// `avancer_borne` (PHASE-3, P7).
    pub(crate) tour: u64,
}

/// E19 : ce que la sonde relève du coût d'une entrée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CarburantEntree {
    /// Le pas où elle est placée.
    pub(crate) place: u64,
    /// Son carburant.
    pub(crate) carburant: u64,
    /// Le tour de la boucle où elle a été jouée d'abord.
    pub(crate) tour: u64,
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
            e8: Compte::default(),
            derniere_variation: None,
            variations: (0, 0),
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
            tour: 0,
        }))
    }
}

/// Ce qu'E6 compare après une entrée acceptée (« même avenir ») : les empreintes de son
/// retour, de la forme canonique et des vues de tous les regards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Avenir {
    retour: [u8; 32],
    canonique: [u8; 32],
    vues: [u8; 32],
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
            candidat: RefCell::new(None),
            avant_refus: RefCell::new(None),
        })
    }

    fn neuve_tete(&mut self) -> Result<Sondee<F::I>, PanneOutil> {
        let mut i = self.neuve()?;
        i.tete = Oui::Oui;
        Ok(i)
    }
}

/// Une instance sondée.
pub(crate) struct Sondee<I: Instance> {
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
    /// E6, « même avenir » : l'instantané pris avant la dernière entrée, tant
    /// qu'aucun refus n'est en cours.
    candidat: RefCell<Option<I::Instantane>>,
    /// E6, « même avenir » : l'instantané d'avant le premier refus en cours, et le
    /// nombre de refus depuis.
    avant_refus: RefCell<Option<(I::Instantane, u32)>>,
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

/// E8 : un appareil qu'aucun regard de la soirée n'a : il ne voit que ce que tout le
/// monde voit (`Qui::Tous`).
fn etranger(regards: &[RegardFil]) -> RegardFil {
    let pris = |n: u64| regards.contains(&RegardFil::Appareil(n));
    let n = (0..u64::from(u16::MAX))
        .map(|k| u64::MAX.saturating_sub(k))
        .find(|n| !pris(*n))
        .unwrap_or(0);
    RegardFil::Appareil(n)
}

/// E8 : les variantes d'une entrée de geste ou de tenu, d'au moins un octet : le même
/// appel, son dernier octet changé (`^ 1`, `+ 1`, `- 1`), dans cet ordre. Aucune pour
/// toute autre entrée. Un geste d'un seul octet est un variant : sa variante est un
/// autre geste, que le jeu accepte ou non.
pub(crate) fn variantes(octets: &[u8]) -> Vec<Vec<u8>> {
    let Ok(Appel::Entree {
        ordre,
        place,
        entree,
    }) = decoder::<Appel<'_>>(octets)
    else {
        return Vec::new();
    };
    let Ok(EntreeFil::Geste { octets: charge, .. } | EntreeFil::Tenu { octets: charge, .. }) =
        decoder::<EntreeFil<'_>>(entree)
    else {
        return Vec::new();
    };
    // Un seul octet se varie aussi : un vote en enum plate (`VoterGauche`) est un
    // geste d'un octet (revue de la phase 2, constat 2).
    let Some(&dernier) = charge.last() else {
        return Vec::new();
    };
    let mut autres: Vec<u8> = Vec::new();
    for o in [
        dernier ^ 1,
        dernier.wrapping_add(1),
        dernier.wrapping_sub(1),
    ] {
        if o != dernier && !autres.contains(&o) {
            autres.push(o);
        }
    }
    let mut rendu = Vec::new();
    for o in autres {
        let mut c = charge.to_vec();
        if let Some(d) = c.last_mut() {
            *d = o;
        }
        let e = match decoder::<EntreeFil<'_>>(entree) {
            Ok(EntreeFil::Geste { de, quand, .. }) => EntreeFil::Geste {
                de,
                octets: &c,
                quand,
            },
            Ok(EntreeFil::Tenu {
                de, cause, quand, ..
            }) => EntreeFil::Tenu {
                de,
                octets: &c,
                cause,
                quand,
            },
            Ok(_) | Err(_) => return Vec::new(),
        };
        let mut e_octets = Vec::new();
        let mut a_octets = Vec::new();
        if encoder(&e, &mut e_octets).is_ok()
            && encoder(
                &Appel::Entree {
                    ordre,
                    place,
                    entree: &e_octets,
                },
                &mut a_octets,
            )
            .is_ok()
        {
            rendu.push(a_octets);
        }
    }
    rendu
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
    #[expect(
        clippy::too_many_lines,
        reason = "une entrée et ses constats se lisent d'un trait : avant, l'entrée, après"
    )]
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
        if verifs.e6 == Oui::Oui {
            self.instantane_candidat();
        }
        let avant_geste = if verifs.e8 == Oui::Oui && self.a_varier(octets, place) {
            match self.inner.borrow_mut().instantane() {
                Ok(i) => Some(i),
                Err(p) => {
                    self.obs.borrow_mut().pannes.push(p);
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
            self.oublier_les_refus();
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
                if verifs.e6 == Oui::Oui {
                    self.meme_avenir(sorte, octets, budget, retour, (ordre, place));
                }
                if let Some(avant) = &avant_geste {
                    self.varier(octets, budget, avant, place);
                }
                self.candidat.borrow_mut().take();
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
                if verifs.e6 == Oui::Oui {
                    self.retenir_le_refus();
                }
                if let Some(avant) = trace_avant {
                    self.juger_le_refus(avant, (ordre, place));
                }
            }
            None => self.oublier_les_refus(),
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

    /// E6 : après un refus ou un `ignorer`, la forme canonique et les vues de tous
    /// les regards sont celles d'avant, octet pour octet.
    fn juger_le_refus(&self, avant: ([u8; 32], [u8; 32]), (ordre, place): (u64, u64)) {
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

    /// E6 : un refus de plus. Le premier d'une suite garde l'instantané pris avant lui.
    fn retenir_le_refus(&self) {
        let mut en_cours = self.avant_refus.borrow_mut();
        match en_cours.as_mut() {
            Some((_, n)) => *n = n.saturating_add(1),
            None => *en_cours = self.candidat.borrow_mut().take().map(|i| (i, 1)),
        }
    }

    /// E6 : prend l'instantané d'avant une entrée, tant qu'aucun refus n'est en cours.
    fn instantane_candidat(&self) {
        if self.avant_refus.borrow().is_some() {
            return;
        }
        match self.inner.borrow_mut().instantane() {
            Ok(i) => *self.candidat.borrow_mut() = Some(i),
            Err(p) => self.obs.borrow_mut().pannes.push(p),
        }
    }

    /// E6 : oublie les refus en cours, et l'instantané d'avant le premier : ce qui suit
    /// n'est plus une entrée acceptée après eux (une faute, une création, une
    /// restauration).
    fn oublier_les_refus(&self) {
        self.candidat.borrow_mut().take();
        self.avant_refus.borrow_mut().take();
    }

    /// Ce qu'E6 compare après une entrée acceptée : le retour, la forme canonique et
    /// les vues de tous les regards, lus sans constat.
    fn avenir(&self, retour: &[u8]) -> Result<Avenir, Echec> {
        let budget = self.obs.borrow().budget;
        let regards = self.obs.borrow().regards.clone();
        let c = recoller(|morceau| {
            self.brut(
                SorteAppel::Lecture,
                &encode(&Appel::Canonique { morceau })?,
                budget,
            )
        })?;
        let v = self.brut(
            SorteAppel::Lecture,
            &encode(&Appel::Regarder { regards })?,
            budget,
        )?;
        Ok(Avenir {
            retour: hacher(retour),
            canonique: hacher(&c),
            vues: hacher(&v),
        })
    }

    /// E6, « même avenir » (RESTE-PHASE-1, point 11) : l'entrée acceptée qui suit des
    /// refus, rejouée sur l'instantané d'avant le premier d'entre eux, rend le même
    /// retour, la même forme canonique et les mêmes vues. Un refus ne change rien,
    /// même hors de l'état : un `static` écrit dans `Avant` se voit ici dès qu'il change
    /// ce que fait l'entrée suivante. L'instance est remise, à la fin, telle que
    /// l'entrée réelle l'a laissée.
    fn meme_avenir(
        &self,
        sorte: SorteAppel,
        octets: &[u8],
        budget: u64,
        retour: &[u8],
        (ordre, place): (u64, u64),
    ) {
        let Some((avant, refus)) = self.avant_refus.borrow_mut().take() else {
            return;
        };
        let constat = (|| -> Result<Option<&'static str>, Echec> {
            // L'instantané d'abord : les lectures qui suivent ne laissent rien à
            // l'instance, même si le jeu y écrit (E7 le juge à part).
            let apres = self.inner.borrow_mut().instantane().map_err(Echec::Panne)?;
            let reel = self.avenir(retour)?;
            self.inner
                .borrow_mut()
                .restaurer(&avant)
                .map_err(Echec::Panne)?;
            let ombre = self.brut(sorte, octets, budget);
            let ombre = match ombre {
                Ok(o) => self.avenir(&o).map(Some),
                Err(Echec::Faute(_)) => Ok(None),
                Err(e) => Err(e),
            };
            self.inner
                .borrow_mut()
                .restaurer(&apres)
                .map_err(Echec::Panne)?;
            Ok(match ombre? {
                None => Some("le retour"),
                Some(o) if o.retour != reel.retour => Some("le retour"),
                Some(o) if o.canonique != reel.canonique => Some("la forme canonique"),
                Some(o) if o.vues != reel.vues => Some("une vue"),
                Some(_) => None,
            })
        })();
        match constat {
            Ok(difference) => {
                let mut obs = self.obs.borrow_mut();
                obs.e6.evaluer();
                if let Some(quoi) = difference {
                    obs.e6.echouer(format!(
                        "un refus a changé l'avenir : l'entrée acceptée au pas {place} (ordre {ordre}), rejouée sans le ou les {refus} refus qui la précèdent, change {quoi}"
                    ));
                }
            }
            Err(e) => self.noter_faute("l'entrée rejouée sans ses refus (E6)", e),
        }
    }

    /// E8 : cette entrée se varie-t-elle ? Un geste ou un tenu d'un téléphone, d'au
    /// moins un octet, et pas plus d'une variation par pas.
    fn a_varier(&self, octets: &[u8], place: u64) -> bool {
        if self.obs.borrow().derniere_variation == Some(place) {
            return false;
        }
        !variantes(octets).is_empty()
    }

    /// Ce que voit chaque regard de la sonde, et l'étranger : la forme canonique, puis
    /// la projection de chacun, l'étranger en premier.
    fn projections(&self, regards: &[RegardFil]) -> Result<(Vec<u8>, Vec<Vec<u8>>), Echec> {
        let budget = self.obs.borrow().budget;
        let canonique = recoller(|morceau| {
            self.brut(
                SorteAppel::Lecture,
                &encode(&Appel::Canonique { morceau })?,
                budget,
            )
        })?;
        let mut p = Vec::with_capacity(regards.len());
        for regard in regards {
            p.push(recoller(|morceau| {
                self.brut(
                    SorteAppel::Lecture,
                    &encode(&Appel::Projeter {
                        regard: *regard,
                        morceau,
                    })?,
                    budget,
                )
            })?);
        }
        Ok((canonique, p))
    }

    /// La vue d'un regard, lue sans constat.
    fn vue_brute(&self, regard: RegardFil) -> Result<Vec<u8>, Echec> {
        let budget = self.obs.borrow().budget;
        self.brut(
            SorteAppel::Lecture,
            &encode(&Appel::Regarder {
                regards: vec![regard],
            })?,
            budget,
        )
    }

    /// E8, la variation (RESTE-PHASE-1, point 16) : l'entrée acceptée se rejoue sur
    /// l'instantané d'avant elle, avec un autre dernier octet, jusqu'à ce qu'une
    /// variante soit acceptée. Si la variante change un secret que l'étranger ne voit
    /// pas (et pas le hasard), la projection de l'étranger doit être la même ; et chaque
    /// regard dont la projection ne change pas doit voir la même vue. L'instance est
    /// remise, à la fin, telle que l'entrée réelle l'a laissée.
    fn varier(&self, octets: &[u8], budget: u64, avant: &I::Instantane, place: u64) {
        {
            let mut obs = self.obs.borrow_mut();
            obs.derniere_variation = Some(place);
            obs.variations.0 = obs.variations.0.saturating_add(1);
        }
        let mut regards = vec![etranger(&self.obs.borrow().regards)];
        regards.extend(self.obs.borrow().regards.iter().copied());
        let constat = (|| -> Result<Option<Option<String>>, Echec> {
            let apres = self.inner.borrow_mut().instantane().map_err(Echec::Panne)?;
            let (canon_a, proj_a) = self.projections(&regards)?;
            let mut jugement = None;
            for v in variantes(octets) {
                self.inner
                    .borrow_mut()
                    .restaurer(avant)
                    .map_err(Echec::Panne)?;
                let acceptee = matches!(
                    self.brut(SorteAppel::Ecriture, &v, budget)
                        .as_deref()
                        .map(decoder::<Retour<'_>>),
                    Ok(Ok(Retour::Issue(IssueFil::Acceptee { .. })))
                );
                if !acceptee {
                    continue;
                }
                {
                    let mut obs = self.obs.borrow_mut();
                    obs.variations.1 = obs.variations.1.saturating_add(1);
                }
                let (canon_b, proj_b) = self.projections(&regards)?;
                jugement = Some(self.juger_la_variation(
                    &regards,
                    (&canon_a, &proj_a),
                    (&canon_b, &proj_b),
                    &apres,
                )?);
                break;
            }
            self.inner
                .borrow_mut()
                .restaurer(&apres)
                .map_err(Echec::Panne)?;
            Ok(jugement)
        })();
        match constat {
            Ok(None | Some(None)) | Err(Echec::Faute(_)) => {}
            Ok(Some(Some(fuite))) => {
                self.obs
                    .borrow_mut()
                    .e8
                    .echouer(format!("au pas {place} : {fuite}"));
            }
            Err(e) => self.noter_faute("la variation d'un geste (E8)", e),
        }
    }

    /// Le jugement d'une variation acceptée : `None` si elle ne touche aucun secret
    /// caché (rien à juger), sinon `Some(None)` si rien ne fuit, `Some(Some(_))` si
    /// quelque chose fuit. Les vues de la variante sont lues avant que l'instance ne
    /// revienne à `apres`, puis celles de l'entrée réelle.
    fn juger_la_variation(
        &self,
        regards: &[RegardFil],
        (canon_a, proj_a): (&[u8], &[Vec<u8>]),
        (canon_b, proj_b): (&[u8], &[Vec<u8>]),
        apres: &I::Instantane,
    ) -> Result<Option<String>, Echec> {
        if canon_a == canon_b {
            return Ok(None);
        }
        let (Some(public_a), Some(public_b)) = (proj_a.first(), proj_b.first()) else {
            return Ok(None);
        };
        let Some(caches) = crate::controles::places_cachees(canon_a, public_a) else {
            return Ok(None);
        };
        // Le hasard tiré autrement : les deux parties ne diffèrent plus par la seule
        // valeur du geste, rien à juger.
        let hasard_a = crate::controles::place_du_hasard(canon_a);
        let hasard_b = crate::controles::place_du_hasard(canon_b);
        let valeur = |c: &[u8], h: Option<(usize, usize)>| {
            h.and_then(|(d, f)| c.get(d..f).map(<[u8]>::to_vec))
        };
        if valeur(canon_a, hasard_a) != valeur(canon_b, hasard_b) {
            return Ok(None);
        }
        // Les valeurs secrètes comparées une à une : une longueur qui change déplace
        // les octets sans changer un secret (revue de la phase 2, constat 8). Si l'une
        // des formes ne s'aligne pas sur sa projection, les octets qui diffèrent
        // doivent recouvrir un secret.
        match crate::controles::secret_change((canon_a, public_a), (canon_b, public_b)) {
            Some(false) => return Ok(None),
            Some(true) => {}
            None => {
                let ecarts = crate::controles::differences(canon_a, canon_b);
                let caches_sans_hasard: Vec<(usize, usize)> = caches
                    .into_iter()
                    .filter(|c| hasard_a != Some(*c))
                    .collect();
                if !crate::controles::se_recouvrent(&ecarts, &caches_sans_hasard) {
                    // La variante ne change que ce que tout le monde voit : rien de
                    // secret.
                    return Ok(None);
                }
            }
        }
        self.obs.borrow_mut().e8.evaluer();
        if public_a != public_b {
            return Ok(Some(String::from(
                "un octet de geste qui change un secret change aussi ce que voit un appareil étranger à la partie (la projection publique) : le secret fuit dans l'état public",
            )));
        }
        // Les regards dont la projection ne change pas doivent voir la même vue.
        let memes: Vec<RegardFil> = regards
            .iter()
            .zip(proj_a.iter().zip(proj_b))
            .skip(1)
            .filter(|(_, (a, b))| a == b)
            .map(|(r, _)| *r)
            .collect();
        let mut vues_b = Vec::with_capacity(memes.len());
        for r in &memes {
            vues_b.push(self.vue_brute(*r)?);
        }
        self.inner
            .borrow_mut()
            .restaurer(apres)
            .map_err(Echec::Panne)?;
        for (r, vb) in memes.iter().zip(&vues_b) {
            if self.vue_brute(*r)? != *vb {
                return Ok(Some(format!(
                    "un octet de geste qui change un secret que le regard {r:?} ne voit pas change sa vue, et pas sa projection"
                )));
            }
        }
        Ok(None)
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

    /// E19 : le carburant d'une entrée, avec son pas et son tour.
    fn noter_carburant_de_l_entree(&self, ordre: u64, place: u64) {
        let carburant = self.inner.borrow().carburant();
        let partie = self.partie();
        let mut obs = self.obs.borrow_mut();
        if obs.verifs.budgets == Oui::Oui
            && let Some(c) = carburant
        {
            let tour = obs.tour;
            obs.carburant_entrees
                .entry((partie, ordre))
                .and_modify(|e| {
                    e.place = place;
                    e.carburant = c;
                })
                .or_insert(CarburantEntree {
                    place,
                    carburant: c,
                    tour,
                });
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
        self.oublier_les_refus();
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
                self.oublier_les_refus();
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
        self.oublier_les_refus();
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

#[cfg(test)]
mod tests {
    use jeu_abi::{Appel, EntreeFil, QuandFil, encoder};

    use super::variantes;

    fn geste(charge: &[u8]) -> Vec<u8> {
        let quand = QuandFil {
            fait: 1,
            fraction: 0,
            recu: 1,
            vu: 1,
            id: 1,
        };
        let mut e = Vec::new();
        encoder(
            &EntreeFil::Geste {
                de: 7,
                octets: charge,
                quand,
            },
            &mut e,
        )
        .expect("encodée");
        let mut a = Vec::new();
        encoder(
            &Appel::Entree {
                ordre: 1,
                place: 1,
                entree: &e,
            },
            &mut a,
        )
        .expect("encodé");
        a
    }

    /// Revue de la phase 2, constat 2 : un geste d'un seul octet (un vote en enum
    /// plate, `VoterGauche`/`VoterDroite`, ou le `Proposer` de `devine`) se varie aussi ;
    /// avant, `devine` ne voyait aucune variation de toute la soirée (OUT-07).
    #[test]
    fn un_geste_d_un_octet_se_varie() {
        assert_eq!(variantes(&geste(&[0])).len(), 2);
        assert_eq!(variantes(&geste(&[1, 4])).len(), 2);
        assert_eq!(variantes(&geste(&[])).len(), 0);
    }
}
