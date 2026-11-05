//! La copie native d'un jeu, comme `partie::Instance` (INTERFACE.md, § 8.4).
//!
//! Elle compile la même crate de règles en natif, et appelle `jeu::colle::appeler` par
//! une fonction Rust au lieu d'un export. Elle n'a pas les garanties de l'enfermement :
//! l'épreuve compare ses retours et sa forme canonique à ceux du module (E9).
//!
//! - **Une panique** est rattrapée par `catch_unwind` et devient [`Faute::Panique`], avec
//!   son message. La copie native doit donc être compilée avec `panic = "unwind"`, ce que
//!   font les profils `dev`, `test` et `release` du workspace.
//! - **Pas de carburant** : la copie native n'a pas de budget ; [`Instance::carburant`]
//!   rend `None`, et une boucle infinie ne rend pas la main. Seul le module la borne.
//! - **Un instantané** clone l'enveloppe ; restaurer la remplace.
//! - **L'empreinte** est le BLAKE3 de la forme canonique de l'enveloppe, secrets compris.
//!   Elle ne se compare qu'entre copies natives : celle d'un module mesure sa mémoire.
//! - **Le jeton de l'hôte** (`jeu::colle::Hote`) : chaque instance en construit un, la
//!   seule ligne `unsafe` de la crate. C'est un sceau, pas un accès à la mémoire.
//! - **Absente du module** : la crate est vide sous `wasm32`. Un jeu ne peut pas s'y
//!   faire tourner lui-même pour se regarder sous un autre regard.

#![cfg(not(target_arch = "wasm32"))]
#![deny(unsafe_code)]

use std::any::Any;
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};

use jeu::colle::{Cadre, Enveloppe, Hote, SansModes, Sortie, Zones, appeler, canonique};
use jeu::{Epreuve, Jeu};
use jeu_abi::SorteAppel;
use partie::{CausePanne, Echec, Fabrique, Faute, Instance, PanneOutil};

/// La copie native d'un jeu.
pub struct InstanceNative<J: Jeu + Epreuve> {
    enveloppe: Option<Enveloppe<J>>,
    hote: Hote,
}

impl<J: Jeu + Epreuve> InstanceNative<J> {
    /// Une instance neuve, avant `Bonjour`.
    pub const fn new() -> InstanceNative<J> {
        #[expect(
            unsafe_code,
            reason = "la copie native est l'hôte de la colle : elle seule tient son jeton"
        )]
        // SAFETY : le jeton reste dans l'instance, que le jeu ne voit pas.
        let hote = unsafe { Hote::new() };
        InstanceNative {
            enveloppe: None,
            hote,
        }
    }

    /// L'enveloppe, pour les tests.
    pub fn enveloppe(&self) -> Option<&Enveloppe<J>> {
        self.enveloppe.as_ref()
    }
}

impl<J: Jeu + Epreuve> Default for InstanceNative<J> {
    fn default() -> Self {
        InstanceNative::new()
    }
}

/// L'instantané d'une copie native : son enveloppe, clonée.
pub struct InstantaneNatif<J: Jeu + Epreuve>(Option<Enveloppe<J>>);

impl<J: Jeu + Epreuve> Clone for InstantaneNatif<J> {
    fn clone(&self) -> Self {
        InstantaneNatif(self.0.clone())
    }
}

/// Ce qui fabrique des copies natives d'un jeu.
pub struct FabriqueNative<J>(PhantomData<J>);

impl<J> FabriqueNative<J> {
    /// La fabrique.
    pub const fn new() -> FabriqueNative<J> {
        FabriqueNative(PhantomData)
    }
}

impl<J> Default for FabriqueNative<J> {
    fn default() -> Self {
        FabriqueNative::new()
    }
}

impl<J: Jeu + Epreuve> Fabrique for FabriqueNative<J> {
    type I = InstanceNative<J>;

    fn neuve(&mut self) -> Result<InstanceNative<J>, PanneOutil> {
        Ok(InstanceNative::new())
    }
}

/// Le message d'une panique rattrapée.
fn message_de(p: &(dyn Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        String::from("panique sans message")
    }
}

impl<J: Jeu + Epreuve> Instance for InstanceNative<J> {
    type Instantane = InstantaneNatif<J>;

    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8], _budget: u64) -> Result<Vec<u8>, Echec> {
        let modes = SansModes;
        let cadre = Cadre {
            modes: &modes,
            zones: Zones::default(),
        };
        let mut retour = Sortie::new(cadre, sorte);
        let (hote, enveloppe) = (&self.hote, &mut self.enveloppe);
        // Une erreur de la colle est déjà écrite dans la sortie, comme `Retour::Illisible` :
        // c'est au cœur de la lire.
        match catch_unwind(AssertUnwindSafe(|| {
            appeler(hote, enveloppe, appel, &mut retour)
        })) {
            Ok(_) => Ok(retour.octets),
            Err(p) => Err(Echec::Faute(Faute::Panique {
                message: message_de(p.as_ref()),
            })),
        }
    }

    fn carburant(&self) -> Option<u64> {
        None
    }

    fn instantane(&mut self) -> Result<InstantaneNatif<J>, PanneOutil> {
        Ok(InstantaneNatif(self.enveloppe.clone()))
    }

    fn restaurer(&mut self, i: &InstantaneNatif<J>) -> Result<(), PanneOutil> {
        self.enveloppe.clone_from(&i.0);
        Ok(())
    }

    fn empreinte(&mut self) -> Result<[u8; 32], PanneOutil> {
        let mut h = blake3::Hasher::new();
        match &self.enveloppe {
            None => {
                let _: &mut blake3::Hasher = h.update(b"jeu-natif : aucune enveloppe");
            }
            Some(e) => {
                let c = canonique(&self.hote, e)
                    .map_err(|err| PanneOutil::new(CausePanne::Interne, err.to_string()))?;
                let _: &mut blake3::Hasher = h.update(&c);
            }
        }
        Ok(*h.finalize().as_bytes())
    }
}
