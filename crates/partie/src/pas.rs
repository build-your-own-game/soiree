//! Jouer un pas sur la définitive (§ 4.3, § 4.4, § 4.6), et le rejouer.
//!
//! Un pas se joue toujours de la même façon, en direct comme au rejeu :
//!
//! 1. les arrivées placées à ce pas, rangées par [`regles::ordonner`] ;
//! 2. d'abord ce que le cœur synthétise : les relâchements d'une pause qui
//!    commence, puis, tenu par tenu, un relâchement par silence ou une valeur
//!    retenue par le débit ;
//! 3. puis chaque arrivée, dans l'ordre : un geste, un tenu s'il change la valeur
//!    effective, une entrée de l'écran, une présence (précédée des relâchements
//!    d'un départ), une réponse à une demande encore en cours ;
//! 4. puis `Temps`, si le réveil le demande, hors pause et hors panne ;
//! 5. enfin les sorties, la lecture de l'état, les demandes, et le point de
//!    reprise s'il est dû.
//!
//! En direct, chaque ligne est écrite au journal. Au rejeu, chaque ligne produite
//! doit être celle du journal, et une entrée que le journal dit écartée n'est pas
//! rejouée.

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;

use jeu_abi::{
    Appareil, Appel, CauseTenu, Duree, EntreeFil, IssueFil, Oui, Pas, QuandFil, Retour, ReveilFil,
    SortieFil, decoder,
};

use crate::coeur::DemandeEnCours;
use crate::instance::{CausePanne, Echec, Fabrique, Faute, Instance, PanneOutil};
use crate::journal::{ArriveePlacee, IssueJournal, Ligne, LigneEntree, QuoiPlace};
use crate::partie::{
    Avancee, Destinataire, LieuFaute, Lu, Mode, Partie, RapportFaute, RefusOuverture, accueillir,
    appeler, creer, encoder_vers_vec, heure_du_pas, illisible,
};
use crate::regles::{self, ArriveeTenu, IssueTenu, Livraison, Origine};

/// Le retard de vue, en pas, au-delà duquel une arrivée vient d'un arriéré : une
/// seconde (PHASE-3, § 2.11, P7). Choisi : une vue arrive d'habitude en quelques pas, et
/// une retransmission de WebSocket la retient de l'ordre de 300 ms. La seule
/// définition : l'épreuve la relit.
pub const ARRIERE_PAS: u64 = 60;

/// Les entrées d'un arriéré qu'un tour de la boucle joue au plus (PHASE-3, § 2.11, P7).
/// Choisi, à remplacer par M1 (le coût fixe d'un arriéré) et M13 ; indépendant de la
/// machine : le rejeu ne dépend pas d'une durée.
pub const ARRIERES_PAR_TOUR: u32 = 8;

/// Une entrée à jouer sur la définitive.
pub(crate) struct AJouer {
    /// L'[`EntreeFil`] encodée.
    octets: Vec<u8>,
    /// À qui va l'avis d'un refus ou d'un écart.
    pour: Option<Destinataire>,
    /// Est-ce un `Temps` ?
    temps: Oui,
    /// Vient-elle d'une arrivée (et non de ce que le cœur synthétise) ?
    arrivee: Oui,
    /// Vient-elle d'un arriéré ([`est_un_arriere`]) ?
    arriere: Oui,
}

/// Ce qu'un pas a produit.
pub(crate) struct Bilan {
    acceptee: Oui,
    sorties: Vec<SortieFil>,
}

/// Un pas commencé sur la définitive et pas encore fini : ce qu'un tour de
/// [`Partie::avancer_borne`] laisse quand il a joué ses arriérés (P7). Ses arrivées
/// sont figées : celles qui arrivent ensuite vont après la fin de l'avancée en cours
/// ([`AvanceeEnCours::fin`]).
pub(crate) struct PasEnCours {
    /// Le pas.
    pub(crate) pas: Pas,
    /// L'indice absolu de sa première ligne au journal.
    pub(crate) debut: usize,
    /// Les entrées qui restent à jouer, dans l'ordre.
    entrees: VecDeque<AJouer>,
    /// Ce que les entrées déjà jouées ont produit.
    bilan: Bilan,
}

/// Une avancée commencée et pas finie : les pas à jouer jusqu'à `borne`, puis le
/// saut, puis l'horloge et la tête (voir [`Partie::avancer`]).
#[derive(Debug, Clone, Copy)]
pub(crate) struct AvanceeEnCours {
    /// Le dernier pas à jouer avant le saut.
    pub(crate) borne: Pas,
    /// Le pas que l'horloge a atteint, rattrapage compris.
    jusqua: Pas,
    /// Le temps à jeter après `borne`.
    pub(crate) saut: Duree,
    /// Le numéro de la première arrivée reçue après le début de l'avancée : celles qui
    /// arrivent pendant qu'elle reste en cours ne comptent, pour la tête, qu'à sa fin.
    sequence: u64,
}

impl AvanceeEnCours {
    /// Le pas définitif quand l'avancée sera finie : sa borne, puis le saut. Une
    /// arrivée reçue pendant qu'elle reste en cours se place au-delà
    /// ([`Partie::plancher`]), comme si l'avancée avait fini d'un seul tour : aucune
    /// n'entre dans l'avancée, et la tête en juge à sa fin (CAT-14).
    pub(crate) const fn fin(&self) -> Pas {
        Pas::du_fil(self.borne.numero().saturating_add(self.saut.en_pas()))
    }
}

/// Ce qu'un tour compte dans son budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Compter {
    /// Rien : le tour n'a pas de borne.
    Rien,
    /// Les entrées d'un arriéré seulement : la soirée (P7).
    Arrieres,
    /// Toute entrée venue d'une arrivée : pour éprouver le découpage (E12, tests).
    Arrivees,
}

/// Ce qu'un tour peut encore jouer.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Tour {
    compter: Compter,
    reste: u32,
}

impl Tour {
    /// Un tour sans borne : [`Partie::avancer`], le rejeu, la tête.
    pub(crate) const SANS_BORNE: Tour = Tour {
        compter: Compter::Rien,
        reste: 0,
    };

    /// Un tour qui joue au plus `n` entrées comptées (au moins une, pour avancer).
    pub(crate) fn borne(compter: Compter, n: u32) -> Tour {
        Tour {
            compter,
            reste: n.max(1),
        }
    }

    const fn compte(self, e: &AJouer) -> bool {
        match self.compter {
            Compter::Rien => false,
            Compter::Arrieres => matches!(e.arriere, Oui::Oui),
            Compter::Arrivees => matches!(e.arrivee, Oui::Oui),
        }
    }

    /// Le tour peut-il jouer cette entrée ? Sinon, le pas reste en cours.
    const fn peut_jouer(self, e: &AJouer) -> bool {
        !self.compte(e) || self.reste > 0
    }

    fn jouer(&mut self, e: &AJouer) {
        if self.compte(e) {
            self.reste = self.reste.saturating_sub(1);
        }
    }
}

/// Une arrivée vient-elle d'un arriéré (PHASE-3, § 2.11) ? Un geste ou une entrée de
/// l'écran faits sur une vue de plus de [`ARRIERE_PAS`] pas avant leur réception :
/// ce que le transport livre d'un coup au retour d'un appareil coupé, endormi ou en
/// arrière-plan, ou de l'ordinateur de la soirée gelé.
///
/// Un tenu, une manette, une présence et une réponse n'en sont jamais : un tenu est
/// complet et le dernier gagne, il n'y a pas d'arriéré de tenus (les 71 tenus de
/// Sidequest ne mettent pas la partie en retard) ; une présence et une réponse viennent
/// du commun, à leur réception. Le vu d'un tenu n'est pas celui que l'appareil a dit,
/// mais la dernière vue qu'on lui a envoyée : son âge ne dirait rien.
fn est_un_arriere(a: &ArriveePlacee) -> Oui {
    let vue = match a.quoi {
        QuoiPlace::Geste { .. } | QuoiPlace::Ecran(_) => true,
        QuoiPlace::Tenu { .. }
        | QuoiPlace::Manette { .. }
        | QuoiPlace::Presence(_)
        | QuoiPlace::Reponse { .. } => false,
    };
    if vue && a.recu.pas.numero().saturating_sub(a.vu.numero()) > ARRIERE_PAS {
        Oui::Oui
    } else {
        Oui::Non
    }
}

impl<F: Fabrique> Partie<F> {
    /// Joue le pas `s` sur la définitive, d'un seul tour.
    pub(crate) fn jouer_pas(&mut self, s: Pas) -> Result<(), PanneOutil> {
        self.commencer_pas(s)?;
        let mut tour = Tour::SANS_BORNE;
        let _: Oui = self.continuer_pas(&mut tour)?;
        Ok(())
    }

    /// Commence le pas `s` : ses arrivées, figées maintenant, rangées et traduites en
    /// entrées, après ce que le cœur synthétise. Rien n'est encore joué.
    pub(crate) fn commencer_pas(&mut self, s: Pas) -> Result<(), PanneOutil> {
        let debut = self.fin_confirmee();
        let mut arrivees = self.arrivees_du_pas(s);
        regles::ordonner(&mut arrivees, &self.ouverture.graine);
        let mut entrees = Vec::new();
        self.synthetiser(s, &mut entrees)?;
        for a in arrivees {
            self.traduire_et_marquer(s, a, &mut entrees)?;
        }
        self.en_cours = Some(PasEnCours {
            pas: s,
            debut,
            entrees: entrees.into(),
            bilan: Bilan {
                acceptee: Oui::Non,
                sorties: Vec::new(),
            },
        });
        Ok(())
    }

    /// Traduit une arrivée, et marque ses entrées : venues d'une arrivée, d'un arriéré
    /// ou non.
    fn traduire_et_marquer(
        &mut self,
        s: Pas,
        a: ArriveePlacee,
        entrees: &mut Vec<AJouer>,
    ) -> Result<(), PanneOutil> {
        let arriere = est_un_arriere(&a);
        let avant = entrees.len();
        self.traduire(s, a, entrees)?;
        for e in entrees.iter_mut().skip(avant) {
            e.arrivee = Oui::Oui;
            e.arriere = arriere;
        }
        Ok(())
    }

    /// Joue le pas en cours, tant que le tour le permet, puis le finit : `Temps`, les
    /// sorties, la lecture de l'état, le point de reprise s'il est dû. Rend `Oui` si le
    /// pas est fini, ou abandonné sur la tête qui a fauté ; `Non` s'il reste en cours.
    pub(crate) fn continuer_pas(&mut self, tour: &mut Tour) -> Result<Oui, PanneOutil> {
        let Some(mut pc) = self.en_cours.take() else {
            return Ok(Oui::Oui);
        };
        if self.etale_mal == Oui::Oui && self.curseur.is_none() && self.mode == Mode::Definitive {
            self.entrer_dans_le_pas_en_cours(&mut pc)?;
        }
        let s = pc.pas;
        loop {
            if self.tete_fautive.is_some() {
                // La tête a fauté : elle se reconstruit, ce pas ne s'y finit pas.
                return Ok(Oui::Oui);
            }
            let Some(prochaine) = pc.entrees.front() else {
                break;
            };
            if !tour.peut_jouer(prochaine) {
                self.en_cours = Some(pc);
                return Ok(Oui::Non);
            }
            let Some(e) = pc.entrees.pop_front() else {
                break;
            };
            tour.jouer(&e);
            self.jouer_entree(s, e, &mut pc.bilan)?;
        }
        let mut bilan = pc.bilan;
        let libre = self.coeur.en_pause == Oui::Non && self.coeur.en_panne == Oui::Non;
        if libre && regles::reveil_demande(self.coeur.reveil, s) == Oui::Oui {
            let e = self.temps(s)?;
            self.jouer_entree(s, e, &mut bilan)?;
        }
        if self.tete_fautive.is_some() {
            return Ok(Oui::Oui);
        }
        self.coeur.pas = s;
        self.journal.pas = self.journal.pas.max(s);
        if self.mode == Mode::Tete {
            // Sur la tête, rien ne part : ni sortie, ni demande, ni point (§ 4.4).
            return Ok(Oui::Oui);
        }
        let nouvelle = self.executer_sorties(s, bilan.sorties)?;
        if bilan.acceptee == Oui::Oui || nouvelle == Oui::Oui {
            self.lire_etat()?;
        }
        if nouvelle == Oui::Oui || self.point_du(s) {
            self.prendre_point()?;
        }
        Ok(Oui::Oui)
    }

    /// Le contrôle négatif du découpage (`etale-mal`, PHASE-3 § 8.3) : une arrivée
    /// reçue pendant un pas inachevé y entre, au lieu d'aller au pas suivant. Le
    /// journal change alors avec le découpage, et E12 doit le voir.
    fn entrer_dans_le_pas_en_cours(&mut self, pc: &mut PasEnCours) -> Result<(), PanneOutil> {
        let s = pc.pas;
        let (mut maintenant, plus_tard): (Vec<ArriveePlacee>, Vec<ArriveePlacee>) =
            core::mem::take(&mut self.en_attente)
                .into_iter()
                .partition(|a| a.place <= s);
        self.en_attente = plus_tard;
        if maintenant.is_empty() {
            return Ok(());
        }
        self.journal.lignes.push(Ligne::Arrivees {
            pas: s,
            arrivees: maintenant.clone(),
        });
        regles::ordonner(&mut maintenant, &self.ouverture.graine);
        let mut entrees = Vec::new();
        for a in maintenant {
            self.traduire_et_marquer(s, a, &mut entrees)?;
        }
        pc.entrees.extend(entrees);
        Ok(())
    }

    /// Avance d'un tour (voir [`Partie::avancer`]) : finit d'abord l'avancée qu'un tour
    /// précédent a laissée, puis avance vers `cible`, tant que `tour` le permet.
    pub(crate) fn avancer_tour(
        &mut self,
        cible: Pas,
        mut tour: Tour,
    ) -> Result<Avancee, PanneOutil> {
        let mut saut = Duree::ZERO;
        let mut fini = Oui::Oui;
        if let Some(av) = self.avancee_en_cours.take() {
            let (f, s) = self.poursuivre(av, &mut tour)?;
            fini = f;
            saut = saut.plus(s);
        }
        if fini == Oui::Oui {
            let tete = self.horloge.max(self.coeur.pas);
            let (jusqua, s) =
                regles::cible(self.coeur.pas, tete, cible, self.declaration.rattrapage);
            let borne = if s > Duree::ZERO {
                jusqua
            } else {
                Pas::du_fil(jusqua.numero().saturating_sub(self.fenetre))
            };
            let av = AvanceeEnCours {
                borne,
                jusqua,
                saut: s,
                sequence: self.coeur.sequence,
            };
            let (_, s) = self.poursuivre(av, &mut tour)?;
            saut = saut.plus(s);
        }
        let mut a = core::mem::replace(&mut self.sortie, Avancee::vide(self.coeur.pas));
        a.definitif = self.coeur.pas;
        a.saut = saut;
        a.en_panne = self.coeur.en_panne;
        a.en_cours = self.en_cours.as_ref().map(|p| p.pas);
        Ok(a)
    }

    /// Poursuit une avancée : le pas en cours, puis chaque pas jusqu'à sa borne, puis
    /// le saut, l'horloge et la tête. Rend `Oui` et le temps jeté si elle est finie ;
    /// sinon, elle reste en cours pour le tour suivant.
    fn poursuivre(
        &mut self,
        av: AvanceeEnCours,
        tour: &mut Tour,
    ) -> Result<(Oui, Duree), PanneOutil> {
        if self.continuer_pas(tour)? == Oui::Non {
            self.avancee_en_cours = Some(av);
            return Ok((Oui::Non, Duree::ZERO));
        }
        while self.coeur.pas < av.borne {
            let s = self.suivant();
            self.commencer_pas(s)?;
            if self.continuer_pas(tour)? == Oui::Non {
                self.avancee_en_cours = Some(av);
                return Ok((Oui::Non, Duree::ZERO));
            }
        }
        if av.saut > Duree::ZERO {
            let de = self.coeur.pas;
            let a = Pas::du_fil(de.numero().saturating_add(av.saut.en_pas()));
            self.noter(Ligne::Saut { de, a })?;
            self.appliquer_saut(a);
        }
        self.horloge = av.jusqua.max(self.coeur.pas);
        if self.fenetre > 0 {
            // Les arrivées reçues pendant que l'avancée restait en cours arrivent, pour
            // la tête, après elle : comme si elle avait fini d'un seul tour (CAT-14).
            // Toutes attendent encore : placées au-delà de la fin de l'avancée
            // (`plancher`), aucune n'y est entrée.
            let (tard, avant): (Vec<ArriveePlacee>, Vec<ArriveePlacee>) =
                core::mem::take(&mut self.en_attente)
                    .into_iter()
                    .partition(|a| a.sequence >= av.sequence);
            self.en_attente = avant;
            let rendu = self.avancer_la_tete(self.horloge);
            if let Some(t) = self.tete.as_mut()
                && tard.iter().any(|a| a.place <= t.pas)
            {
                t.a_rejouer = true;
            }
            self.en_attente.extend(tard);
            rendu?;
        }
        Ok((Oui::Oui, av.saut))
    }

    /// Finit le pas et l'avancée qu'un tour borné a laissés, d'un seul tour : avant une
    /// pause, une reprise, ce qui ne se fait qu'entre deux pas.
    pub(crate) fn finir_l_avancee(&mut self) -> Result<(), PanneOutil> {
        if let Some(av) = self.avancee_en_cours.take() {
            let mut tour = Tour::SANS_BORNE;
            let (_, saut) = self.poursuivre(av, &mut tour)?;
            // Le temps jeté se dit au prochain `Avancee`.
            self.sortie.saut = self.sortie.saut.plus(saut);
        }
        Ok(())
    }

    /// Les arrivées placées à ce pas : en direct, celles qui attendent, écrites au
    /// journal ; au rejeu, celles du journal.
    fn arrivees_du_pas(&mut self, s: Pas) -> Vec<ArriveePlacee> {
        if let Some(c) = self.curseur {
            return match self.journal.ligne(c) {
                Some(Ligne::Arrivees { pas, arrivees }) if *pas == s => {
                    let a = arrivees.clone();
                    self.curseur = Some(c.saturating_add(1));
                    a
                }
                Some(_) | None => Vec::new(),
            };
        }
        let (maintenant, plus_tard): (Vec<ArriveePlacee>, Vec<ArriveePlacee>) =
            core::mem::take(&mut self.en_attente)
                .into_iter()
                .partition(|a| a.place <= s);
        self.en_attente = plus_tard;
        if !maintenant.is_empty() {
            self.journal.lignes.push(Ligne::Arrivees {
                pas: s,
                arrivees: maintenant.clone(),
            });
        }
        maintenant
    }

    /// Ce que le cœur synthétise au début du pas : relâchements d'une pause, par
    /// silence, valeurs retenues par le débit.
    fn synthetiser(&mut self, s: Pas, entrees: &mut Vec<AJouer>) -> Result<(), PanneOutil> {
        let pause = self.coeur.relacher_pour_pause == Oui::Oui;
        self.coeur.relacher_pour_pause = Oui::Non;
        let mut telephones = Vec::new();
        if let Some(r) = self.regles_telephone {
            let crate::coeur::EtatCoeur { tenus, fiches, .. } = &mut self.coeur;
            for t in tenus.iter_mut() {
                let nouvelle = derniere_nouvelle(fiches, t.appareil, s);
                let l = if pause {
                    regles::relacher(&mut t.etat, CauseTenu::Pause, s)
                } else {
                    None
                };
                if let Some(l) = l.or_else(|| regles::a_ce_pas(&mut t.etat, &r, s, nouvelle)) {
                    telephones.push((t.appareil, l));
                }
            }
        }
        let mut manettes = Vec::new();
        if let Some(r) = self.regles_manette {
            let crate::coeur::EtatCoeur {
                manettes: m,
                fiches,
                ..
            } = &mut self.coeur;
            for t in m.iter_mut() {
                let nouvelle = derniere_nouvelle(fiches, t.appareil, s);
                let l = if pause {
                    regles::relacher(&mut t.etat, CauseTenu::Pause, s)
                } else {
                    None
                };
                if let Some(l) = l.or_else(|| regles::a_ce_pas(&mut t.etat, &r, s, nouvelle)) {
                    manettes.push((t.appareil, l));
                }
            }
        }
        for (de, l) in telephones {
            let quand = self.quand_synthetise(l.fait, s);
            entrees.push(entree_tenu(de, &l.valeur.octets, l.cause, quand)?);
        }
        for (de, l) in manettes {
            let quand = self.quand_synthetise(l.fait, s);
            entrees.push(entree_manette(de, &l, quand)?);
        }
        Ok(())
    }

    /// Le `Quand` d'une entrée que le cœur synthétise : reçue et vue à ce pas,
    /// avec une identité neuve.
    fn quand_synthetise(&mut self, fait: jeu_abi::Instant, s: Pas) -> QuandFil {
        QuandFil {
            fait: fait.pas.numero(),
            fraction: fait.fraction,
            recu: s.numero(),
            vu: s.numero(),
            id: self.coeur.synthese(),
        }
    }

    /// Les relâchements d'un appareil qui part (§ 4.3, cause `Depart`).
    fn relacher_appareil(
        &mut self,
        de: Appareil,
        s: Pas,
        entrees: &mut Vec<AJouer>,
    ) -> Result<(), PanneOutil> {
        let mut liv = Vec::new();
        for t in self.coeur.tenus.iter_mut().filter(|t| t.appareil == de) {
            if let Some(l) = regles::relacher(&mut t.etat, CauseTenu::Depart, s) {
                liv.push(l);
            }
        }
        let mut liv_m = Vec::new();
        for m in self.coeur.manettes.iter_mut().filter(|m| m.appareil == de) {
            if let Some(l) = regles::relacher(&mut m.etat, CauseTenu::Depart, s) {
                liv_m.push(l);
            }
        }
        for l in liv {
            let quand = self.quand_synthetise(l.fait, s);
            entrees.push(entree_tenu(de, &l.valeur.octets, l.cause, quand)?);
        }
        for l in liv_m {
            let quand = self.quand_synthetise(l.fait, s);
            entrees.push(entree_manette(de, &l, quand)?);
        }
        Ok(())
    }

    /// Ce qu'une arrivée devient au pas `s` : zéro, une ou plusieurs entrées.
    fn traduire(
        &mut self,
        s: Pas,
        a: ArriveePlacee,
        entrees: &mut Vec<AJouer>,
    ) -> Result<(), PanneOutil> {
        self.coeur.sequence = self.coeur.sequence.max(a.sequence.saturating_add(1));
        if let Some(de) = appareil_de(&a.quoi) {
            self.coeur.suivre(de, a.origine, a.fait, a.recu, a.place);
        }
        let mut quand = QuandFil {
            fait: a.fait.pas.numero(),
            fraction: a.fait.fraction,
            recu: a.recu.pas.numero(),
            vu: a.vu.numero(),
            id: a.sequence,
        };
        match a.quoi {
            QuoiPlace::Geste { de, octets, id } => {
                quand.id = id.fil();
                let e = EntreeFil::Geste {
                    de: de.fil(),
                    octets: &octets,
                    quand,
                };
                entrees.push(a_jouer(&e, Some(Destinataire::Appareil(de)))?);
            }
            QuoiPlace::Tenu { de, numero, valeur } => {
                let (Some(r), Some(rang)) = (self.regles_telephone, rang_de(a.origine)) else {
                    return Ok(());
                };
                let cle = valeur.cle.clone();
                let arrivee = ArriveeTenu {
                    valeur,
                    numero,
                    fait: a.fait,
                };
                let issue = self
                    .coeur
                    .avec_tenu(de, rang, &cle, |e| regles::tenu(e, arrivee, &r, s));
                if let IssueTenu::Livree(l) = issue {
                    entrees.push(entree_tenu(de, &l.valeur.octets, l.cause, quand)?);
                }
            }
            QuoiPlace::Manette { de, numero, etat } => {
                let (Some(r), Some(rang)) = (self.regles_manette, rang_de(a.origine)) else {
                    return Ok(());
                };
                let arrivee = ArriveeTenu {
                    valeur: etat,
                    numero,
                    fait: a.fait,
                };
                let issue = self
                    .coeur
                    .avec_manette(de, rang, |e| regles::tenu(e, arrivee, &r, s));
                if let IssueTenu::Livree(l) = issue {
                    entrees.push(entree_manette(de, &l, quand)?);
                }
            }
            QuoiPlace::Ecran(q) => {
                let e = EntreeFil::Ecran {
                    quoi: q.fil(),
                    quand,
                };
                entrees.push(a_jouer(&e, Some(Destinataire::Ecran))?);
            }
            QuoiPlace::Presence(p) => {
                if let jeu_abi::Presence::Depart { appareil, .. } = p {
                    self.relacher_appareil(appareil, s, entrees)?;
                }
                self.coeur.presence(&p);
                entrees.push(a_jouer(&EntreeFil::Presence(p), None)?);
            }
            QuoiPlace::Reponse { id, reponse } => {
                // Une réponse à une demande que le jeu ne liste plus est jetée
                // avant de l'atteindre (TL-45, SAL-164).
                if self.coeur.attend_reponse(id) {
                    self.coeur.repondre(id);
                    let e = EntreeFil::Reponse {
                        id,
                        reponse: reponse.fil(),
                    };
                    entrees.push(a_jouer(&e, None)?);
                }
            }
        }
        Ok(())
    }

    /// Le `Temps` du pas `s`, qui porte le temps jeté depuis le dernier.
    fn temps(&mut self, s: Pas) -> Result<AJouer, PanneOutil> {
        let saut = core::mem::replace(&mut self.coeur.saut, Duree::ZERO);
        let e = EntreeFil::Temps {
            saut: saut.en_pas(),
            heure: heure_du_pas(&self.ouverture, s),
        };
        Ok(AJouer {
            octets: encoder_vers_vec(&e)?,
            pour: None,
            temps: Oui::Oui,
            arrivee: Oui::Non,
            arriere: Oui::Non,
        })
    }

    /// Joue une entrée sur la définitive, l'écrit au journal, et isole une faute
    /// (§ 4.6).
    fn jouer_entree(&mut self, s: Pas, e: AJouer, bilan: &mut Bilan) -> Result<(), PanneOutil> {
        let ordre = self.coeur.ordre;
        self.coeur.ordre = ordre.saturating_add(1);
        if self.mode == Mode::Tete {
            return self.jouer_entree_sur_la_tete(ordre, s, e, bilan);
        }
        let mut a_reconstruire = None;
        let issue = match self.ecartee_en_reference(ordre) {
            Some(faute) => IssueJournal::Ecartee { faute },
            None => match self.appeler_entree(ordre, s, &e.octets) {
                Ok((issue, reveil)) => {
                    if let Some(r) = reveil {
                        self.coeur.reveil = r;
                    }
                    issue
                }
                Err(Echec::Faute(faute)) => {
                    a_reconstruire = Some(self.definitive.carburant());
                    IssueJournal::Ecartee { faute }
                }
                Err(Echec::Panne(p)) => return Err(p),
            },
        };
        let ligne = LigneEntree {
            partie: self.coeur.partie,
            ordre,
            place: s,
            octets: e.octets,
            issue,
        };
        self.noter(Ligne::Entree(ligne.clone()))?;
        self.consequences(&ligne, e.pour, e.temps, bilan);
        if let Some(carburant) = a_reconstruire {
            self.reconstruire()?;
            if let IssueJournal::Ecartee { faute } = ligne.issue {
                let rapport = self.rapport(
                    faute,
                    LieuFaute::Entree {
                        partie: ligne.partie,
                        ordre,
                        place: s,
                        octets: ligne.octets,
                    },
                    carburant,
                );
                self.dire(|a| a.fautes.push(rapport));
            }
        }
        if ordre.wrapping_add(1).is_multiple_of(256) {
            let empreinte = self.definitive.empreinte()?;
            self.noter(Ligne::Empreinte {
                partie: self.coeur.partie,
                ordre,
                empreinte,
            })?;
        }
        Ok(())
    }

    /// Une entrée sur la tête (§ 4.4, § 4.6) : rien n'est écrit au journal, rien n'est
    /// dit ; une faute arrête le rejeu de la tête, qui se reconstruira sans elle.
    fn jouer_entree_sur_la_tete(
        &mut self,
        ordre: u64,
        s: Pas,
        e: AJouer,
        bilan: &mut Bilan,
    ) -> Result<(), PanneOutil> {
        if self.ecartee_de_la_tete(s, &e.octets) {
            return Ok(());
        }
        match self.appeler_entree(ordre, s, &e.octets) {
            Ok((issue, reveil)) => {
                if let Some(r) = reveil {
                    self.coeur.reveil = r;
                }
                let ligne = LigneEntree {
                    partie: self.coeur.partie,
                    ordre,
                    place: s,
                    octets: e.octets,
                    issue,
                };
                self.consequences(&ligne, e.pour, e.temps, bilan);
                Ok(())
            }
            Err(Echec::Faute(_)) => {
                self.tete_fautive = Some((s, e.octets));
                Ok(())
            }
            Err(Echec::Panne(p)) => Err(p),
        }
    }

    /// Ce qu'une issue change au cœur et à ce qu'il dit au commun.
    fn consequences(
        &mut self,
        l: &LigneEntree,
        pour: Option<Destinataire>,
        temps: Oui,
        bilan: &mut Bilan,
    ) {
        let mut temps_ecarte = false;
        match &l.issue {
            IssueJournal::Acceptee { sorties } => {
                bilan.acceptee = Oui::Oui;
                bilan.sorties.extend_from_slice(sorties);
            }
            IssueJournal::Refusee { avis } => {
                let avis = avis.clone();
                match pour {
                    Some(Destinataire::Appareil(a)) => self.dire(|s| s.avis.push((a, avis))),
                    Some(Destinataire::Ecran) => self.dire(|s| s.avis_ecran.push(avis)),
                    // Un refus d'une entrée sans origine vaut `ignorer` (§ 2.7).
                    None => {}
                }
            }
            IssueJournal::Ignoree => {}
            IssueJournal::Ecartee { .. } => {
                temps_ecarte = true;
                if let Some(d) = pour {
                    self.dire(|s| s.ecartes.push(d));
                }
            }
        }
        if temps == Oui::Oui {
            if temps_ecarte {
                self.coeur.temps_ecartes = self.coeur.temps_ecartes.saturating_add(1);
                if self.coeur.temps_ecartes >= 3 {
                    self.coeur.en_panne = Oui::Oui;
                }
            } else {
                self.coeur.temps_ecartes = 0;
                // Un `Temps` accepté après le dernier point à intervalle en fait le
                // point sain (§ 4.6).
                if self.coeur.periodique_confirme == Oui::Non {
                    self.coeur.periodique_confirme = Oui::Oui;
                    self.point_sain = self.periodique.clone();
                }
            }
        }
    }

    /// Au rejeu, la faute de cette entrée si le journal la dit écartée.
    fn ecartee_en_reference(&self, ordre: u64) -> Option<Faute> {
        let c = self.curseur?;
        match self.journal.ligne(c) {
            Some(Ligne::Entree(l)) if l.partie == self.coeur.partie && l.ordre == ordre => {
                match &l.issue {
                    IssueJournal::Ecartee { faute } => Some(faute.clone()),
                    IssueJournal::Acceptee { .. }
                    | IssueJournal::Refusee { .. }
                    | IssueJournal::Ignoree => None,
                }
            }
            Some(_) | None => None,
        }
    }

    /// `Appel::Entree` sur la définitive.
    fn appeler_entree(
        &mut self,
        ordre: u64,
        s: Pas,
        octets: &[u8],
    ) -> Result<(IssueJournal, Option<ReveilFil>), Echec> {
        let appel = Appel::Entree {
            ordre,
            place: s.numero(),
            entree: octets,
        };
        let r = appeler(&mut self.definitive, &appel, self.ouverture.budgets.entree)?;
        issue_du_retour(&r)
    }

    /// Écrit une ligne au journal ; au rejeu, vérifie que c'est celle du journal.
    pub(crate) fn noter(&mut self, l: Ligne) -> Result<(), PanneOutil> {
        let Some(c) = self.curseur else {
            self.journal.lignes.push(l);
            return Ok(());
        };
        match self.journal.ligne(c) {
            Some(r) if *r == l => {
                self.curseur = Some(c.saturating_add(1));
                Ok(())
            }
            Some(r) => Err(divergence(c, &alloc::format!("{r:?}"), &l)),
            None => Err(divergence(c, "la fin du journal", &l)),
        }
    }

    /// La fin du journal déjà confirmée : tout en direct, le curseur au rejeu.
    pub(crate) fn fin_confirmee(&self) -> usize {
        self.curseur.unwrap_or(self.journal.fin())
    }

    /// Dit quelque chose au commun ; au rejeu, c'est déjà dit.
    pub(crate) fn dire(&mut self, f: impl FnOnce(&mut crate::Avancee)) {
        if self.curseur.is_none() && self.mode == Mode::Definitive {
            f(&mut self.sortie);
        }
    }

    pub(crate) fn rapport(
        &self,
        faute: Faute,
        lieu: LieuFaute,
        carburant: Option<u64>,
    ) -> RapportFaute {
        RapportFaute {
            faute,
            lieu,
            carburant,
            graine: self.ouverture.graine,
            point: self.point.indice,
            jusqu_a: self.fin_confirmee(),
        }
    }

    /// Reconstruit la définitive après une faute : restaurer le dernier point, puis
    /// rejouer les entrées du journal qui le suivent, sauf les écartées, en
    /// vérifiant chaque issue et chaque empreinte (§ 4.6).
    pub(crate) fn reconstruire(&mut self) -> Result<(), PanneOutil> {
        self.tete_a_rejouer();
        self.definitive.restaurer(&self.point.instantane)?;
        let partie = self.coeur.partie;
        let budget = self.ouverture.budgets.entree;
        let fin = self.fin_confirmee();
        let lignes = self.journal.entre(self.point.indice, fin);
        for (k, l) in lignes.iter().enumerate() {
            let ou = self.point.indice.saturating_add(k);
            match l {
                Ligne::Entree(e) if e.partie == partie => {
                    if let IssueJournal::Ecartee { .. } = e.issue {
                        continue;
                    }
                    let appel = Appel::Entree {
                        ordre: e.ordre,
                        place: e.place.numero(),
                        entree: &e.octets,
                    };
                    let rejouee = appeler(&mut self.definitive, &appel, budget)
                        .and_then(|r| issue_du_retour(&r))
                        .map(|(i, _)| i);
                    if rejouee.as_ref() != Ok(&e.issue) {
                        return Err(divergence(ou, &alloc::format!("{:?}", e.issue), &rejouee));
                    }
                }
                Ligne::Empreinte {
                    partie: p,
                    empreinte,
                    ..
                } if *p == partie => {
                    let e = self.definitive.empreinte()?;
                    if e != *empreinte {
                        return Err(divergence(ou, "l'empreinte notée", &e));
                    }
                }
                Ligne::Entree(_)
                | Ligne::Empreinte { .. }
                | Ligne::Arrivees { .. }
                | Ligne::Saut { .. }
                | Ligne::Pause { .. }
                | Ligne::Vue { .. }
                | Ligne::Partie { .. } => {}
                // Le point d'une reconstruction suit toujours la dernière reprise.
                Ligne::Reprise { .. } => {
                    return Err(divergence(ou, "aucune reprise après le point", l));
                }
            }
        }
        Ok(())
    }

    /// Les sorties d'un pas définitif (§ 4.4, D10). Rend `Oui` si une nouvelle
    /// partie a commencé.
    fn executer_sorties(&mut self, s: Pas, sorties: Vec<SortieFil>) -> Result<Oui, PanneOutil> {
        let mut nouvelle = None;
        for so in sorties {
            match so {
                SortieFil::Oublier(n) => {
                    let a = Appareil::du_fil(n);
                    self.coeur.oublier(a);
                    self.dire(|x| x.oublier.push(a));
                }
                SortieFil::Retirer(n) => {
                    let a = Appareil::du_fil(n);
                    self.dire(|x| x.retirer.push(a));
                }
                SortieFil::NouvellePartie { graine } => {
                    nouvelle = nouvelle.or(Some(graine));
                }
            }
        }
        match nouvelle {
            Some(g) => self.nouvelle_partie(s, g),
            None => Ok(Oui::Non),
        }
    }

    /// Une nouvelle partie : `Garder` sur l'ancienne instance, une instance neuve,
    /// `Creer` avec la graine de la sortie, la garde et les présents. L'ancienne
    /// mémoire disparaît (CAT-73, CAT-75, PUN-15).
    fn nouvelle_partie(&mut self, s: Pas, graine: [u8; 32]) -> Result<Oui, PanneOutil> {
        let garde = self.garder()?;
        let presents = encoder_vers_vec(&self.coeur.presents())?;
        let mut neuve = self.fabrique.neuve()?;
        match accueillir(&mut neuve, &self.ouverture.budgets) {
            Ok(_) => {}
            Err(RefusOuverture::Panne(p)) => return Err(p),
            Err(RefusOuverture::Faute(f)) => {
                let r = self.rapport(f, LieuFaute::Creation, neuve.carburant());
                self.dire(|a| a.fautes.push(r));
                return Ok(Oui::Non);
            }
            Err(autre) => {
                return Err(PanneOutil::new(
                    CausePanne::Interne,
                    alloc::format!("{autre:?}"),
                ));
            }
        }
        let reveil = match creer(
            &mut neuve,
            &self.ouverture,
            graine,
            s,
            &presents,
            garde.as_deref(),
        ) {
            Ok(r) => r,
            Err(Echec::Faute(f)) => {
                let r = self.rapport(f, LieuFaute::Creation, neuve.carburant());
                self.dire(|a| a.fautes.push(r));
                return Ok(Oui::Non);
            }
            Err(Echec::Panne(p)) => return Err(p),
        };
        let numero = self.coeur.partie.saturating_add(1);
        self.noter(Ligne::Partie { numero, graine })?;
        self.definitive = neuve;
        self.tete_a_rejouer();
        let c = &mut self.coeur;
        let ancienne = c.partie;
        c.partie = numero;
        c.ordre = 0;
        c.reveil = reveil;
        c.temps_ecartes = 0;
        c.en_panne = Oui::Non;
        let annulees: Vec<u64> = c
            .demandes
            .drain(..)
            .filter(|d| d.repondue == Oui::Non)
            .filter_map(|d: DemandeEnCours| crate::coeur::exposer(ancienne, d.id))
            .collect();
        for t in &mut c.tenus {
            t.etat.livre = None;
            t.etat.livre_a = None;
            t.etat.retenue = None;
        }
        for m in &mut c.manettes {
            m.etat.livre = None;
            m.etat.livre_a = None;
            m.etat.retenue = None;
        }
        for id in annulees {
            self.dire(|a| a.annuler.push(jeu_abi::IdDemande::du_fil(id)));
        }
        // Un point tout de suite : une faute avant la fin du pas se reconstruit
        // depuis la nouvelle partie. Le point de la fin du pas le remplace.
        self.prendre_point()?;
        // La nouvelle partie est saine : une panne avant son premier `Temps` accepté
        // repart d'elle, et non d'un point de la partie d'avant, que le cœur ne
        // saurait plus rejoindre (§ 4.6).
        self.point_sain = self.periodique.clone();
        Ok(Oui::Oui)
    }

    /// `Garder` sur la définitive : ce que la nouvelle partie reçoit.
    fn garder(&mut self) -> Result<Option<Vec<u8>>, PanneOutil> {
        let r = match self.lire(&Appel::Garder, "Garder") {
            Ok(r) => r,
            Err(Lu::Faute(_)) => return Ok(None),
            Err(Lu::Panne(p)) => return Err(p),
        };
        match decoder::<Retour<'_>>(&r) {
            Ok(Retour::Octets(o)) => Ok(Some(o.to_vec())),
            Ok(Retour::Illisible(e)) => Err(illisible(e)),
            Ok(_) | Err(_) => {
                self.rapporter_lecture(Faute::SortieInvalide, "Garder");
                Ok(None)
            }
        }
    }

    /// Un point de reprise est-il dû après le pas `s` ?
    fn point_du(&self, s: Pas) -> bool {
        let tous_les = self.ouverture.point_tous_les;
        tous_les > 0 && s.numero().saturating_sub(self.coeur.dernier_point.numero()) >= tous_les
    }

    /// Prend un point de reprise à son heure (création, nouvelle partie, intervalle) :
    /// le prochain sera dû `point_tous_les` pas plus tard.
    pub(crate) fn prendre_point(&mut self) -> Result<(), PanneOutil> {
        self.coeur.dernier_point = self.coeur.pas;
        self.coeur.periodique_confirme = Oui::Non;
        self.instantaner()?;
        let p = Arc::new(crate::partie::PointDeReprise {
            instantane: self.point.instantane.clone(),
            coeur: self.point.coeur.clone(),
            indice: self.point.indice,
            sain: None,
            periodique: None,
        });
        self.periodique = Some(Arc::clone(&p));
        self.point.periodique = Some(p);
        Ok(())
    }

    /// Restaure la définitive au point sain `sain` (§ 4.6), pour
    /// [`Partie::repartir`] en direct et pour le rejeu de [`Ligne::Reprise`]. Rend
    /// les présences à redire au jeu.
    pub(crate) fn appliquer_reprise(
        &mut self,
        sain: &crate::partie::PointDeReprise<<F::I as crate::instance::Instance>::Instantane>,
    ) -> Result<Vec<jeu_abi::Presence>, PanneOutil> {
        self.tete_a_rejouer();
        self.definitive.restaurer(&sain.instantane)?;
        let avant = &sain.coeur;
        let presences = crate::coeur::presences_changees(&avant.fiches, &self.coeur.fiches);
        let ecoule = self.coeur.pas.numero().saturating_sub(avant.pas.numero());
        let partie = self.coeur.partie;
        let annulees: Vec<u64> = self
            .coeur
            .demandes
            .iter()
            .filter(|d| d.repondue == Oui::Non && !avant.demandes.iter().any(|a| a.id == d.id))
            .filter_map(|d| crate::coeur::exposer(partie, d.id))
            .collect();
        let c = &mut self.coeur;
        c.reveil = avant.reveil;
        c.demandes.clone_from(&avant.demandes);
        c.saut = c.saut.plus(Duree::pas(ecoule));
        c.temps_ecartes = 0;
        c.en_panne = Oui::Non;
        for t in &mut c.tenus {
            t.etat.livre = None;
            t.etat.livre_a = None;
            t.etat.retenue = None;
        }
        for m in &mut c.manettes {
            m.etat.livre = None;
            m.etat.livre_a = None;
            m.etat.retenue = None;
        }
        for id in annulees {
            self.dire(|a| a.annuler.push(jeu_abi::IdDemande::du_fil(id)));
        }
        self.lire_etat()?;
        self.prendre_point()?;
        Ok(presences)
    }

    /// Prend un point de reprise : la définitive, l'état du cœur, l'endroit du
    /// journal. Il devient le point d'où une faute se reconstruit.
    pub(crate) fn instantaner(&mut self) -> Result<(), PanneOutil> {
        let instantane = self.definitive.instantane()?;
        self.point = crate::partie::PointDeReprise {
            instantane,
            coeur: self.coeur.clone(),
            indice: self.fin_confirmee(),
            sain: self.point_sain.clone(),
            periodique: self.periodique.clone(),
        };
        Ok(())
    }

    /// Rejoue le cœur sur le journal de référence, du curseur à la fin (§ 4.5).
    pub(crate) fn rejouer(&mut self) -> Result<(), PanneOutil> {
        loop {
            let Some(c) = self.curseur else {
                return Ok(());
            };
            let fin = self.journal.pas;
            let ligne = self.journal.ligne(c);
            if ligne.is_none() && self.coeur.pas >= fin {
                return Ok(());
            }
            match ligne {
                Some(Ligne::Saut { de, a }) if *de == self.coeur.pas => {
                    let a = *a;
                    self.curseur = Some(c.saturating_add(1));
                    self.appliquer_saut(a);
                }
                Some(Ligne::Vue { regard, pas, apres }) if *apres == self.coeur.pas => {
                    let (regard, pas) = (*regard, *pas);
                    self.curseur = Some(c.saturating_add(1));
                    self.coeur.voir(regard, pas);
                }
                Some(Ligne::Reprise { apres, depuis }) if *apres == self.coeur.pas => {
                    let depuis = *depuis;
                    self.curseur = Some(c.saturating_add(1));
                    let sain = self.point_sain.clone().filter(|p| p.indice == depuis);
                    let Some(sain) = sain else {
                        return Err(divergence(
                            c,
                            &alloc::format!("la reprise depuis la ligne {depuis}"),
                            &self.point_sain.as_ref().map(|p| p.indice),
                        ));
                    };
                    let _: Vec<jeu_abi::Presence> = self.appliquer_reprise(&sain)?;
                }
                Some(Ligne::Pause { apres, en_pause }) if *apres == self.coeur.pas => {
                    let en_pause = *en_pause;
                    self.curseur = Some(c.saturating_add(1));
                    self.appliquer_pause(en_pause);
                }
                Some(l) if self.coeur.pas >= fin => {
                    return Err(divergence(
                        c,
                        &alloc::format!("{l:?}"),
                        &"aucun pas à jouer",
                    ));
                }
                Some(_) | None => {
                    let s = self.suivant();
                    self.jouer_pas(s)?;
                }
            }
        }
    }
}

/// Le dernier signe de vie d'un appareil, ou ce pas s'il est inconnu.
fn derniere_nouvelle(fiches: &[crate::coeur::Fiche], a: Appareil, s: Pas) -> Pas {
    fiches
        .iter()
        .find(|f| f.appareil == a)
        .map_or(s, |f| f.derniere_nouvelle)
}

fn rang_de(o: Origine) -> Option<u32> {
    match o {
        Origine::Appareil(r) => Some(r),
        Origine::Commun | Origine::Ecran => None,
    }
}

fn appareil_de(q: &QuoiPlace) -> Option<Appareil> {
    match q {
        QuoiPlace::Geste { de, .. }
        | QuoiPlace::Tenu { de, .. }
        | QuoiPlace::Manette { de, .. } => Some(*de),
        QuoiPlace::Presence(p) => Some(crate::partie::appareil_de(p)),
        QuoiPlace::Ecran(_) | QuoiPlace::Reponse { .. } => None,
    }
}

fn a_jouer(e: &EntreeFil<'_>, pour: Option<Destinataire>) -> Result<AJouer, PanneOutil> {
    Ok(AJouer {
        octets: encoder_vers_vec(e)?,
        pour,
        temps: Oui::Non,
        arrivee: Oui::Non,
        arriere: Oui::Non,
    })
}

fn entree_tenu(
    de: Appareil,
    octets: &[u8],
    cause: CauseTenu,
    quand: QuandFil,
) -> Result<AJouer, PanneOutil> {
    let e = EntreeFil::Tenu {
        de: de.fil(),
        octets,
        cause,
        quand,
    };
    a_jouer(&e, Some(Destinataire::Appareil(de)))
}

fn entree_manette(
    de: Appareil,
    l: &Livraison<jeu_abi::Manette>,
    quand: QuandFil,
) -> Result<AJouer, PanneOutil> {
    let e = EntreeFil::Manette {
        de: de.fil(),
        etat: l.valeur,
        cause: l.cause,
        quand,
    };
    a_jouer(&e, Some(Destinataire::Appareil(de)))
}

/// L'issue que rend un `Appel::Entree`, et le réveil d'une acceptation.
fn issue_du_retour(r: &[u8]) -> Result<(IssueJournal, Option<ReveilFil>), Echec> {
    match decoder::<Retour<'_>>(r) {
        Ok(Retour::Issue(IssueFil::Acceptee { sorties, reveil })) => {
            Ok((IssueJournal::Acceptee { sorties }, Some(reveil)))
        }
        Ok(Retour::Issue(IssueFil::Refusee { avis })) => Ok((
            IssueJournal::Refusee {
                avis: avis.to_vec(),
            },
            None,
        )),
        Ok(Retour::Issue(IssueFil::Ignoree)) => Ok((IssueJournal::Ignoree, None)),
        // Sur la partie qui fait foi, aucun secret ne manque (§ 2.7).
        Ok(Retour::Issue(IssueFil::Attente)) => Err(Echec::Faute(Faute::Attente)),
        Ok(Retour::Illisible(e)) => Err(Echec::Panne(illisible(e))),
        Ok(_) | Err(_) => Err(Echec::Faute(Faute::SortieInvalide)),
    }
}

/// La panne d'un rejeu qui ne retrouve pas le journal.
fn divergence(ligne: usize, attendu: &str, obtenu: &impl core::fmt::Debug) -> PanneOutil {
    PanneOutil::new(
        CausePanne::RejeuDivergent,
        alloc::format!("ligne {ligne} du journal : attendu {attendu}, obtenu {obtenu:?}"),
    )
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use jeu_abi::{Appareil, Duree, IdGeste, Instant, Pas, TexteCourt, encoder};

    use crate::journal::Ligne;
    use crate::modele::{FabriqueModele, GesteModele, ReglageModele};
    use crate::{Arrivee, Budgets, Ouverture, Partie, Placement};

    fn ouverture() -> Ouverture {
        Ouverture {
            graine: [3; 32],
            pas: 0,
            heure: 1_000_000,
            salle: TexteCourt::new("SALLE"),
            budgets: Budgets {
                entree: 1_000,
                lecture: 1_000,
                creation: 1_000,
                canonique: 1_000,
            },
            point_tous_les: 600,
        }
    }

    fn dormant() -> ReglageModele {
        let mut r = ReglageModele::ordinaire();
        r.reveil = jeu_abi::ReveilFil::Jamais;
        r
    }

    fn geste(g: GesteModele, id: u64) -> Arrivee {
        let mut octets = Vec::new();
        encoder(&g, &mut octets).unwrap();
        Arrivee::Geste {
            de: Appareil::du_fil(1),
            octets,
            fait: Instant::de(Pas::du_fil(1)),
            vu: Pas::du_fil(1),
            id: IdGeste::du_fil(id),
            demi_aller_retour: Duree::ZERO,
        }
    }

    /// RESTE-PHASE-1 (point 25, cargo-mutants) : au rejeu, une entrée n'est tenue pour
    /// écartée que si sa propre ligne du journal le dit. Dans un journal mal aligné,
    /// la ligne écartée d'une autre entrée ne la dispense pas d'être jouée : elle est
    /// jouée, puis le rejeu diverge. Un journal mal aligné ne se fabrique pas par
    /// l'API publique : ce test retire une ligne à la main.
    #[test]
    fn une_ligne_ecartee_ne_vaut_que_pour_son_entree() {
        let mut p = Partie::ouvrir(FabriqueModele::new(dormant()), ouverture()).unwrap();
        let depart = p.point().unwrap();
        for (k, g) in [GesteModele::Ajouter(1), GesteModele::Paniquer]
            .into_iter()
            .enumerate()
        {
            let id = u64::try_from(k).unwrap();
            let _: Placement = p
                .arriver(geste(g, id), Instant::de(Pas::du_fil(1)))
                .unwrap();
        }
        let _: crate::Avancee = p.avancer(Pas::du_fil(1)).unwrap();
        let mut journal = p.journal().clone();
        let ordres: Vec<(u64, bool)> = journal
            .entrees()
            .map(|e| {
                (
                    e.ordre,
                    matches!(e.issue, crate::IssueJournal::Ecartee { .. }),
                )
            })
            .collect();
        assert_eq!(ordres, vec![(0, false), (1, true)]);
        // Sans la ligne de l'entrée d'ordre 0, la suivante, écartée, est sous le
        // curseur quand l'entrée d'ordre 0 se rejoue.
        journal
            .lignes
            .retain(|l| !matches!(l, Ligne::Entree(e) if e.ordre == 0));
        let f = FabriqueModele::new(dormant());
        let compteurs = alloc::rc::Rc::clone(&f.compteurs);
        let r = Partie::reprendre(f, ouverture(), &depart, &journal);
        assert!(r.is_err(), "un journal mal aligné diverge");
        assert_eq!(
            compteurs.ecritures.get(),
            1,
            "l'entrée d'ordre 0 est jouée avant que le rejeu ne diverge"
        );
    }

    /// RESTE-PHASE-1 (point 25, cargo-mutants) : un journal qui a encore des lignes
    /// au-delà de son dernier pas définitif diverge : le rejeu ne joue pas de pas que la
    /// partie n'a pas joués. Ce journal ne se fabrique pas par l'API publique : ce test
    /// recule son dernier pas à la main.
    #[test]
    fn des_lignes_apres_le_dernier_pas_divergent() {
        let mut p = Partie::ouvrir(FabriqueModele::new(dormant()), ouverture()).unwrap();
        let depart = p.point().unwrap();
        for s in 1..=6u64 {
            let _: Placement = p
                .arriver(
                    geste(GesteModele::Ajouter(1), s),
                    Instant::de(Pas::du_fil(s)),
                )
                .unwrap();
            let _: crate::Avancee = p.avancer(Pas::du_fil(s)).unwrap();
        }
        let mut journal = p.journal().clone();
        journal.pas = Pas::du_fil(3);
        let r = Partie::reprendre(
            FabriqueModele::new(dormant()),
            ouverture(),
            &depart,
            &journal,
        );
        match r {
            Err(crate::RefusReprise::Panne(panne)) => {
                assert_eq!(panne.cause, crate::CausePanne::RejeuDivergent);
            }
            Err(autre) => panic!("attendu une divergence, obtenu {autre:?}"),
            Ok(_) => panic!("un journal aux lignes en trop se rejoue sans diverger"),
        }
    }
}
