//! La tête (§ 4.2, § 4.4, D12) : une seconde instance, copiée de la définitive et
//! rejouée sur la fenêtre, pour la scène de l'écran et ses effets `Provisoire`.
//!
//! # Ce que la tête fait, et ce qu'elle ne fait pas
//!
//! La tête joue les mêmes pas que la définitive jouera, de la même façon
//! (`Partie::jouer_pas`), mais en mode [`Mode::Tete`] : rien n'est écrit au journal,
//! aucun avis, aucune demande, aucune sortie, aucun point de reprise. Le cœur qu'elle
//! fait avancer est une copie de celui de la définitive, jetée au prochain rejeu.
//! Les arrivées placées dans la fenêtre restent à la définitive, qui les jouera à son
//! tour : la tête n'en voit qu'une copie.
//!
//! # Quand elle se rejoue
//!
//! La tête continue d'un appel à l'autre tant que rien n'arrive à une place qu'elle a
//! déjà jouée. Sinon (un geste en retard, une faute de la définitive, une nouvelle
//! partie, un saut), elle repart d'un instantané de la définitive et rejoue la fenêtre
//! jusqu'à la cible (§ 4.4).
//!
//! # Une faute sur la tête
//!
//! La tête se reconstruit depuis la définitive, sans l'entrée fautive ; la définitive
//! la jugera à son tour (§ 4.6). L'entrée reste écartée de la tête jusqu'à ce que la
//! définitive dépasse son pas. Après [`ESSAIS_MAX`] fautes dans un même appel, la tête
//! cesse d'être montrée jusqu'au prochain appel : l'écran montre la définitive.

use alloc::vec::Vec;

use jeu_abi::{Appel, Pas, RegardFil, Retour, SceneFil, decoder};

use crate::coeur::EtatCoeur;
use crate::instance::{Echec, Fabrique, Instance, PanneOutil};
use crate::journal::Journal;
use crate::partie::{Mode, Partie, accueillir, appeler, illisible};

/// Les reconstructions de la tête après une faute, au plus, dans un même appel.
pub(crate) const ESSAIS_MAX: u32 = 8;

/// La tête séparée.
pub(crate) struct TeteSeparee<I> {
    /// L'instance ; `None` le temps qu'un pas s'y joue.
    pub(crate) instance: Option<I>,
    /// Le dernier pas joué sur la tête.
    pub(crate) pas: Pas,
    /// Le cœur de la tête : une copie de celui de la définitive, avancée avec elle.
    pub(crate) coeur: EtatCoeur,
    /// Une arrivée, une faute, une nouvelle partie l'ont rendue fausse : elle repart
    /// d'un instantané de la définitive au prochain appel.
    pub(crate) a_rejouer: bool,
    /// Peut-elle être montrée ? Non après trop de fautes dans un même appel.
    pub(crate) valide: bool,
    /// Les entrées qui ont fauté sur la tête, par pas et par octets : la tête ne les
    /// rejoue plus ; la définitive les jugera.
    pub(crate) ecartees: Vec<(Pas, Vec<u8>)>,
}

impl<I> TeteSeparee<I> {
    /// La tête est-elle en avance sur la définitive, et montrable ?
    pub(crate) fn montrable(&self, definitif: Pas) -> bool {
        self.valide && !self.a_rejouer && self.pas > definitif && self.instance.is_some()
    }
}

impl<F: Fabrique> Partie<F> {
    /// La tête doit repartir de la définitive au prochain appel.
    pub(crate) fn tete_a_rejouer(&mut self) {
        if let Some(t) = self.tete.as_mut() {
            t.a_rejouer = true;
        }
    }

    /// Amène la tête jusqu'à `jusqua` (§ 4.4) : elle continue si rien n'est arrivé
    /// derrière elle, sinon elle repart d'un instantané de la définitive. Une faute la
    /// reconstruit sans l'entrée fautive, [`ESSAIS_MAX`] fois au plus.
    pub(crate) fn avancer_la_tete(&mut self, jusqua: Pas) -> Result<(), PanneOutil> {
        let definitif = self.coeur.pas;
        if let Some(t) = self.tete.as_mut() {
            t.ecartees.retain(|(p, _)| *p > definitif);
            t.valide = true;
        }
        for _ in 0..ESSAIS_MAX {
            let repartir = self
                .tete
                .as_ref()
                .is_none_or(|t| t.a_rejouer || t.pas < definitif || t.instance.is_none());
            if repartir {
                self.repartir_de_la_definitive()?;
            }
            if self.jouer_sur_la_tete(jusqua)? {
                return Ok(());
            }
        }
        if let Some(t) = self.tete.as_mut() {
            t.valide = false;
            t.a_rejouer = true;
        }
        Ok(())
    }

    /// La tête devient une copie de la définitive, à son pas.
    fn repartir_de_la_definitive(&mut self) -> Result<(), PanneOutil> {
        let instantane = self.definitive.instantane()?;
        let (instance, ecartees) = match self.tete.take() {
            Some(TeteSeparee {
                instance: Some(i),
                ecartees,
                ..
            }) => (i, ecartees),
            Some(TeteSeparee { ecartees, .. }) => (self.tete_neuve()?, ecartees),
            None => (self.tete_neuve()?, Vec::new()),
        };
        let mut instance = instance;
        instance.restaurer(&instantane)?;
        self.tete = Some(TeteSeparee {
            instance: Some(instance),
            pas: self.coeur.pas,
            coeur: self.coeur.clone(),
            a_rejouer: false,
            valide: true,
            ecartees,
        });
        Ok(())
    }

    /// Une instance neuve pour la tête, accueillie (`Bonjour`, `Decrire`).
    fn tete_neuve(&mut self) -> Result<F::I, PanneOutil> {
        let mut i = self.fabrique.neuve_tete()?;
        match accueillir(&mut i, &self.ouverture.budgets) {
            Ok(_) => Ok(i),
            Err(crate::partie::RefusOuverture::Panne(p)) => Err(p),
            Err(autre) => Err(PanneOutil::new(
                crate::instance::CausePanne::Interne,
                alloc::format!("la tête ne s'accueille pas : {autre:?}"),
            )),
        }
    }

    /// Joue la tête jusqu'à `jusqua`. Rend `false` si une entrée y a fauté : elle est
    /// alors notée écartée de la tête, et la tête est à rejouer.
    ///
    /// Le pas se joue par le même chemin que sur la définitive : l'instance, le cœur,
    /// le journal et les arrivées de la définitive sont mis de côté le temps du rejeu,
    /// et remis ensuite.
    fn jouer_sur_la_tete(&mut self, jusqua: Pas) -> Result<bool, PanneOutil> {
        let Some(mut t) = self.tete.take() else {
            return Ok(true);
        };
        let Some(mut instance) = t.instance.take() else {
            t.a_rejouer = true;
            self.tete = Some(t);
            return Ok(true);
        };
        core::mem::swap(&mut self.definitive, &mut instance);
        core::mem::swap(&mut self.coeur, &mut t.coeur);
        let journal = core::mem::replace(&mut self.journal, Journal::vide(self.coeur.pas));
        let curseur = self.curseur.take();
        let deja = t.pas;
        let arrivees: Vec<_> = self
            .en_attente
            .iter()
            .filter(|a| a.place > deja)
            .cloned()
            .collect();
        let en_attente = core::mem::replace(&mut self.en_attente, arrivees);
        let ecartees_tete = core::mem::take(&mut t.ecartees);
        self.mode = Mode::Tete;
        self.tete_fautive = None;
        self.tete_ecartees = ecartees_tete;
        let mut rendu = Ok(());
        while self.coeur.pas < jusqua && self.tete_fautive.is_none() {
            let s = self.suivant();
            if let Err(p) = self.jouer_pas(s) {
                rendu = Err(p);
                break;
            }
        }
        self.mode = Mode::Definitive;
        let fautive = self.tete_fautive.take();
        t.ecartees = core::mem::take(&mut self.tete_ecartees);
        core::mem::swap(&mut self.definitive, &mut instance);
        core::mem::swap(&mut self.coeur, &mut t.coeur);
        self.journal = journal;
        self.curseur = curseur;
        self.en_attente = en_attente;
        t.pas = t.coeur.pas;
        t.instance = Some(instance);
        let reussi = fautive.is_none();
        if let Some(f) = fautive {
            t.ecartees.push(f);
            t.a_rejouer = true;
        }
        self.tete = Some(t);
        rendu.map(|()| reussi)
    }

    /// Sur la tête : cette entrée a-t-elle déjà fauté ? Elle ne se rejoue plus.
    pub(crate) fn ecartee_de_la_tete(&self, s: Pas, octets: &[u8]) -> bool {
        self.mode == Mode::Tete
            && self
                .tete_ecartees
                .iter()
                .any(|(p, o)| *p == s && o.as_slice() == octets)
    }

    /// La scène de la tête, si elle est en avance et montrable : lue sur elle, avec son
    /// pas. Une faute de la lecture la rend à rejouer, et l'écran montre la définitive.
    pub(crate) fn scene_de_la_tete(&mut self) -> Result<Option<(SceneFil, Pas)>, PanneOutil> {
        let definitif = self.coeur.pas;
        let budget = self.ouverture.budgets.lecture;
        let Some(t) = self.tete.as_mut() else {
            return Ok(None);
        };
        if !t.montrable(definitif) {
            return Ok(None);
        }
        let Some(instance) = t.instance.as_mut() else {
            return Ok(None);
        };
        let appel = Appel::Regarder {
            regards: alloc::vec![RegardFil::Ecran],
        };
        let r = match appeler(instance, &appel, budget) {
            Ok(r) => r,
            Err(Echec::Faute(_)) => {
                t.a_rejouer = true;
                return Ok(None);
            }
            Err(Echec::Panne(p)) => return Err(p),
        };
        match decoder::<Retour<'_>>(&r) {
            Ok(Retour::Vues(v)) => {
                let Some(vue) = v.first() else {
                    t.a_rejouer = true;
                    return Ok(None);
                };
                if let Ok(s) = decoder::<SceneFil>(vue.octets) {
                    Ok(Some((s, t.pas)))
                } else {
                    t.a_rejouer = true;
                    Ok(None)
                }
            }
            Ok(Retour::Illisible(e)) => Err(illisible(e)),
            Ok(_) | Err(_) => {
                t.a_rejouer = true;
                Ok(None)
            }
        }
    }
}
