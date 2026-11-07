//! Le geste en cours (PHASE-3, § 2.3, P39) : avant chaque appel `Entree` de la partie
//! qui fait foi, l'autorité dit à la page quel geste elle donne au jeu.
//!
//! Le Worker est le seul fil qu'on peut tuer, et un appel WebAssembly ne s'interrompt pas
//! sur son propre fil : quand un appel ne rend plus la main, la page tue le Worker. Ce que
//! le Worker n'a pas écrit est perdu, l'entrée en cours comprise ; le téléphone, sans
//! accusé, la renvoie, et elle bloquerait de nouveau. Pour l'écarter après deux arrêts
//! (`chien::Chien`), la page doit savoir laquelle c'était, et le savoir **avant** l'appel :
//! après, il est trop tard. [`FabriqueQuiDit`] enveloppe la fabrique des instances et
//! appelle `dire` juste avant chaque `Entree`, avec l'identité du geste qu'elle porte
//! (`QuandFil::id`), ou `None` pour une autre entrée (le temps, l'écran, un tenu, une
//! présence, une réponse) ; dans le Worker, `dire` poste `VersPage::EnCours` à la page,
//! un message qui part même si le Worker se fige ensuite.
//!
//! Les instances de la tête (`Fabrique::neuve_tete`) ne disent rien : elles ne jouent
//! que des copies de la définitive, et les trois jeux n'ont pas de fenêtre.

use std::rc::Rc;

use jeu_abi::{Appel, EntreeFil, IdGeste, SorteAppel, decoder};
use partie::{Echec, Fabrique, Instance, PanneOutil};

/// Ce qu'on dit avant chaque `Entree` : le geste en cours, ou `None`.
pub type Dire = Rc<dyn Fn(Option<IdGeste>)>;

/// Le geste d'un appel `Entree` encodé, s'il en porte un ; `None` pour tout autre appel,
/// ou une autre entrée.
pub fn geste_de_l_appel(appel: &[u8]) -> Option<IdGeste> {
    let Ok(Appel::Entree { entree, .. }) = decoder::<Appel<'_>>(appel) else {
        return None;
    };
    match decoder::<EntreeFil<'_>>(entree) {
        Ok(EntreeFil::Geste { quand, .. }) => Some(IdGeste::du_fil(quand.id)),
        Ok(
            EntreeFil::Temps { .. }
            | EntreeFil::Ecran { .. }
            | EntreeFil::Tenu { .. }
            | EntreeFil::Manette { .. }
            | EntreeFil::Presence(_)
            | EntreeFil::Reponse { .. },
        )
        | Err(_) => None,
    }
}

/// Une fabrique qui dit le geste en cours avant chaque `Entree` de ses instances.
pub struct FabriqueQuiDit<F> {
    fabrique: F,
    dire: Dire,
}

impl<F> FabriqueQuiDit<F> {
    /// Enveloppe `fabrique` ; `dire` est appelée avant chaque `Entree`.
    pub fn new(fabrique: F, dire: Dire) -> FabriqueQuiDit<F> {
        FabriqueQuiDit { fabrique, dire }
    }
}

/// Une instance qui dit le geste en cours, ou se tait (la tête).
pub struct InstanceQuiDit<I> {
    instance: I,
    dire: Option<Dire>,
}

impl<F: Fabrique> Fabrique for FabriqueQuiDit<F> {
    type I = InstanceQuiDit<F::I>;

    fn neuve(&mut self) -> Result<Self::I, PanneOutil> {
        Ok(InstanceQuiDit {
            instance: self.fabrique.neuve()?,
            dire: Some(Rc::clone(&self.dire)),
        })
    }

    fn neuve_tete(&mut self) -> Result<Self::I, PanneOutil> {
        Ok(InstanceQuiDit {
            instance: self.fabrique.neuve_tete()?,
            dire: None,
        })
    }
}

impl<I: Instance> Instance for InstanceQuiDit<I> {
    type Instantane = I::Instantane;

    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8], budget: u64) -> Result<Vec<u8>, Echec> {
        if let Some(dire) = &self.dire
            && sorte == SorteAppel::Ecriture
            && let Ok(Appel::Entree { .. }) = decoder::<Appel<'_>>(appel)
        {
            dire(geste_de_l_appel(appel));
        }
        self.instance.appeler(sorte, appel, budget)
    }

    fn carburant(&self) -> Option<u64> {
        self.instance.carburant()
    }

    fn instantane(&mut self) -> Result<Self::Instantane, PanneOutil> {
        self.instance.instantane()
    }

    fn restaurer(&mut self, i: &Self::Instantane) -> Result<(), PanneOutil> {
        self.instance.restaurer(i)
    }

    fn empreinte(&mut self) -> Result<[u8; 32], PanneOutil> {
        self.instance.empreinte()
    }
}
