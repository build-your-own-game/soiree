//! La file de ce que le relais envoie sur un lien (PHASE-3, § 2.5) : **une file fiable
//! bornée en octets** (SAL-06), et **une place par canal remplaçable**, où la dernière
//! trame d'un canal remplace celle qui attend (SAL-70, SAL-122). Une file fiable pleine
//! ne jette rien en silence : elle le dit, et la coque ferme le lien (SAL-07) ; le
//! téléphone revient et reçoit l'état entier.
//!
//! Sans entrées-sorties : la coque la partage entre le fil qui possède les salles (il
//! pousse) et le fil d'écriture du lien (il prend).

use std::collections::VecDeque;

/// Une place remplaçable : la trame d'un canal, venue d'un lien (sur le lien de
/// l'autorité, chaque appareil a ses propres canaux).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Place {
    /// Le lien d'où vient la trame (0 pour le relais lui-même).
    pub origine: u32,
    /// Le canal.
    pub canal: u8,
}

/// Ce qui attend de partir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Envoi {
    /// Une trame de la file fiable.
    Fiable(Vec<u8>),
    /// La dernière trame d'une place remplaçable.
    Remplacable(Place, Vec<u8>),
    /// Un battement : un `Ping` de WebSocket, auquel tout client répond par un `Pong`
    /// (RFC 6455, § 5.5.2) ; il ne compte pas dans la borne.
    Battement,
    /// La fin : ce qui précède part, puis la trame de fermeture.
    Fin,
}

/// La file fiable est pleine : le lien doit se fermer (SAL-07).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilePleine {
    /// Les octets qui y attendaient.
    pub en_attente: usize,
}

/// La file d'un lien.
#[derive(Debug)]
pub struct FileSortante {
    borne: usize,
    envois: VecDeque<Envoi>,
    octets_fiables: usize,
    octets_remplacables: usize,
    fermee: bool,
}

impl FileSortante {
    /// Une file vide, bornée à `borne` octets fiables.
    pub fn new(borne: usize) -> FileSortante {
        FileSortante {
            borne,
            envois: VecDeque::new(),
            octets_fiables: 0,
            octets_remplacables: 0,
            fermee: false,
        }
    }

    /// Sa borne, en octets fiables.
    pub const fn borne(&self) -> usize {
        self.borne
    }

    /// Change la borne (le lien de la garde a la sienne, PHASE-3, § 2.4). Ce qui
    /// attend déjà n'est pas jugé de nouveau.
    pub const fn changer_de_borne(&mut self, borne: usize) {
        self.borne = borne;
    }

    /// Les octets qui attendent, fiables et remplaçables.
    pub const fn en_attente(&self) -> usize {
        self.octets_fiables.saturating_add(self.octets_remplacables)
    }

    /// Les octets fiables qui attendent.
    pub const fn fiables(&self) -> usize {
        self.octets_fiables
    }

    /// Rien n'attend.
    pub fn est_vide(&self) -> bool {
        self.envois.is_empty()
    }

    /// La file a reçu sa fin : plus rien ne s'y ajoute.
    pub const fn est_fermee(&self) -> bool {
        self.fermee
    }

    /// Ajoute une trame fiable. Pleine, rien n'est ajouté, et la file le dit : la coque
    /// ferme le lien (SAL-07). Une file fermée ne prend plus rien, sans erreur.
    pub fn fiable(&mut self, octets: Vec<u8>) -> Result<(), FilePleine> {
        if self.fermee {
            return Ok(());
        }
        let total = self.octets_fiables.saturating_add(octets.len());
        if total > self.borne {
            return Err(FilePleine {
                en_attente: self.octets_fiables,
            });
        }
        self.octets_fiables = total;
        self.envois.push_back(Envoi::Fiable(octets));
        Ok(())
    }

    /// Ajoute la dernière trame d'une place remplaçable : elle prend la place de celle
    /// qui attend encore, au même rang, ou va à la fin (SAL-70).
    pub fn remplacable(&mut self, place: Place, octets: Vec<u8>) {
        if self.fermee {
            return;
        }
        let nouveaux = octets.len();
        for e in &mut self.envois {
            if let Envoi::Remplacable(p, ancien) = e
                && *p == place
            {
                self.octets_remplacables = self
                    .octets_remplacables
                    .saturating_sub(ancien.len())
                    .saturating_add(nouveaux);
                *ancien = octets;
                return;
            }
        }
        self.octets_remplacables = self.octets_remplacables.saturating_add(nouveaux);
        self.envois.push_back(Envoi::Remplacable(place, octets));
    }

    /// Ajoute un battement, s'il n'y en a pas déjà un qui attend.
    pub fn battement(&mut self) {
        if self.fermee || self.envois.iter().any(|e| *e == Envoi::Battement) {
            return;
        }
        self.envois.push_back(Envoi::Battement);
    }

    /// La fin : ce qui attend part, puis la fermeture ; rien ne s'ajoute plus.
    pub fn fermer(&mut self) {
        if self.fermee {
            return;
        }
        self.fermee = true;
        self.envois.push_back(Envoi::Fin);
    }

    /// La fin, tout de suite : ce qui attend est jeté (la file pleine, le lien perdu).
    pub fn fermer_sans_attendre(&mut self) {
        self.envois.clear();
        self.octets_fiables = 0;
        self.octets_remplacables = 0;
        self.fermee = true;
        self.envois.push_back(Envoi::Fin);
    }

    /// Le prochain envoi, dans l'ordre.
    pub fn prendre(&mut self) -> Option<Envoi> {
        let e = self.envois.pop_front()?;
        match &e {
            Envoi::Fiable(o) => {
                self.octets_fiables = self.octets_fiables.saturating_sub(o.len());
            }
            Envoi::Remplacable(_, o) => {
                self.octets_remplacables = self.octets_remplacables.saturating_sub(o.len());
            }
            Envoi::Battement | Envoi::Fin => {}
        }
        Some(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SAL-06, SAL-07 : la file fiable a une borne en octets ; pleine, elle refuse et le
    /// dit, sans rien jeter de ce qui attend.
    #[test]
    fn une_file_fiable_pleine_refuse_sans_rien_jeter_sal06_sal07() {
        let mut f = FileSortante::new(10);
        assert_eq!(f.fiable(vec![1; 6]), Ok(()));
        assert_eq!(f.fiable(vec![2; 4]), Ok(()));
        assert_eq!(f.fiable(vec![3; 1]), Err(FilePleine { en_attente: 10 }));
        assert_eq!(f.fiables(), 10);
        assert_eq!(f.prendre(), Some(Envoi::Fiable(vec![1; 6])));
        assert_eq!(f.fiable(vec![3; 1]), Ok(()));
        assert_eq!(f.prendre(), Some(Envoi::Fiable(vec![2; 4])));
        assert_eq!(f.prendre(), Some(Envoi::Fiable(vec![3; 1])));
        assert_eq!(f.prendre(), None);
        assert_eq!(f.en_attente(), 0);
    }

    /// SAL-70 : la dernière trame d'une place remplace celle qui attend, au même rang ;
    /// une autre place, ou une autre origine, ne la touche pas.
    #[test]
    fn la_derniere_trame_d_une_place_remplace_celle_qui_attend_sal70() {
        let mut f = FileSortante::new(100);
        let a = Place {
            origine: 3,
            canal: 0,
        };
        let b = Place {
            origine: 4,
            canal: 0,
        };
        f.remplacable(a, vec![1]);
        assert_eq!(f.fiable(vec![9]), Ok(()));
        f.remplacable(b, vec![2]);
        f.remplacable(a, vec![5, 5]);
        assert_eq!(f.en_attente(), 4);
        assert_eq!(f.prendre(), Some(Envoi::Remplacable(a, vec![5, 5])));
        assert_eq!(f.prendre(), Some(Envoi::Fiable(vec![9])));
        assert_eq!(f.prendre(), Some(Envoi::Remplacable(b, vec![2])));
        f.remplacable(a, vec![6]);
        assert_eq!(f.prendre(), Some(Envoi::Remplacable(a, vec![6])));
    }

    #[test]
    fn la_fin_passe_apres_ce_qui_attend_et_ferme_la_file() {
        let mut f = FileSortante::new(100);
        assert_eq!(f.fiable(vec![1]), Ok(()));
        f.battement();
        f.battement();
        f.fermer();
        assert_eq!(f.fiable(vec![2]), Ok(()));
        assert_eq!(f.prendre(), Some(Envoi::Fiable(vec![1])));
        assert_eq!(f.prendre(), Some(Envoi::Battement));
        assert_eq!(f.prendre(), Some(Envoi::Fin));
        assert_eq!(f.prendre(), None);
        assert!(f.est_fermee());
    }
}
