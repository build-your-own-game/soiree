//! La copie native d'un jeu, vue sans son type (§ 8.8, `natif: Option<&mut dyn
//! FabriqueDynamique>`).
//!
//! `epreuve` ne dépend d'aucun jeu : la copie native lui arrive comme une fabrique
//! d'instances dont le type est effacé. Toute [`partie::Fabrique`] en est une ; une
//! crate de jeu appelle [`crate::ligne_de_commande`] avec `FabriqueNative<SonJeu>`.

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use jeu_abi::SorteAppel;
use partie::{CausePanne, Echec, Fabrique, Instance, PanneOutil};

/// Une instance dont le type est effacé.
pub trait InstanceDynamique {
    /// [`Instance::appeler`].
    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8], budget: u64) -> Result<Vec<u8>, Echec>;
    /// [`Instance::carburant`].
    fn carburant(&self) -> Option<u64>;
    /// [`Instance::instantane`], effacé.
    fn instantane(&mut self) -> Result<Rc<dyn Any>, PanneOutil>;
    /// [`Instance::restaurer`] ; refuse un instantané d'une autre sorte d'instance.
    fn restaurer(&mut self, i: &dyn Any) -> Result<(), PanneOutil>;
    /// [`Instance::empreinte`].
    fn empreinte(&mut self) -> Result<[u8; 32], PanneOutil>;
}

impl<I> InstanceDynamique for I
where
    I: Instance + 'static,
    I::Instantane: 'static,
{
    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8], budget: u64) -> Result<Vec<u8>, Echec> {
        Instance::appeler(self, sorte, appel, budget)
    }

    fn carburant(&self) -> Option<u64> {
        Instance::carburant(self)
    }

    fn instantane(&mut self) -> Result<Rc<dyn Any>, PanneOutil> {
        let i = Instance::instantane(self)?;
        Ok(Rc::new(i))
    }

    fn restaurer(&mut self, i: &dyn Any) -> Result<(), PanneOutil> {
        let i = i.downcast_ref::<I::Instantane>().ok_or_else(|| {
            PanneOutil::new(
                CausePanne::Interne,
                "instantané d'une autre sorte d'instance",
            )
        })?;
        Instance::restaurer(self, i)
    }

    fn empreinte(&mut self) -> Result<[u8; 32], PanneOutil> {
        Instance::empreinte(self)
    }
}

/// Une fabrique dont le type est effacé : la copie native d'un jeu.
pub trait FabriqueDynamique {
    /// [`Fabrique::neuve`].
    fn neuve(&mut self) -> Result<Box<dyn InstanceDynamique>, PanneOutil>;
}

impl<F> FabriqueDynamique for F
where
    F: Fabrique,
    F::I: 'static,
    <F::I as Instance>::Instantane: 'static,
{
    fn neuve(&mut self) -> Result<Box<dyn InstanceDynamique>, PanneOutil> {
        Ok(Box::new(Fabrique::neuve(self)?))
    }
}

/// Une fabrique effacée, rendue à [`partie::Fabrique`] pour qu'une `Partie` y joue.
/// Elle se partage : une reprise prend une fabrique neuve, sur la même copie native.
#[derive(Clone)]
pub(crate) struct Effacee<'a>(pub(crate) Rc<RefCell<&'a mut dyn FabriqueDynamique>>);

/// Une instance effacée, comme [`partie::Instance`].
pub(crate) struct InstanceEffacee(Box<dyn InstanceDynamique>);

impl Fabrique for Effacee<'_> {
    type I = InstanceEffacee;

    fn neuve(&mut self) -> Result<InstanceEffacee, PanneOutil> {
        Ok(InstanceEffacee(self.0.borrow_mut().neuve()?))
    }
}

impl Instance for InstanceEffacee {
    type Instantane = Rc<dyn Any>;

    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8], budget: u64) -> Result<Vec<u8>, Echec> {
        self.0.appeler(sorte, appel, budget)
    }

    fn carburant(&self) -> Option<u64> {
        self.0.carburant()
    }

    fn instantane(&mut self) -> Result<Rc<dyn Any>, PanneOutil> {
        self.0.instantane()
    }

    fn restaurer(&mut self, i: &Rc<dyn Any>) -> Result<(), PanneOutil> {
        self.0.restaurer(i.as_ref())
    }

    fn empreinte(&mut self) -> Result<[u8; 32], PanneOutil> {
        self.0.empreinte()
    }
}
