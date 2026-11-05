//! Charger un module dans le navigateur, comme `moteur_wasmtime::Moteur::charger` :
//! réécrire, compiler, instancier une sonde, la faire vérifier par
//! [`moteur_commun::verifier_chargement`], puis fabriquer des instances.

use std::rc::Rc;
use std::sync::Arc;

use jeu_abi::{Bonjour, DeclarationFil};
use moteur_commun::{Carte, Plan, Protocole, RefusModule, globales_d_origine, verifier_chargement};
use partie::{Fabrique, PanneOutil};
use reecriture::Reecrit;

use crate::machine::{MachineNavigateur, ModuleNavigateur};

/// Le module réécrit dans le navigateur, comme `partie::Instance` : le protocole du
/// commun sur une [`MachineNavigateur`].
pub type InstanceNavigateur = Protocole<MachineNavigateur>;

/// Réécrit `original` sous `reglages`, le compile, l'instancie une fois, appelle
/// `Bonjour` et `Decrire`, vérifie l'ABI, les zones et la déclaration (sous
/// [`moteur_commun::BUDGET_DE_CHARGEMENT`]).
///
/// Le garde-fou mural n'est pas ici : le navigateur n'interrompt pas un appel sur son
/// fil, et c'est la page qui tue le Worker (PHASE-3, § 2.3).
pub fn charger(
    original: &[u8],
    reglages: &reecriture::Reglages,
) -> Result<ModuleCharge, RefusModule> {
    let reecrit = reecriture::reecrire(original, reglages)?;
    let module = ModuleNavigateur::compiler(&reecrit.octets)?;
    let exports = module.exports();
    let globales = globales_d_origine(exports.iter().map(String::as_str))?;
    let plan = Plan::new(&reecrit, reglages, &globales);
    let mut sonde = MachineNavigateur::new(&module)?;
    let charge = verifier_chargement(&mut sonde, plan)?;
    Ok(ModuleCharge {
        reecrit: Rc::new(reecrit),
        bonjour: charge.bonjour,
        declaration: Rc::new(charge.declaration),
        carburant_du_chargement: charge.carburant,
        module,
        carte: Arc::new(charge.carte),
    })
}

/// Un module chargé : réécrit, compilé, son `Bonjour` et sa déclaration vérifiés.
#[derive(Clone)]
pub struct ModuleCharge {
    reecrit: Rc<Reecrit>,
    bonjour: Bonjour,
    declaration: Rc<DeclarationFil>,
    carburant_du_chargement: (u64, u64),
    module: Rc<ModuleNavigateur>,
    carte: Arc<Carte>,
}

impl ModuleCharge {
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

    /// Ce que ses instances partagent : ses zones, `soiree_memoire_max`, la mémoire
    /// qu'un instantané garde.
    pub fn carte(&self) -> &Carte {
        &self.carte
    }

    /// Le carburant du chargement : celui du premier `Bonjour`, et celui de `Decrire`.
    pub fn carburant_du_chargement(&self) -> (u64, u64) {
        self.carburant_du_chargement
    }

    /// Ce qui fabrique ses instances.
    pub fn fabrique(&self) -> FabriqueNavigateur {
        FabriqueNavigateur {
            module: Rc::clone(&self.module),
            carte: Arc::clone(&self.carte),
        }
    }
}

/// Ce qui fabrique des instances neuves d'un module chargé.
#[derive(Clone)]
pub struct FabriqueNavigateur {
    module: Rc<ModuleNavigateur>,
    carte: Arc<Carte>,
}

impl Fabrique for FabriqueNavigateur {
    type I = InstanceNavigateur;

    fn neuve(&mut self) -> Result<InstanceNavigateur, PanneOutil> {
        Protocole::new(
            MachineNavigateur::new(&self.module)?,
            Arc::clone(&self.carte),
        )
    }
}
