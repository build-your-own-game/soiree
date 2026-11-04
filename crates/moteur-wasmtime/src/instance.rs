//! `InstanceWasm` et `FabriqueWasm` : le module réécrit comme `partie::Instance`.
//!
//! Chaque instance a son fil, avec une pile native assez grande pour que
//! `soiree_profondeur` pique toujours avant elle (§ 8.7) : la pile de l'appelant
//! n'entre pas en jeu, que le cœur tourne dans un test, sur le fil principal ou
//! ailleurs. Le fil porte le magasin de wasmtime ; l'instance lui envoie chaque
//! opération et attend sa réponse.

use std::sync::Arc;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use jeu_abi::SorteAppel;
use partie::{CausePanne, Echec, Fabrique, Instance, PanneOutil};

use crate::interieur::{Base, InstantaneWasm, Interieur, Zones};

type Tache = Box<dyn FnOnce(&mut Option<Interieur>) + Send>;

fn interne(quoi: impl Into<String>) -> PanneOutil {
    PanneOutil::new(CausePanne::Interne, quoi)
}

/// Le fil d'une instance.
struct Fil {
    envoi: Option<mpsc::Sender<Tache>>,
    poignee: Option<JoinHandle<()>>,
}

impl Fil {
    fn lancer(pile: usize) -> Result<Fil, PanneOutil> {
        let (envoi, recu) = mpsc::channel::<Tache>();
        let poignee = thread::Builder::new()
            .name(String::from("soiree-instance"))
            .stack_size(pile)
            .spawn(move || {
                let mut interieur = None;
                while let Ok(tache) = recu.recv() {
                    tache(&mut interieur);
                }
            })
            .map_err(|e| PanneOutil::new(CausePanne::Instanciation, format!("fil : {e}")))?;
        Ok(Fil {
            envoi: Some(envoi),
            poignee: Some(poignee),
        })
    }

    /// Exécute `f` sur le fil, et attend son résultat.
    fn faire<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Option<Interieur>) -> R + Send + 'static,
    ) -> Result<R, PanneOutil> {
        let (rendre, attendre) = mpsc::sync_channel(1);
        let tache: Tache = Box::new(move |i| {
            let _: Result<(), mpsc::SendError<R>> = rendre.send(f(i));
        });
        self.envoi
            .as_ref()
            .ok_or_else(|| interne("fil arrêté"))?
            .send(tache)
            .map_err(|_| interne("le fil de l'instance s'est arrêté"))?;
        attendre
            .recv()
            .map_err(|_| interne("le fil de l'instance s'est arrêté pendant l'opération"))
    }
}

impl Drop for Fil {
    fn drop(&mut self) {
        drop(self.envoi.take());
        if let Some(p) = self.poignee.take() {
            let _: thread::Result<()> = p.join();
        }
    }
}

/// Ce qu'une instance d'un module chargé connaît de lui.
pub(crate) struct Commun {
    pub(crate) base: Arc<Base>,
    pub(crate) zones: Zones,
    /// La valeur de `soiree_memoire_max`, en pages : `limites.memoire` (§ 8.7).
    pub(crate) memoire_max: u32,
    /// Le garde-fou doit tourner tant qu'une instance vit.
    pub(crate) _horloge: Arc<crate::horloge::Horloge>,
}

/// Le module réécrit dans wasmtime, comme `partie::Instance`.
pub struct InstanceWasm {
    commun: Arc<Commun>,
    fil: Fil,
    machine: Option<usize>,
    carburant: Option<u64>,
    creees: u64,
}

fn absente() -> PanneOutil {
    interne("l'instance n'existe plus après une panne")
}

impl InstanceWasm {
    fn new(commun: Arc<Commun>, machine: Option<usize>) -> Result<InstanceWasm, PanneOutil> {
        let fil = Fil::lancer(commun.base.pile_fil)?;
        let mut i = InstanceWasm {
            commun,
            fil,
            machine,
            carburant: None,
            creees: 0,
        };
        i.recreer()?;
        Ok(i)
    }

    /// Remplace l'instance du fil par une neuve.
    fn recreer(&mut self) -> Result<(), PanneOutil> {
        let base = Arc::clone(&self.commun.base);
        let max = self.commun.memoire_max;
        let machine = self.machine;
        self.fil.faire(move |i| {
            *i = None;
            *i = Some(Interieur::new(&base, max, machine)?);
            Ok::<(), PanneOutil>(())
        })??;
        self.creees = self.creees.saturating_add(1);
        Ok(())
    }

    /// Le nombre d'instances wasmtime créées pour celle-ci : 1, plus une par
    /// restauration dans une mémoire plus grande.
    pub fn instances_creees(&self) -> u64 {
        self.creees
    }

    /// La taille de la mémoire, en pages.
    pub fn pages(&mut self) -> Result<u64, PanneOutil> {
        self.fil
            .faire(|i| i.as_ref().map(Interieur::pages).ok_or_else(absente))?
    }

    /// La valeur de `soiree_memoire_max`, en pages.
    pub fn memoire_max(&mut self) -> Result<i64, PanneOutil> {
        self.fil
            .faire(|i| i.as_mut().map(Interieur::memoire_max).ok_or_else(absente))?
    }
}

impl Instance for InstanceWasm {
    type Instantane = InstantaneWasm;

    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8], budget: u64) -> Result<Vec<u8>, Echec> {
        let commun = Arc::clone(&self.commun);
        let appel = appel.to_vec();
        let rendu = self.fil.faire(move |i| {
            i.as_mut()
                .map(|i| i.appeler(&commun.base, Some(commun.zones), sorte, &appel, budget))
        })?;
        let rendu = rendu.ok_or_else(absente)?;
        self.carburant = Some(rendu.carburant);
        rendu.issue
    }

    fn carburant(&self) -> Option<u64> {
        self.carburant
    }

    fn instantane(&mut self) -> Result<InstantaneWasm, PanneOutil> {
        let base = Arc::clone(&self.commun.base);
        self.fil
            .faire(move |i| i.as_mut().ok_or_else(absente)?.instantane(&base))?
    }

    fn restaurer(&mut self, i: &InstantaneWasm) -> Result<(), PanneOutil> {
        if i.module != self.commun.base.empreinte {
            return Err(interne("instantané d'un autre module"));
        }
        if self.pages()? > i.pages {
            // Une mémoire ne rétrécit pas : instance neuve (§ 8.6).
            self.recreer()?;
        }
        let i = i.clone();
        self.fil
            .faire(move |interieur| interieur.as_mut().ok_or_else(absente)?.restaurer(&i))?
    }

    fn empreinte(&mut self) -> Result<[u8; 32], PanneOutil> {
        let commun = Arc::clone(&self.commun);
        self.fil.faire(move |i| {
            i.as_mut()
                .ok_or_else(absente)?
                .empreinte(&commun.base, commun.zones)
        })?
    }
}

/// Ce qui fabrique des instances neuves d'un module chargé.
#[derive(Clone)]
pub struct FabriqueWasm {
    pub(crate) commun: Arc<Commun>,
    pub(crate) machine: Option<usize>,
}

impl FabriqueWasm {
    /// Pour l'épreuve et les tests : une machine qui n'accorde que `octets` de
    /// mémoire à chaque instance. Un `memory.grow` permis par le module qu'elle
    /// refuse est une panne de la machine (§ 4.6).
    #[doc(hidden)]
    pub fn simuler_une_machine_limitee(&mut self, octets: usize) {
        self.machine = Some(octets);
    }
}

impl Fabrique for FabriqueWasm {
    type I = InstanceWasm;

    fn neuve(&mut self) -> Result<InstanceWasm, PanneOutil> {
        InstanceWasm::new(Arc::clone(&self.commun), self.machine)
    }
}

/// Une instance sonde, pour `charger` : sans zones connues au premier appel.
pub(crate) struct Sonde {
    fil: Fil,
    base: Arc<Base>,
}

impl Sonde {
    pub(crate) fn new(base: Arc<Base>, memoire_max: u32) -> Result<Sonde, PanneOutil> {
        let fil = Fil::lancer(base.pile_fil)?;
        let b = Arc::clone(&base);
        fil.faire(move |i| {
            *i = Some(Interieur::new(&b, memoire_max, None)?);
            Ok::<(), PanneOutil>(())
        })??;
        Ok(Sonde { fil, base })
    }

    pub(crate) fn appeler(
        &self,
        zones: Option<Zones>,
        sorte: SorteAppel,
        appel: Vec<u8>,
        budget: u64,
    ) -> Result<Vec<u8>, Echec> {
        let base = Arc::clone(&self.base);
        let rendu = self.fil.faire(move |i| {
            i.as_mut()
                .map(|i| i.appeler(&base, zones, sorte, &appel, budget))
        })?;
        rendu.ok_or_else(absente)?.issue
    }

    pub(crate) fn pages(&self) -> Result<u64, PanneOutil> {
        self.fil
            .faire(|i| i.as_ref().map(Interieur::pages).ok_or_else(absente))?
    }
}
