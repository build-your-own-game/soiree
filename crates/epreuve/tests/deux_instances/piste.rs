//! Une fabrique et une instance qui en enveloppent d'autres, pour n'importe quelle
//! sorte d'instance : elles comptent ce que le cœur leur demande, et injectent des
//! fautes du jeu là où une règle le dit.
//!
//! C'est l'équivalent, pour la copie native et le module, des compteurs de
//! `partie::modele::FabriqueModele` : sans rien changer à `jeu-natif` ni à
//! `moteur-wasmtime`, `sans_fenetre_une_seule_instance` et
//! `faute_isolee_ne_se_rejoue_pas` (§ 8.6) se vérifient sur les vraies instances.
//!
//! Une faute injectée imite au plus près une faute réelle : « écrire puis fauter »
//! laisse l'appel modifier la mémoire de l'instance avant de dire qu'il a fauté,
//! comme une panique au milieu d'une étape ; le cœur doit alors restaurer, pas
//! seulement écarter (§ 4.6). Sur le module, « budget » donne à l'appel un carburant
//! trop petit : la faute est réelle, et l'instance s'arrête au milieu de l'étape.

use std::cell::RefCell;
use std::rc::Rc;

use jeu_abi::{Appel, SorteAppel, decoder};
use partie::{Echec, Fabrique, Faute, Instance, PanneOutil};

/// Ce que fait une règle quand elle se déclenche.
#[derive(Debug, Clone)]
pub enum Effet {
    /// L'appel passe, puis l'instance dit qu'elle a fauté : sa mémoire est à moitié
    /// écrite, comme après une panique au milieu d'une étape.
    EcrireEtFauter(Faute),
    /// L'appel ne passe pas : la faute tout de suite.
    Fauter(Faute),
    /// L'appel passe sous ce carburant au lieu du sien : il doit fauter de lui-même,
    /// par [`Faute::Budget`]. Le module seulement : la copie native n'a pas de
    /// carburant.
    Budget(u64),
}

/// Une faute à injecter : sur quels appels, quoi, combien de fois.
pub struct Regle {
    /// Son nom, relevé à chaque déclenchement.
    pub nom: &'static str,
    /// Les appels qu'elle vise.
    pub cible: Box<dyn Fn(&Appel<'_>) -> bool>,
    /// Ce qu'elle fait.
    pub effet: Effet,
    /// Les déclenchements qui restent.
    pub fois: u32,
}

/// Ce que les instances d'une fabrique ont reçu.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Releve {
    /// Les instances fabriquées.
    pub neuves: u64,
    /// Les instantanés pris.
    pub instantanes: u64,
    /// Les restaurations.
    pub restaurations: u64,
    /// L'ordre de chaque `Appel::Entree` reçu, dans l'ordre des appels.
    pub entrees: Vec<u64>,
    /// Le nom de chaque règle déclenchée, dans l'ordre.
    pub declenchees: Vec<&'static str>,
}

impl Releve {
    /// Combien d'`Appel::Entree` ont porté cet ordre.
    pub fn appels_de(&self, ordre: u64) -> usize {
        self.entrees.iter().filter(|o| **o == ordre).count()
    }
}

#[derive(Default)]
struct Etat {
    regles: Vec<Regle>,
    releve: Releve,
}

/// Une fabrique pistée.
pub struct Piste<F> {
    fabrique: F,
    etat: Rc<RefCell<Etat>>,
}

impl<F> Piste<F> {
    /// Une fabrique pistée, avec ces règles.
    pub fn new(fabrique: F, regles: Vec<Regle>) -> Piste<F> {
        Piste {
            fabrique,
            etat: Rc::new(RefCell::new(Etat {
                regles,
                releve: Releve::default(),
            })),
        }
    }

    /// Ce que les instances de cette fabrique ont reçu jusqu'ici.
    pub fn releve(&self) -> Releve {
        self.etat.borrow().releve.clone()
    }
}

impl<F: Fabrique> Fabrique for Piste<F> {
    type I = InstancePiste<F::I>;

    fn neuve(&mut self) -> Result<InstancePiste<F::I>, PanneOutil> {
        self.etat.borrow_mut().releve.neuves += 1;
        Ok(InstancePiste {
            instance: self.fabrique.neuve()?,
            etat: Rc::clone(&self.etat),
        })
    }
}

/// Une instance pistée.
pub struct InstancePiste<I> {
    instance: I,
    etat: Rc<RefCell<Etat>>,
}

impl<I: Instance> Instance for InstancePiste<I> {
    type Instantane = I::Instantane;

    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8], budget: u64) -> Result<Vec<u8>, Echec> {
        let effet = {
            let mut etat = self.etat.borrow_mut();
            let decode = decoder::<Appel<'_>>(appel).ok();
            if let Some(Appel::Entree { ordre, .. }) = &decode {
                etat.releve.entrees.push(*ordre);
            }
            let mut effet = None;
            if let Some(a) = &decode {
                let Etat { regles, releve } = &mut *etat;
                if let Some(r) = regles.iter_mut().find(|r| r.fois > 0 && (r.cible)(a)) {
                    r.fois -= 1;
                    releve.declenchees.push(r.nom);
                    effet = Some(r.effet.clone());
                }
            }
            effet
        };
        match effet {
            None => self.instance.appeler(sorte, appel, budget),
            Some(Effet::Fauter(f)) => Err(Echec::Faute(f)),
            Some(Effet::EcrireEtFauter(f)) => {
                let _: Result<Vec<u8>, Echec> = self.instance.appeler(sorte, appel, budget);
                Err(Echec::Faute(f))
            }
            Some(Effet::Budget(b)) => match self.instance.appeler(sorte, appel, b) {
                Err(Echec::Faute(Faute::Budget)) => Err(Echec::Faute(Faute::Budget)),
                autre => panic!("un carburant de {b} n'a pas fait fauter l'appel : {autre:?}"),
            },
        }
    }

    fn carburant(&self) -> Option<u64> {
        self.instance.carburant()
    }

    fn instantane(&mut self) -> Result<I::Instantane, PanneOutil> {
        self.etat.borrow_mut().releve.instantanes += 1;
        self.instance.instantane()
    }

    fn restaurer(&mut self, i: &I::Instantane) -> Result<(), PanneOutil> {
        self.etat.borrow_mut().releve.restaurations += 1;
        self.instance.restaurer(i)
    }

    fn empreinte(&mut self) -> Result<[u8; 32], PanneOutil> {
        self.instance.empreinte()
    }
}
