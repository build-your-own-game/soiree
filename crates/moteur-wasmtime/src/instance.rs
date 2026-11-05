//! `InstanceWasm` et `FabriqueWasm` : le module réécrit comme `partie::Instance`.
//!
//! Chaque instance a son fil, avec une pile native assez grande pour que
//! `soiree_profondeur` pique toujours avant elle (§ 8.7) : la pile de l'appelant
//! n'entre pas en jeu, que le cœur tourne dans un test, sur le fil principal ou
//! ailleurs. Le fil porte le protocole du commun ([`moteur_commun::Protocole`]) et la
//! machine wasmtime ; l'instance lui envoie chaque opération et attend sa réponse.

use std::sync::Arc;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use jeu_abi::SorteAppel;
use moteur_commun::{Carte, Charge, Instantane, Plan, Protocole, RefusModule, verifier_chargement};
use partie::{CausePanne, Echec, Fabrique, Instance, PanneOutil};

use crate::machine::{Base, MachineWasm};

fn interne(quoi: impl Into<String>) -> PanneOutil {
    PanneOutil::new(CausePanne::Interne, quoi)
}

/// Le fil d'une instance, qui porte un `T`.
struct Fil<T> {
    envoi: Option<mpsc::Sender<Tache<T>>>,
    poignee: Option<JoinHandle<()>>,
}

type Tache<T> = Box<dyn FnOnce(&mut Option<T>) + Send>;

impl<T: 'static> Fil<T> {
    fn lancer(pile: usize) -> Result<Fil<T>, PanneOutil> {
        let (envoi, recu) = mpsc::channel::<Tache<T>>();
        let poignee = thread::Builder::new()
            .name(String::from("soiree-instance"))
            .stack_size(pile)
            .spawn(move || {
                let mut porte = None;
                while let Ok(tache) = recu.recv() {
                    tache(&mut porte);
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
        f: impl FnOnce(&mut Option<T>) -> R + Send + 'static,
    ) -> Result<R, PanneOutil> {
        let (rendre, attendre) = mpsc::sync_channel(1);
        let tache: Tache<T> = Box::new(move |i| {
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

impl<T> Drop for Fil<T> {
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
    pub(crate) carte: Arc<Carte>,
    /// Le garde-fou doit tourner tant qu'une instance vit.
    pub(crate) _horloge: Arc<crate::horloge::Horloge>,
}

type ProtocoleWasm = Protocole<MachineWasm>;

/// Le module réécrit dans wasmtime, comme `partie::Instance`.
pub struct InstanceWasm {
    fil: Fil<ProtocoleWasm>,
    carburant: Option<u64>,
    creees: u64,
}

fn absente() -> PanneOutil {
    interne("l'instance n'existe plus après une panne")
}

impl InstanceWasm {
    fn new(commun: &Commun, machine: Option<usize>) -> Result<InstanceWasm, PanneOutil> {
        let fil = Fil::lancer(commun.base.pile_fil)?;
        let base = Arc::clone(&commun.base);
        let carte = Arc::clone(&commun.carte);
        fil.faire(move |p| {
            *p = Some(Protocole::new(MachineWasm::new(&base, machine)?, carte)?);
            Ok::<(), PanneOutil>(())
        })??;
        Ok(InstanceWasm {
            fil,
            carburant: None,
            creees: 1,
        })
    }

    /// Le nombre d'instances wasmtime créées pour celle-ci : 1, plus une par
    /// restauration dans une mémoire plus grande.
    pub fn instances_creees(&self) -> u64 {
        self.creees
    }

    /// La taille de la mémoire, en pages.
    pub fn pages(&mut self) -> Result<u64, PanneOutil> {
        self.fil
            .faire(|p| p.as_mut().map(ProtocoleWasm::pages).ok_or_else(absente))?
    }

    /// La valeur de `soiree_memoire_max`, en pages.
    pub fn memoire_max(&mut self) -> Result<i64, PanneOutil> {
        self.fil
            .faire(|p| p.as_mut().ok_or_else(absente)?.memoire_max())?
    }
}

impl Instance for InstanceWasm {
    type Instantane = Instantane;

    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8], budget: u64) -> Result<Vec<u8>, Echec> {
        let appel = appel.to_vec();
        let (issue, carburant) = self.fil.faire(move |p| {
            let p = p.as_mut().ok_or_else(absente)?;
            let issue = p.appeler(sorte, &appel, budget);
            Ok::<_, PanneOutil>((issue, p.carburant()))
        })??;
        self.carburant = carburant;
        issue
    }

    fn carburant(&self) -> Option<u64> {
        self.carburant
    }

    fn instantane(&mut self) -> Result<Instantane, PanneOutil> {
        self.fil
            .faire(|p| p.as_mut().ok_or_else(absente)?.instantane())?
    }

    fn restaurer(&mut self, i: &Instantane) -> Result<(), PanneOutil> {
        let i = i.clone();
        let creees = self.fil.faire(move |p| {
            let p = p.as_mut().ok_or_else(absente)?;
            p.restaurer(&i)?;
            Ok::<u64, PanneOutil>(p.instances_creees())
        })??;
        self.creees = creees;
        Ok(())
    }

    fn empreinte(&mut self) -> Result<[u8; 32], PanneOutil> {
        self.fil
            .faire(|p| p.as_mut().ok_or_else(absente)?.empreinte())?
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
        InstanceWasm::new(&self.commun, self.machine)
    }
}

/// Le chargement, sur le fil d'une instance sonde : une machine neuve, qui ne sert qu'à
/// [`verifier_chargement`].
pub(crate) fn sonder(base: &Arc<Base>, plan: Plan) -> Result<Charge, RefusModule> {
    let fil = Fil::<MachineWasm>::lancer(base.pile_fil)?;
    let b = Arc::clone(base);
    fil.faire(move |m| {
        let mut machine = MachineWasm::new(&b, None)?;
        let charge = verifier_chargement(&mut machine, plan);
        *m = Some(machine);
        charge
    })?
}
