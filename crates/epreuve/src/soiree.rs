//! La soirée simulée (§ 7.1) : la `Partie` du cœur sur une horloge simulée, des
//! téléphones et un organisateur simulés dont les bots du jeu tiennent la main, le
//! transport de `telephone.rs`, les demandes exécutées, et les pannes du plan,
//! programmées puis constatées.
//!
//! La soirée joue un pas de l'horloge à la fois : les pannes du pas, les arrivées
//! (présences, paquets, réponses), l'organisateur, `avancer`, les vues et les
//! constats, puis les téléphones. Tout ce qu'elle demande au cœur s'écrit dans un
//! script ([`Op`]) : rejoué sur une autre instance, il doit redonner la même partie
//! (E9, E14).
//!
//! Elle joue le module, pas la copie native : c'est le module qui passe l'épreuve et
//! que l'on joue (CONTRAT, « Le cadre technique »).

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use jeu_abi::{
    Appareil, Appel, ApplicableFil, Capacites, CauseDepart, ComposantBotFil, CoupFil, DemandeFil,
    Duree, EchecDemande, EntreeFil, IdDemande, Instant, IssueFil, Oui, Pas, Presence, QuandFil,
    RegardFil, Retour, RoleAppareil, SorteAppel, SorteOctets, StyleFil, TexteCourt, decoder,
    encoder,
};
use moteur_wasmtime::{FabriqueWasm, InstanceWasm, InstantaneWasm, ModuleCharge};
use partie::{
    Arrivee, Avancee, Budgets, ComposantArrivee, DemandeArrivee, Echec, EcranArrivee, Fabrique,
    Faute, Instance, Journal, Ligne, Ouverture, Partie, PointDeReprise, RefusReprise,
    ReponseArrivee,
};

use crate::controles;
use crate::plan::{IdControle, Panne};
use crate::sonde::{Compte, Obs, Sonde, Verifs, hacher, premiere_entree_differente};
use crate::telephone::{
    EtatEvenement, FERMETURE, Lien, Paquet, SILENCE_MAX, Telephone, VueEnRoute,
};
use crate::tirage::Tirage;

/// Le carburant de chaque sorte d'appel, dans l'épreuve. Des points de départ, non
/// mesurés : larges, pour qu'un jeu honnête n'y bute jamais, et qu'une boucle
/// infinie s'arrête en moins d'une seconde sur une machine de développement.
pub const BUDGETS: Budgets = Budgets {
    entree: 1 << 28,
    lecture: 1 << 28,
    creation: 1 << 28,
    canonique: 1 << 28,
};

/// Les pas entre deux points de reprise, dans l'épreuve (§ 4.5 : 10 s de partie).
pub const POINT_TOUS_LES: u64 = 600;

/// Les pas entre deux consultations du bot d'un téléphone sage : 250 ms, un temps de
/// réaction humain.
const PERIODE_SAGE: u64 = 15;
/// … d'un téléphone lent.
const PERIODE_LENTE: u64 = 60;
/// … d'un téléphone hostile.
const PERIODE_HOSTILE: u64 = 8;
/// … d'un téléphone de la soirée longue.
const PERIODE_LONGUE: u64 = 120;
/// … de l'organisateur.
const PERIODE_ECRAN: u64 = 15;
/// Les pas entre deux constats des secrets (E8) dans une même phase.
const PERIODE_SECRETS: u64 = 30;
/// Les pas entre deux mesures de la mémoire (E15) : cinq minutes.
pub(crate) const PERIODE_MEMOIRE: u64 = 5 * 60 * 60;
/// Les échantillons d'octets de gestes gardés pour les entrées absurdes.
const ECHANTILLONS_MAX: usize = 64;

/// L'ouverture d'une soirée de cette graine.
pub(crate) fn ouverture(graine: [u8; 32]) -> Ouverture {
    Ouverture {
        graine,
        pas: 0,
        heure: 1_700_000_000_000,
        salle: TexteCourt::new("EPREUVE"),
        budgets: BUDGETS,
        point_tous_les: POINT_TOUS_LES,
    }
}

/// Une opération de la soirée sur la partie : le script qui la rejoue.
#[derive(Debug, Clone)]
pub(crate) enum Op {
    Arriver(Arrivee, Instant),
    Avancer(Pas),
    Regarder(Vec<RegardFil>),
    Etat,
    Canonique,
    /// Un point de reprise pris par la soirée.
    Point,
    /// L'autorité redémarre depuis le dernier point ; `lignes` est la longueur du
    /// journal à cet instant.
    Redemarrer {
        lignes: usize,
    },
}

/// La sorte d'une soirée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sorte {
    /// Une soirée principale, avec ses pannes et ses constats (E2 à E8, E14).
    Principale,
    /// Des téléphones hostiles et des arrivées absurdes (E13).
    Hostile,
    /// Six heures, des appareils qui vont et viennent (E15).
    Longue,
    /// Sans panne ni constat : la soirée dont le script se rejoue renuméroté, coupé ou
    /// retardé (E10, E11, E12).
    Proprietes,
    /// Des gels pendant une échéance, et la phase après chaque entrée (E18).
    Gel,
}

/// Ce que la soirée vérifie.
#[derive(Debug, Clone)]
pub(crate) struct Reglage {
    pub(crate) sorte: Sorte,
    pub(crate) graine: [u8; 32],
    pub(crate) duree: u64,
    pub(crate) pannes: Vec<Panne>,
    pub(crate) verifs: Verifs,
    pub(crate) e5: Oui,
    pub(crate) e8: Oui,
    pub(crate) e14: Oui,
}

/// Où en est une panne programmée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Avancement {
    Attend,
    EnCours {
        fin: u64,
    },
    Constatee,
    Manquee,
    /// Une panne de téléphone, pour un jeu qui n'en a pas : jamais programmée.
    SansObjet,
}

#[derive(Debug, Clone)]
struct Programme {
    /// Vient-elle du plan, ou de la soirée elle-même ?
    du_plan: Oui,
    panne: Panne,
    debut: u64,
    cible: usize,
    etat: Avancement,
    note: String,
}

/// Une mesure de la mémoire (E15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Echantillon {
    pub(crate) pas: u64,
    pub(crate) vivants: u64,
    pub(crate) retenus: u64,
    pub(crate) presents: u64,
}

/// Ce que la soirée a vu de la partie (E2, E3).
#[derive(Debug, Clone, Default)]
pub(crate) struct Faits {
    pub(crate) phases: BTreeSet<u16>,
    pub(crate) cas: BTreeSet<u16>,
    /// Les parties où une phase terminale a été vue.
    pub(crate) terminales: BTreeSet<u32>,
    pub(crate) parties: u32,
    /// E8 : les valeurs secrètes trop courtes pour être cherchées dans les vues.
    pub(crate) secrets_non_cherches: u64,
}

/// Une faute du jeu, rattachée au contrôle qui l'a vue.
#[derive(Debug, Clone)]
pub(crate) struct FauteVue {
    pub(crate) controle: IdControle,
    pub(crate) detail: String,
}

/// Ce qu'une soirée laisse aux contrôles.
pub(crate) struct Bilan {
    pub(crate) ouverture: Ouverture,
    pub(crate) journal: Journal,
    pub(crate) script: Vec<Op>,
    pub(crate) fautes: Vec<FauteVue>,
    pub(crate) pannes_outil: Vec<String>,
    pub(crate) faits: Faits,
    pub(crate) obs: Rc<RefCell<Obs>>,
    pub(crate) e5: Compte,
    pub(crate) e8: Compte,
    pub(crate) e13: Compte,
    pub(crate) e14: Compte,
    /// Les pannes du plan : constatée ou non, et ce qu'on en sait.
    pub(crate) pannes: Vec<(Panne, Oui, String)>,
    /// Les pannes du plan sans objet pour ce jeu : celles d'un téléphone, quand il
    /// n'en a pas (besoin 2 des jeux).
    pub(crate) pannes_sans_objet: Vec<Panne>,
    pub(crate) memoire: Vec<Echantillon>,
    pub(crate) regards: Vec<RegardFil>,
    pub(crate) echantillons: Vec<(SorteOctets, Vec<u8>)>,
    pub(crate) abandon: Option<String>,
    pub(crate) pas_joues: u64,
    pub(crate) arrivees: u64,
    /// E17 : flottants non finis de la forme canonique, longueurs saturées des scènes.
    pub(crate) e17: Compte,
    /// Les gels de la soirée : leur premier pas, et le pas où le temps reprend.
    pub(crate) gels: Vec<(u64, u64)>,
}

/// L'organisateur simulé, à l'écran.
#[derive(Debug, Clone, Default)]
struct Ecran {
    vu: u64,
    vue: Vec<u8>,
    depuis: u64,
    prochain: u64,
}

/// Une présence que le commun annoncera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Annonce {
    Depart(CauseDepart),
    Retour,
}

/// Le type de la partie de l'épreuve.
type PartieEpreuve = Partie<Sonde<FabriqueWasm>>;
/// Un point de reprise du module.
pub type Point = PointDeReprise<InstantaneWasm>;

/// Une soirée en cours.
pub(crate) struct Soiree<'m> {
    module: &'m ModuleCharge,
    reglage: Reglage,
    obs: Rc<RefCell<Obs>>,
    partie: PartieEpreuve,
    ouverture: Ouverture,
    t: Tirage,
    /// Le hasard qui choisit les points gardés pour E14, à part : une soirée sans E14
    /// tire le même reste.
    tp: Tirage,
    absurde: Tirage,
    pas: u64,
    tels: Vec<Telephone>,
    ecran: Ecran,
    annonces: Vec<(u64, usize, Annonce)>,
    demandes: Vec<(IdDemande, DemandeArrivee, u64)>,
    programme: Vec<Programme>,
    gel: Option<u64>,
    copie: Option<InstanceWasm>,
    projection: Option<InstanceWasm>,
    dernier_point: Option<Point>,
    points: Vec<Point>,
    script: Vec<Op>,
    fautes: Vec<FauteVue>,
    pannes_outil: Vec<String>,
    faits: Faits,
    e5: Compte,
    e8: Compte,
    e13: Compte,
    e14: Compte,
    memoire: Vec<Echantillon>,
    lignes_vues: usize,
    acceptees_vues: u64,
    partie_vue: u32,
    forcer_vues: bool,
    dernier_e8: u64,
    derniere_phase: Option<u16>,
    appareils: Vec<u64>,
    echantillons: Vec<(SorteOctets, Vec<u8>)>,
    vus_pour_echantillon: u64,
    stop: Option<String>,
    arrivees: u64,
    en_panne_vue: bool,
    e17: Compte,
    dernier_e17: u64,
    /// La dernière échéance que la partie a dite attendre (E18).
    echeance: Option<u64>,
    gels: Vec<(u64, u64)>,
    prochain_gel: u64,
}

/// Les gels à l'échéance d'une soirée de E18.
const GELS_E18: usize = 4;
/// Ce qu'un gel de E18 dure au-delà de l'échéance : trois secondes.
const GEL_APRES_ECHEANCE: u64 = 180;
/// Les pas, au moins, entre deux gels de E18 : vingt secondes.
const ENTRE_DEUX_GELS: u64 = 1200;

fn octets<T: serde::Serialize>(v: &T) -> Vec<u8> {
    let mut o = Vec::new();
    match encoder(v, &mut o) {
        Ok(()) => o,
        Err(_) => Vec::new(),
    }
}

/// Le contrôle qui porte une faute du jeu vue dans une soirée de cette sorte.
const fn controle_des_fautes(s: Sorte) -> IdControle {
    match s {
        Sorte::Principale => IdControle::E2,
        Sorte::Hostile => IdControle::E13,
        Sorte::Longue => IdControle::E15,
        Sorte::Proprietes => IdControle::E10,
        Sorte::Gel => IdControle::E18,
    }
}

/// Ce qu'une échéance peut être passée, en pas, et compter encore pour E5 : le
/// `Temps` qui la suit arrive au pas d'après. Point de départ, non mesuré.
pub(crate) const ECHEANCE_PASSEE_MAX: u64 = 2;

/// Une échéance au pas `e` peut-elle encore faire avancer la partie, au pas définitif
/// `definitif`, sous ce réveil ? (E5, CAT-78.) Après un saut, `e` est le pas où le
/// numéro reprend, si l'échéance est passée pendant le gel.
pub(crate) fn echeance_vivante(e: u64, definitif: u64, reveil: jeu_abi::ReveilFil) -> bool {
    let reveillee = match reveil {
        // `A` réveille à son pas, ou au premier pas après s'il est passé.
        jeu_abi::ReveilFil::ChaquePas | jeu_abi::ReveilFil::A(_) => true,
        jeu_abi::ReveilFil::Jamais => false,
    };
    reveillee && e.saturating_add(ECHEANCE_PASSEE_MAX) >= definitif
}

/// Le numéro de la partie en cours au point `indice` du journal.
pub(crate) fn partie_au(journal: &Journal, indice: usize) -> u32 {
    journal
        .lignes()
        .iter()
        .take(indice)
        .filter_map(|l| match l {
            Ligne::Partie { numero, .. } => Some(*numero),
            Ligne::Arrivees { .. }
            | Ligne::Entree(_)
            | Ligne::Empreinte { .. }
            | Ligne::Saut { .. }
            | Ligne::Pause { .. }
            | Ligne::Vue { .. }
            | Ligne::Reprise { .. } => None,
        })
        .next_back()
        .unwrap_or(0)
}

impl<'m> Soiree<'m> {
    /// Ouvre une soirée. `Err` : la partie ne s'ouvre pas.
    pub(crate) fn ouvrir(module: &'m ModuleCharge, reglage: Reglage) -> Result<Soiree<'m>, String> {
        let obs = Obs::new(reglage.verifs, BUDGETS.lecture);
        let ouverture = ouverture(reglage.graine);
        let partie = Partie::ouvrir(Sonde::new(module.fabrique(), &obs), ouverture.clone())
            .map_err(|e| format!("la partie ne s'ouvre pas : {e:?}"))?;
        let mut t = Tirage::new(&reglage.graine, "soirée");
        let absurde = t.deriver("absurde");
        let tp = t.deriver("points");
        let declaration = module.declaration();
        let mut tels = Vec::new();
        if let ApplicableFil::Oui(_) = declaration.telephones {
            let joueurs = usize::from(declaration.places.joueurs_max()).clamp(1, 8);
            let n = match reglage.sorte {
                Sorte::Principale | Sorte::Hostile | Sorte::Proprietes | Sorte::Gel => {
                    joueurs.saturating_add(1)
                }
                Sorte::Longue => joueurs,
            };
            for i in 0..n {
                let k = u64::try_from(i).unwrap_or(0);
                let style = match reglage.sorte {
                    // Un téléphone sur deux hostile : les autres rejoignent la partie, et
                    // leurs gestes donnent aux entrées absurdes des joueurs à imiter.
                    Sorte::Hostile if i.checked_rem(2) == Some(0) => StyleFil::Hostile,
                    Sorte::Principale if i.saturating_add(1) == n => StyleFil::Lent,
                    Sorte::Hostile
                    | Sorte::Principale
                    | Sorte::Longue
                    | Sorte::Proprietes
                    | Sorte::Gel => StyleFil::Sage,
                };
                let latence = t.entre(1, 4);
                let id = t.entier() | 1;
                tels.push(Telephone::new(
                    id,
                    style,
                    30_u64.saturating_add(k.saturating_mul(18)),
                    latence,
                ));
            }
        }
        let appareils = tels.iter().map(|t| t.appareil).collect();
        let mut s = Soiree {
            module,
            reglage,
            obs,
            partie,
            ouverture,
            t,
            tp,
            absurde,
            pas: 0,
            tels,
            ecran: Ecran::default(),
            annonces: Vec::new(),
            demandes: Vec::new(),
            programme: Vec::new(),
            gel: None,
            copie: None,
            projection: None,
            dernier_point: None,
            points: Vec::new(),
            script: Vec::new(),
            fautes: Vec::new(),
            pannes_outil: Vec::new(),
            faits: Faits::default(),
            e5: Compte::default(),
            e8: Compte::default(),
            e13: Compte::default(),
            e14: Compte::default(),
            memoire: Vec::new(),
            lignes_vues: usize::MAX,
            acceptees_vues: u64::MAX,
            partie_vue: 0,
            forcer_vues: true,
            dernier_e8: 0,
            derniere_phase: None,
            appareils,
            echantillons: Vec::new(),
            vus_pour_echantillon: 0,
            stop: None,
            arrivees: 0,
            en_panne_vue: false,
            e17: Compte::default(),
            dernier_e17: 0,
            echeance: None,
            gels: Vec::new(),
            prochain_gel: 0,
        };
        s.programmer();
        Ok(s)
    }

    fn panne_outil(&mut self, quoi: impl Into<String>) {
        let q = quoi.into();
        if self.stop.is_none() {
            self.stop = Some(q.clone());
        }
        self.pannes_outil.push(q);
    }

    fn recu(&mut self) -> Instant {
        Instant {
            pas: Pas::du_fil(self.pas),
            fraction: 0,
        }
    }

    /// Une arrivée au cœur, notée au script.
    fn arriver(&mut self, a: Arrivee) -> Option<partie::Placement> {
        let recu = self.recu();
        self.arrivees = self.arrivees.saturating_add(1);
        self.script.push(Op::Arriver(a.clone(), recu));
        match self.partie.arriver(a, recu) {
            Ok(p) => Some(p),
            Err(p) => {
                self.panne_outil(format!("arriver : {p}"));
                None
            }
        }
    }

    /// Programme les pannes du plan, et les allées et venues de la soirée longue.
    fn programmer(&mut self) {
        let duree = self.reglage.duree;
        let n = self.tels.len();
        let mut k = 0usize;
        for panne in self.reglage.pannes.clone() {
            let debut = self.t.entre(
                duree.saturating_mul(10) / 100,
                duree.saturating_mul(38) / 100,
            );
            let cible = match panne {
                Panne::Coupure { .. }
                | Panne::Silence { .. }
                | Panne::TrouNoir { .. }
                | Panne::Retenu { .. }
                | Panne::Rechargement
                | Panne::Jumeau => {
                    let c = k.checked_rem(n.max(1)).unwrap_or(0);
                    k = k.saturating_add(1);
                    c
                }
                Panne::Rafales | Panne::RedemarrageAutorite | Panne::Gel { .. } => 0,
            };
            // Les pannes d'un téléphone dans la première moitié de la soirée, le
            // redémarrage ensuite, les gels à la fin : elles se mêlent sans qu'un long
            // gel n'avale les autres, qu'il faut constater (OUT-26, OUT-77).
            let entre = |t: &mut Tirage, de: u64, a: u64| {
                t.entre(
                    duree.saturating_mul(de) / 100,
                    duree.saturating_mul(a) / 100,
                )
            };
            let debut = match panne {
                Panne::Rafales => 1,
                Panne::Coupure { .. }
                | Panne::Silence { .. }
                | Panne::TrouNoir { .. }
                | Panne::Retenu { .. }
                | Panne::Rechargement
                | Panne::Jumeau => debut,
                Panne::RedemarrageAutorite => entre(&mut self.t, 40, 48),
                Panne::Gel { duree: d } if d < Duree::s(10) => entre(&mut self.t, 50, 55),
                Panne::Gel { .. } => entre(&mut self.t, 60, 65),
            };
            // Un jeu sans téléphone ne vit pas une panne de téléphone : elle est sans
            // objet, ni programmée ni manquée (besoin 2 des jeux).
            let etat = if n == 0 && panne.du_telephone() == Oui::Oui {
                Avancement::SansObjet
            } else {
                Avancement::Attend
            };
            self.programme.push(Programme {
                du_plan: Oui::Oui,
                panne,
                debut,
                cible,
                etat,
                note: String::new(),
            });
        }
        if self.reglage.sorte == Sorte::Longue {
            // Des départs pour de bon, chacun remplacé par un appareil neuf, toutes les
            // quatre à huit minutes ; des coupures courtes ; un gel de dix minutes sur
            // une soirée de deux heures au moins (§ 7.1, OUT-84, SQ-76, PUN-15).
            let mut p = 0u64;
            loop {
                p = p.saturating_add(self.t.entre(4 * 3600, 8 * 3600));
                if p >= duree {
                    break;
                }
                self.programme.push(Programme {
                    du_plan: Oui::Non,
                    panne: Panne::Coupure {
                        duree: Duree::s(self.t.entre(2, 30)),
                    },
                    debut: p,
                    cible: usize::try_from(self.t.sous(u64::try_from(n).unwrap_or(1))).unwrap_or(0),
                    etat: Avancement::Attend,
                    note: String::from("allée et venue"),
                });
            }
            if duree >= 2 * 3600 * 60 {
                self.programme.push(Programme {
                    du_plan: Oui::Non,
                    panne: Panne::Gel {
                        duree: Duree::s(600),
                    },
                    debut: duree / 2,
                    cible: 0,
                    etat: Avancement::Attend,
                    note: String::from("gel de dix minutes"),
                });
            }
        }
    }

    /// Joue la soirée jusqu'au bout, et rend son bilan.
    pub(crate) fn jouer(mut self) -> Bilan {
        let _: Option<()> = self.regarder();
        if self.reglage.e14 == Oui::Oui {
            self.prendre_point(Oui::Oui);
        }
        let mut prochain_depart = self.prochain_depart_longue();
        for pas in 1..=self.reglage.duree {
            if self.stop.is_some() {
                break;
            }
            self.pas = pas;
            self.arrivees_et_departs();
            if self.reglage.sorte == Sorte::Longue && pas >= prochain_depart {
                self.renouveler();
                prochain_depart = pas.saturating_add(self.prochain_depart_longue());
            }
            self.pannes_du_pas();
            if self.stop.is_some() {
                break;
            }
            if let Some(fin) = self.gel {
                if pas < fin {
                    self.bots_telephones();
                    continue;
                }
                self.gel = None;
            }
            self.livrer();
            self.bot_ecran();
            if self.reglage.sorte == Sorte::Hostile {
                self.absurdes();
            }
            self.avancer();
            self.apres_le_pas();
            if self.reglage.sorte == Sorte::Gel {
                self.geler_a_l_echeance();
            }
            self.vues_des_telephones();
            self.bots_telephones();
            if pas.checked_rem(POINT_TOUS_LES) == Some(0) {
                self.prendre_point(Oui::Non);
            }
            if self.reglage.sorte == Sorte::Longue
                && pas.checked_rem(PERIODE_MEMOIRE) == Some(PERIODE_MEMOIRE / 2)
            {
                self.mesurer_la_memoire();
            }
        }
        self.finir()
    }

    /// E18 : un gel pendant une échéance attendue, qui dure jusqu'au-delà d'elle : au
    /// retour, la phase qui finit à l'échéance finit, et la suivante doit recevoir
    /// toute sa durée (PUN-34, D15).
    fn geler_a_l_echeance(&mut self) {
        let pas = self.pas;
        if self.gel.is_some() || self.gels.len() >= GELS_E18 || pas < self.prochain_gel {
            return;
        }
        let Some(e) = self.echeance else {
            return;
        };
        if e <= pas.saturating_add(30) || e >= self.reglage.duree.saturating_sub(1200) {
            return;
        }
        let fin = e.saturating_add(GEL_APRES_ECHEANCE);
        self.gel = Some(fin);
        self.gels.push((pas, fin));
        self.prochain_gel = fin.saturating_add(ENTRE_DEUX_GELS);
    }

    fn prochain_depart_longue(&mut self) -> u64 {
        match self.reglage.sorte {
            Sorte::Longue => self.t.entre(4 * 3600, 8 * 3600),
            Sorte::Principale | Sorte::Hostile | Sorte::Proprietes | Sorte::Gel => u64::MAX,
        }
    }

    /// Un appareil présent part pour de bon ; un neuf arrive trente secondes plus tard.
    fn renouveler(&mut self) {
        let presents: Vec<usize> = self
            .tels
            .iter()
            .enumerate()
            .filter(|(_, t)| t.lien == Lien::Ok && t.part.is_none())
            .map(|(i, _)| i)
            .collect();
        let n = u64::try_from(presents.len()).unwrap_or(0);
        if n < 2 {
            return;
        }
        let k = usize::try_from(self.t.sous(n)).unwrap_or(0);
        if let Some(i) = presents.get(k).copied()
            && let Some(tel) = self.tels.get_mut(i)
        {
            tel.part = Some(self.pas.saturating_add(1));
        }
        let latence = self.t.entre(1, 4);
        let id = self.t.entier() | 1;
        self.appareils.push(id);
        self.tels.push(Telephone::new(
            id,
            StyleFil::Sage,
            self.pas.saturating_add(1800),
            latence,
        ));
    }

    /// Les premières connexions, et les départs pour de bon.
    fn arrivees_et_departs(&mut self) {
        let pas = self.pas;
        let mut annonces = Vec::new();
        for (i, tel) in self.tels.iter_mut().enumerate() {
            if tel.lien == Lien::Hors && tel.part.is_none() && tel.arrive == pas {
                tel.lien = Lien::Ok;
                annonces.push((pas, i, Annonce::Retour));
            }
            if tel.part == Some(pas) {
                tel.lien = Lien::Hors;
                tel.perdre_le_reseau();
                tel.evenements.clear();
                annonces.push((pas, i, Annonce::Depart(CauseDepart::Fermeture)));
            }
        }
        self.annonces.extend(annonces);
    }

    /// Les pannes qui commencent et finissent à ce pas.
    fn pannes_du_pas(&mut self) {
        let pas = self.pas;
        let duree = self.reglage.duree;
        for k in 0..self.programme.len() {
            let Some(p) = self.programme.get(k).cloned() else {
                continue;
            };
            match p.etat {
                Avancement::Attend if pas >= p.debut => {
                    let etat = self.commencer(&p);
                    if let Some(prog) = self.programme.get_mut(k) {
                        match etat {
                            Some(e) => prog.etat = e,
                            None if pas.saturating_add(60) < duree.saturating_mul(9) / 10 => {
                                prog.debut = pas.saturating_add(60);
                            }
                            None => {
                                prog.etat = Avancement::Manquee;
                                prog.note = String::from("aucun moment où elle pouvait commencer");
                            }
                        }
                    }
                }
                Avancement::EnCours { fin } if pas >= fin => {
                    let etat = self.finir_panne(&p);
                    if let Some(prog) = self.programme.get_mut(k) {
                        prog.etat = etat;
                    }
                }
                Avancement::Attend
                | Avancement::EnCours { .. }
                | Avancement::Constatee
                | Avancement::Manquee
                | Avancement::SansObjet => {}
            }
        }
    }

    /// Commence une panne ; `None` si elle ne peut pas commencer maintenant.
    #[expect(
        clippy::too_many_lines,
        reason = "un bras par panne du § 7.1, chacun avec ce qu'il fait au lien et aux présences"
    )]
    fn commencer(&mut self, p: &Programme) -> Option<Avancement> {
        let pas = self.pas;
        let fin = pas.saturating_add(p.panne.duree().map_or(0, Duree::en_pas));
        match p.panne {
            Panne::Rafales => {
                for (i, t) in self.tels.iter_mut().enumerate() {
                    if i.checked_rem(2) == Some(1) {
                        t.rafales = Oui::Oui;
                    }
                }
                Some(Avancement::EnCours {
                    fin: self.reglage.duree,
                })
            }
            // Ni redémarrage ni jumeau pendant un gel : l'autorité figée ne reçoit rien,
            // et la panne attend la fin du gel.
            Panne::RedemarrageAutorite if self.gel.is_some() => None,
            Panne::RedemarrageAutorite => self.redemarrer(),
            Panne::Gel { .. } => {
                if self.gel.is_some() {
                    return None;
                }
                self.gel = Some(fin);
                self.gels.push((pas, fin));
                Some(Avancement::EnCours { fin })
            }
            Panne::Jumeau => {
                if self.gel.is_some() {
                    return None;
                }
                let tel = self.tels.get_mut(p.cible)?;
                if tel.lien != Lien::Ok {
                    return None;
                }
                tel.jumeau_jusqua = pas.saturating_add(1200);
                Some(Avancement::EnCours {
                    fin: pas.saturating_add(1200),
                })
            }
            Panne::Coupure { duree }
            | Panne::Silence { duree }
            | Panne::TrouNoir { duree }
            | Panne::Retenu { duree } => {
                let annonce = {
                    let tel = self.tels.get_mut(p.cible)?;
                    if tel.lien != Lien::Ok || tel.present == Oui::Non || tel.part.is_some() {
                        return None;
                    }
                    match p.panne {
                        Panne::Coupure { .. } => {
                            tel.lien = Lien::Coupe { fin };
                            tel.perdre_le_reseau();
                            Some((
                                pas.saturating_add(FERMETURE.min(duree.en_pas() / 2)),
                                Annonce::Depart(CauseDepart::Fermeture),
                            ))
                        }
                        Panne::Silence { .. } => {
                            tel.lien = Lien::Silence { fin };
                            (duree.en_pas() > SILENCE_MAX).then(|| {
                                (
                                    pas.saturating_add(SILENCE_MAX),
                                    Annonce::Depart(CauseDepart::Silence),
                                )
                            })
                        }
                        Panne::TrouNoir { .. } => {
                            tel.lien = Lien::TrouNoir { fin };
                            tel.perdre_le_reseau();
                            (duree.en_pas() > SILENCE_MAX).then(|| {
                                (
                                    pas.saturating_add(SILENCE_MAX),
                                    Annonce::Depart(CauseDepart::Silence),
                                )
                            })
                        }
                        Panne::Retenu { .. }
                        | Panne::Rafales
                        | Panne::Rechargement
                        | Panne::Jumeau
                        | Panne::RedemarrageAutorite
                        | Panne::Gel { .. } => {
                            tel.lien = Lien::Retenu { fin };
                            for r in &mut tel.reseau {
                                r.livre_a = r.livre_a.max(fin);
                            }
                            None
                        }
                    }
                };
                if let Some((a, quoi)) = annonce {
                    self.annonces.push((a, p.cible, quoi));
                }
                Some(Avancement::EnCours { fin })
            }
            Panne::Rechargement => {
                let attente = self.t.entre(60, 180);
                let tel = self.tels.get_mut(p.cible)?;
                if tel.lien != Lien::Ok || tel.present == Oui::Non || tel.part.is_some() {
                    return None;
                }
                let fin = pas.saturating_add(attente);
                tel.lien = Lien::Recharge { fin };
                tel.perdre_le_reseau();
                // Ce que la page n'avait pas envoyé meurt avec elle ; son état tenu aussi.
                tel.evenements
                    .retain(|e| matches!(e.etat, EtatEvenement::Place(_)));
                tel.tenu = None;
                tel.vue.clear();
                tel.envoyee.clear();
                tel.vues.clear();
                self.annonces.push((
                    pas.saturating_add(FERMETURE),
                    p.cible,
                    Annonce::Depart(CauseDepart::Fermeture),
                ));
                Some(Avancement::EnCours { fin })
            }
        }
    }

    /// Finit une panne.
    fn finir_panne(&mut self, p: &Programme) -> Avancement {
        match p.panne {
            Panne::Rafales => Avancement::Manquee,
            // Tant qu'aucun geste du jumeau n'a été placé, l'onglet jumeau reste ouvert,
            // jusqu'aux neuf dixièmes de la soirée : un jeu dont les bots se taisent un
            // moment ne fait pas manquer la panne.
            Panne::Jumeau => {
                let limite = self.reglage.duree.saturating_mul(9) / 10;
                if self.pas < limite {
                    let pas = self.pas;
                    if let Some(t) = self.tels.get_mut(p.cible) {
                        t.jumeau_jusqua = pas.saturating_add(600);
                    }
                    Avancement::EnCours {
                        fin: pas.saturating_add(600),
                    }
                } else {
                    Avancement::Manquee
                }
            }
            Panne::RedemarrageAutorite | Panne::Gel { .. } => Avancement::Constatee,
            Panne::Coupure { .. }
            | Panne::Silence { .. }
            | Panne::TrouNoir { .. }
            | Panne::Retenu { .. }
            | Panne::Rechargement => {
                let pas = self.pas;
                let gigue = self.t.sous(3);
                if let Some(tel) = self.tels.get_mut(p.cible) {
                    if matches!(tel.lien, Lien::Hors) {
                        return Avancement::Manquee;
                    }
                    tel.lien = Lien::Ok;
                    tel.envoyer_les_gestes(pas, gigue);
                    tel.envoyer_le_tenu(pas, gigue);
                    tel.envoyee.clear();
                }
                self.annonces.push((pas, p.cible, Annonce::Retour));
                self.forcer_vues = true;
                Avancement::Constatee
            }
        }
    }

    /// Une panne qui se constate en cours de route : rafale, jumeau.
    fn constater(&mut self, quoi: fn(&Panne) -> bool) {
        for p in &mut self.programme {
            if quoi(&p.panne) && matches!(p.etat, Avancement::EnCours { .. }) {
                p.etat = Avancement::Constatee;
            }
        }
    }

    /// L'autorité redémarre depuis le dernier point et le journal (§ 4.5).
    fn redemarrer(&mut self) -> Option<Avancement> {
        let point = self.dernier_point.clone()?;
        let journal = self.partie.journal().clone();
        self.script.push(Op::Canonique);
        let avant = self.partie.canonique();
        let empreinte = self.obs.borrow().derniere_ecriture;
        let reference = self.obs.borrow().entrees.clone();
        self.script.push(Op::Redemarrer {
            lignes: journal.lignes().len(),
        });
        {
            let mut obs = self.obs.borrow_mut();
            let p = partie_au(&journal, point.indice());
            obs.partie_reprise = p;
            obs.prochaine_partie = p.saturating_add(1);
        }
        let reprise = Partie::reprendre(
            Sonde::new(self.module.fabrique(), &self.obs),
            self.ouverture.clone(),
            &point,
            &journal,
        );
        match reprise {
            Ok(p) => {
                self.partie = p;
                self.e14.evaluer();
                if let Some((p, o)) =
                    premiere_entree_differente(&reference, &self.obs.borrow().entrees)
                {
                    self.e14.echouer(format!(
                        "au pas {} : le redémarrage diverge à l'entrée d'ordre {o} de la partie {p}",
                        self.pas
                    ));
                }
                match (avant, self.partie.canonique()) {
                    (Ok(a), Ok(b)) if a == b => {}
                    (Ok(_), Ok(_)) => self.e14.echouer(format!(
                        "au pas {} : la forme canonique après le redémarrage n'est pas celle d'avant",
                        self.pas
                    )),
                    (a, b) => self.e14.echouer(format!(
                        "au pas {} : forme canonique illisible autour du redémarrage ({:?}, {:?})",
                        self.pas,
                        a.err(),
                        b.err()
                    )),
                }
                // L'empreinte juste après la dernière écriture rejouée, celle que la
                // sonde relève avant ses propres lectures.
                let apres = self.obs.borrow().derniere_ecriture;
                if let (Some(a), Some(b)) = (empreinte, apres)
                    && a != b
                {
                    self.e14.echouer(format!(
                        "au pas {} : l'empreinte de la mémoire après le redémarrage n'est pas celle d'avant",
                        self.pas
                    ));
                }
                let definitif = self.partie.definitif().numero();
                let pas = self.pas;
                for tel in &mut self.tels {
                    tel.renvoyer_le_provisoire(definitif);
                    tel.envoyer_les_gestes(pas, 0);
                    tel.envoyee.clear();
                }
                self.forcer_vues = true;
                self.lignes_vues = usize::MAX;
                Some(Avancement::Constatee)
            }
            Err(RefusReprise::Panne(p)) => {
                self.e14.evaluer();
                self.e14
                    .echouer(format!("au pas {} : le redémarrage échoue : {p}", self.pas));
                self.stop = Some(String::from("l'autorité n'a pas pu redémarrer"));
                Some(Avancement::Constatee)
            }
            Err(autre) => {
                self.e14.evaluer();
                self.e14.echouer(format!(
                    "au pas {} : le redémarrage est refusé : {autre:?}",
                    self.pas
                ));
                self.stop = Some(String::from("l'autorité n'a pas pu redémarrer"));
                Some(Avancement::Constatee)
            }
        }
    }

    /// Les présences annoncées, les paquets arrivés, les réponses dues.
    fn livrer(&mut self) {
        let pas = self.pas;
        let (dues, restent): (Vec<_>, Vec<_>) = std::mem::take(&mut self.annonces)
            .into_iter()
            .partition(|(a, _, _)| *a <= pas);
        self.annonces = restent;
        for (_, i, quoi) in dues {
            self.annoncer(i, quoi);
        }
        for i in 0..self.tels.len() {
            let (paquets, rafales) = match self.tels.get_mut(i) {
                Some(t) if t.lien.envoie() => (t.arrivent(pas), t.rafales),
                Some(_) | None => (Vec::new(), Oui::Non),
            };
            if rafales == Oui::Oui && paquets.len() >= 2 {
                self.constater(|p| matches!(p, Panne::Rafales));
            }
            for paquet in paquets {
                self.livrer_paquet(i, paquet);
            }
        }
        let (dues, restent): (Vec<_>, Vec<_>) = std::mem::take(&mut self.demandes)
            .into_iter()
            .partition(|(_, _, a)| *a <= pas);
        self.demandes = restent;
        for (id, d, _) in dues {
            let reponse = match d {
                DemandeArrivee::Image { .. } => ReponseArrivee::Image(Err(EchecDemande::Refuse)),
                DemandeArrivee::Rendu { .. } => ReponseArrivee::Rendu(Ok(b"PNG".to_vec())),
                DemandeArrivee::Lire { .. } => ReponseArrivee::Lu(Err(EchecDemande::Illisible)),
                DemandeArrivee::Telecharger { .. } => ReponseArrivee::Fait(Ok(())),
                // Une ligne sur dix échoue : un échec revient comme une réponse (SQ-78).
                DemandeArrivee::Ligne { .. } => {
                    if self.t.une_sur(10) {
                        ReponseArrivee::Fait(Err(EchecDemande::Interne))
                    } else {
                        ReponseArrivee::Fait(Ok(()))
                    }
                }
            };
            let _: Option<partie::Placement> = self.arriver(Arrivee::Reponse { id, reponse });
        }
    }

    fn annoncer(&mut self, i: usize, quoi: Annonce) {
        let Some(tel) = self.tels.get_mut(i) else {
            return;
        };
        let appareil = Appareil::du_fil(tel.appareil);
        let p = match quoi {
            Annonce::Depart(cause) => {
                if tel.present == Oui::Non {
                    return;
                }
                tel.present = Oui::Non;
                Presence::Depart { appareil, cause }
            }
            Annonce::Retour => {
                if tel.present == Oui::Oui || !matches!(tel.lien, Lien::Ok) {
                    return;
                }
                tel.present = Oui::Oui;
                let p = match tel.connu {
                    Oui::Oui => Presence::Retour {
                        appareil,
                        capacites: Capacites::AUCUNE,
                    },
                    Oui::Non => Presence::Arrivee {
                        appareil,
                        role: RoleAppareil::Telephone,
                        capacites: Capacites::AUCUNE,
                    },
                };
                tel.connu = Oui::Oui;
                tel.envoyee.clear();
                p
            }
        };
        self.forcer_vues = true;
        let _: Option<partie::Placement> = self.arriver(Arrivee::Presence(p));
    }

    fn livrer_paquet(&mut self, i: usize, paquet: Paquet) {
        let Some(tel) = self.tels.get(i) else {
            return;
        };
        let de = Appareil::du_fil(tel.appareil);
        let demi = Duree::pas(tel.latence);
        let vu = Pas::du_fil(tel.vu);
        let (a, id, jumeau) = match paquet {
            Paquet::Geste { id } => {
                let Some(e) = tel
                    .evenements
                    .iter()
                    .find(|e| e.id == id && e.etat == EtatEvenement::EnVol)
                else {
                    return;
                };
                (
                    Arrivee::Geste {
                        de,
                        octets: e.octets.clone(),
                        fait: Instant {
                            pas: Pas::du_fil(e.fait.0),
                            fraction: e.fait.1,
                        },
                        vu,
                        id: jeu_abi::IdGeste::du_fil(id),
                        demi_aller_retour: demi,
                    },
                    Some(id),
                    e.jumeau,
                )
            }
            Paquet::Tenu {
                octets,
                numero,
                fait,
            } => (
                Arrivee::Tenu {
                    de,
                    octets,
                    numero,
                    fait: Instant {
                        pas: Pas::du_fil(fait.0),
                        fraction: fait.1,
                    },
                    demi_aller_retour: demi,
                },
                None,
                Oui::Non,
            ),
        };
        let placement = self.arriver(a);
        let Some(id) = id else {
            return;
        };
        let place = match placement {
            Some(partie::Placement::Place { pas }) => Some(EtatEvenement::Place(pas.numero())),
            Some(partie::Placement::FilePleine) => Some(EtatEvenement::AEnvoyer),
            Some(
                partie::Placement::Malforme(_)
                | partie::Placement::HorsScene
                | partie::Placement::Perimee
                | partie::Placement::Ecartee,
            )
            | None => None,
        };
        if jumeau == Oui::Oui && matches!(place, Some(EtatEvenement::Place(_))) {
            self.constater(|p| matches!(p, Panne::Jumeau));
        }
        if let Some(tel) = self.tels.get_mut(i) {
            match place {
                Some(etat) => {
                    if let Some(e) = tel.evenements.iter_mut().find(|e| e.id == id) {
                        e.etat = etat;
                    }
                }
                None => tel.evenements.retain(|e| e.id != id),
            }
        }
    }

    /// Un appel `Bot` sur la définitive : les coups du bot.
    fn bot(&mut self, regard: RegardFil, style: StyleFil, depuis_vue: u64) -> Vec<Coup> {
        let graine = self.t.entier();
        let appel = Appel::Bot {
            regard,
            style,
            graine,
            depuis_vue,
        };
        match self.partie.definitive().lire(&appel) {
            Ok(r) => {
                if let Ok(Retour::Bot(coups)) = decoder::<Retour<'_>>(&r) {
                    coups.iter().map(Coup::de).collect()
                } else {
                    self.faute(IdControle::E2, "Bot", &Faute::SortieInvalide);
                    Vec::new()
                }
            }
            Err(Echec::Faute(f)) => {
                self.faute(IdControle::E2, "Bot", &f);
                Vec::new()
            }
            Err(Echec::Panne(p)) => {
                self.panne_outil(format!("Bot : {p}"));
                Vec::new()
            }
        }
    }

    /// Une faute du jeu pendant une lecture de l'épreuve : la définitive n'est plus
    /// sûre, et la soirée s'arrête.
    fn faute(&mut self, controle: IdControle, ou: &str, f: &Faute) {
        let c = match self.reglage.sorte {
            Sorte::Principale => controle,
            Sorte::Hostile | Sorte::Longue | Sorte::Proprietes | Sorte::Gel => {
                controle_des_fautes(self.reglage.sorte)
            }
        };
        self.fautes.push(FauteVue {
            controle: c,
            detail: format!("au pas {}, {ou} : {f}", self.pas),
        });
        if self.stop.is_none() {
            self.stop = Some(format!(
                "le jeu a fauté pendant une lecture de l'épreuve ({ou})"
            ));
        }
    }

    fn bot_ecran(&mut self) {
        if self.pas < self.ecran.prochain {
            return;
        }
        self.ecran.prochain = self
            .pas
            .saturating_add(PERIODE_ECRAN)
            .saturating_add(self.t.sous(5));
        let depuis = self.pas.saturating_sub(self.ecran.depuis);
        let coups = self.bot(RegardFil::Ecran, StyleFil::Sage, depuis);
        let vu = Pas::du_fil(self.ecran.vu);
        for c in coups {
            let quoi = match c {
                Coup::Action(a) => EcranArrivee::Action(a),
                Coup::Touche(t) => EcranArrivee::Touche(t),
                Coup::Composant(c) => EcranArrivee::Composant(c),
                Coup::Geste(_) | Coup::Tenu(_) | Coup::Rien => continue,
            };
            let _: Option<partie::Placement> = self.arriver(Arrivee::Ecran { quoi, vu });
        }
    }

    fn bots_telephones(&mut self) {
        let pas = self.pas;
        for i in 0..self.tels.len() {
            let Some(tel) = self.tels.get(i) else {
                continue;
            };
            if !tel.lien.agit() || tel.connu == Oui::Non || pas < tel.prochain_bot {
                continue;
            }
            let regard = RegardFil::Appareil(tel.appareil);
            let style = tel.style;
            let depuis = pas.saturating_sub(tel.vue_depuis);
            let jumeau = pas < tel.jumeau_jusqua;
            let periode = match (self.reglage.sorte, style) {
                (Sorte::Longue, _) => PERIODE_LONGUE,
                (_, StyleFil::Lent) => PERIODE_LENTE,
                (_, StyleFil::Hostile) => PERIODE_HOSTILE,
                (_, StyleFil::Sage) => PERIODE_SAGE,
            };
            let prochain = pas.saturating_add(periode).saturating_add(self.t.sous(5));
            let mut coups: Vec<(Coup, Oui)> = self
                .bot(regard, style, depuis)
                .into_iter()
                .map(|c| (c, Oui::Non))
                .collect();
            if jumeau {
                coups.extend(
                    self.bot(regard, style, depuis)
                        .into_iter()
                        .map(|c| (c, Oui::Oui)),
                );
            }
            for (c, j) in coups {
                let fraction = self.t.fraction();
                let gigue = self.t.sous(3);
                match c {
                    Coup::Geste(o) => {
                        self.echantillonner(SorteOctets::GesteTelephone, &o);
                        if let Some(tel) = self.tels.get_mut(i) {
                            tel.emettre(o, (pas, fraction), j, pas, gigue);
                        }
                    }
                    Coup::Tenu(o) => {
                        self.echantillonner(SorteOctets::TenuTelephone, &o);
                        if let Some(tel) = self.tels.get_mut(i) {
                            tel.tenir(o, (pas, fraction), pas, gigue);
                        }
                    }
                    Coup::Action(_) | Coup::Touche(_) | Coup::Composant(_) | Coup::Rien => {}
                }
            }
            if let Some(tel) = self.tels.get_mut(i) {
                tel.prochain_bot = prochain;
            }
        }
    }

    /// Garde un échantillon des octets que les bots envoient, pour les abîmer (E13) :
    /// un réservoir, pour que les gestes de toute la soirée y aient leur chance.
    fn echantillonner(&mut self, sorte: SorteOctets, o: &[u8]) {
        self.vus_pour_echantillon = self.vus_pour_echantillon.saturating_add(1);
        if self.echantillons.len() < ECHANTILLONS_MAX {
            self.echantillons.push((sorte, o.to_vec()));
            return;
        }
        let k = self.absurde.sous(self.vus_pour_echantillon);
        if let Some(place) = usize::try_from(k)
            .ok()
            .and_then(|k| self.echantillons.get_mut(k))
        {
            *place = (sorte, o.to_vec());
        }
    }

    /// Les vues en route qui arrivent sur les téléphones.
    fn vues_des_telephones(&mut self) {
        let pas = self.pas;
        for tel in &mut self.tels {
            let (arrivees, restent): (Vec<VueEnRoute>, Vec<VueEnRoute>) =
                std::mem::take(&mut tel.vues)
                    .into_iter()
                    .partition(|v| v.livre_a <= pas);
            tel.vues = restent;
            for v in arrivees {
                tel.vu = tel.vu.max(v.pas);
                if v.octets != tel.vue {
                    tel.vue = v.octets;
                    tel.vue_depuis = pas;
                }
            }
        }
    }

    fn avancer(&mut self) {
        let cible = Pas::du_fil(self.pas);
        self.script.push(Op::Avancer(cible));
        let a = match self.partie.avancer(cible) {
            Ok(a) => a,
            Err(p) => {
                self.panne_outil(format!("avancer : {p}"));
                return;
            }
        };
        self.traiter(a);
    }

    fn traiter(&mut self, a: Avancee) {
        let controle = controle_des_fautes(self.reglage.sorte);
        for r in a.fautes {
            self.fautes.push(FauteVue {
                controle,
                detail: format!("au pas {} : {} ({:?})", self.pas, r.faute, r.lieu),
            });
        }
        for (id, d) in a.lancer {
            let delai = self.t.entre(12, 120);
            self.demandes.push((id, d, self.pas.saturating_add(delai)));
        }
        for id in a.annuler {
            self.demandes.retain(|(i, _, _)| *i != id);
        }
        for o in a.oublier {
            if let Some(t) = self.tels.iter_mut().find(|t| t.appareil == o.fil()) {
                t.connu = Oui::Non;
            }
        }
        for r in a.retirer {
            if let Some(k) = self.tels.iter().position(|t| t.appareil == r.fil()) {
                if let Some(t) = self.tels.get_mut(k) {
                    t.connu = Oui::Non;
                    let fin = self.pas.saturating_add(300);
                    t.lien = Lien::Recharge { fin };
                    t.perdre_le_reseau();
                }
                self.annonces
                    .push((self.pas, k, Annonce::Depart(CauseDepart::Voulu)));
                self.programme.push(Programme {
                    du_plan: Oui::Non,
                    panne: Panne::Rechargement,
                    debut: 0,
                    cible: k,
                    etat: Avancement::EnCours {
                        fin: self.pas.saturating_add(300),
                    },
                    note: String::from("retiré par le jeu"),
                });
            }
        }
        if a.saut > Duree::ZERO {
            for p in &mut self.programme {
                if matches!(p.panne, Panne::Gel { .. })
                    && matches!(p.etat, Avancement::EnCours { .. })
                {
                    p.etat = Avancement::Constatee;
                }
            }
        }
        if a.en_panne == Oui::Oui && !self.en_panne_vue {
            self.en_panne_vue = true;
            self.fautes.push(FauteVue {
                controle,
                detail: format!(
                    "au pas {} : trois `Temps` écartés de suite, la partie est en panne",
                    self.pas
                ),
            });
        }
        let definitif = a.definitif.numero();
        for t in &mut self.tels {
            t.confirmer(definitif);
        }
    }

    /// Les regards présents : l'écran, et chaque téléphone annoncé.
    fn regards(&self) -> Vec<RegardFil> {
        let mut r = vec![RegardFil::Ecran];
        for t in &self.tels {
            if t.present == Oui::Oui {
                r.push(RegardFil::Appareil(t.appareil));
            }
        }
        r
    }

    /// Les vues, envoyées à qui les attend. `None` si la partie a lâché.
    fn regarder(&mut self) -> Option<()> {
        let regards = self.regards();
        self.obs.borrow_mut().regards.clone_from(&regards);
        self.script.push(Op::Regarder(regards.clone()));
        let vues = match self.partie.regarder(&regards) {
            Ok(v) => v,
            Err(p) => {
                self.panne_outil(format!("regarder : {p}"));
                return None;
            }
        };
        let pas = self.pas;
        for v in vues {
            match v.regard {
                RegardFil::Ecran => {
                    if self.reglage.sorte == Sorte::Principale {
                        self.e17.evaluer();
                        match decoder::<jeu_abi::SceneFil>(&v.octets) {
                            Ok(scene) => {
                                let n = controles::fixes_satures(&scene);
                                if n > 0 {
                                    self.e17.echouer(format!(
                                        "au pas {pas} : {n} longueur(s) saturée(s) dans la scène de l'écran"
                                    ));
                                }
                            }
                            Err(e) => self.e17.echouer(format!(
                                "au pas {pas} : la scène de l'écran ne se relit pas : {e}"
                            )),
                        }
                    }
                    self.ecran.vu = v.pas.numero();
                    if v.octets != self.ecran.vue {
                        self.ecran.vue = v.octets;
                        self.ecran.depuis = pas;
                    }
                }
                RegardFil::Appareil(a) => {
                    let gigue = self.t.sous(3);
                    if let Some(t) = self.tels.iter_mut().find(|t| t.appareil == a)
                        && v.octets != t.envoyee
                        && t.lien.agit()
                    {
                        t.envoyee.clone_from(&v.octets);
                        let livre_a = t.livraison(pas, gigue);
                        if !matches!(t.lien, Lien::Coupe { .. } | Lien::TrouNoir { .. }) {
                            t.vues.push(VueEnRoute {
                                livre_a,
                                pas: v.pas.numero(),
                                octets: v.octets,
                            });
                        }
                    }
                }
            }
        }
        self.lignes_vues = self.partie.journal().lignes().len();
        Some(())
    }

    /// Après le pas : les vues, l'état, et les constats qui s'y lisent.
    fn apres_le_pas(&mut self) {
        let lignes = self.partie.journal().lignes().len();
        if (lignes != self.lignes_vues || self.forcer_vues) && self.stop.is_none() {
            self.forcer_vues = false;
            let _: Option<()> = self.regarder();
        }
        let acceptees = self.obs.borrow().acceptees;
        let partie = self.partie.definitive().partie();
        if acceptees == self.acceptees_vues && partie == self.partie_vue {
            return;
        }
        self.acceptees_vues = acceptees;
        self.partie_vue = partie;
        self.faits.parties = self.faits.parties.max(partie.saturating_add(1));
        self.script.push(Op::Etat);
        let etat = match self.partie.etat() {
            Ok(e) => e,
            Err(Echec::Faute(_)) => return,
            Err(Echec::Panne(p)) => {
                self.panne_outil(format!("etat : {p}"));
                return;
            }
        };
        self.faits.phases.insert(etat.phase_indice);
        self.faits.cas.insert(etat.cas_indice);
        self.echeance = etat.attente.echeance.map(Pas::numero);
        if self.reglage.sorte == Sorte::Principale
            && self.pas >= self.dernier_e17.saturating_add(PERIODE_SECRETS)
        {
            self.dernier_e17 = self.pas;
            self.verifier_les_flottants();
        }
        if etat.terminale == Oui::Oui {
            self.faits.terminales.insert(partie);
        }
        let phase_changee = self.derniere_phase != Some(etat.phase_indice);
        self.derniere_phase = Some(etat.phase_indice);
        if self.reglage.e5 == Oui::Oui {
            self.verifier_attente(&etat.attente);
        }
        if self.reglage.e8 == Oui::Oui
            && (phase_changee || self.pas >= self.dernier_e8.saturating_add(PERIODE_SECRETS))
        {
            self.dernier_e8 = self.pas;
            self.verifier_secrets();
        }
    }

    fn copie(&mut self) -> Option<&mut InstanceWasm> {
        if self.copie.is_none() {
            self.copie = self.instance_nue();
        }
        self.copie.as_mut()
    }

    /// Une instance nue du module, accueillie : `Bonjour` et `Decrire`.
    fn instance_nue(&mut self) -> Option<InstanceWasm> {
        let mut i = match self.module.fabrique().neuve() {
            Ok(i) => i,
            Err(p) => {
                self.panne_outil(format!("instance de l'épreuve : {p}"));
                return None;
            }
        };
        for a in [Appel::Bonjour, Appel::Decrire] {
            if let Err(e) = i.appeler(SorteAppel::Lecture, &octets(&a), BUDGETS.lecture) {
                self.panne_outil(format!("instance de l'épreuve : {e}"));
                return None;
            }
        }
        Some(i)
    }

    /// E5 : chaque attendu présent a un exemple accepté (essayé sur une copie), ou une
    /// échéance existe ; un attendu absent sans échéance est une faute (CAT-78).
    ///
    /// Une échéance ne compte que si elle peut arriver : le réveil du jeu n'est pas
    /// `Jamais`, et elle n'est pas passée de plus
    /// de [`ECHEANCE_PASSEE_MAX`] pas. `libre` lève l'exigence d'une fin, pas celle-ci :
    /// un attendu absent reste une faute, et un attendu présent doit avoir un exemple
    /// accepté (« l'écran a toujours une action acceptée »).
    #[expect(
        clippy::too_many_lines,
        reason = "le constat se lit d'un trait : attendus présents, absents, puis l'essai de chaque exemple"
    )]
    fn verifier_attente(&mut self, a: &partie::AttenteVisible) {
        self.e5.evaluer();
        let definitif = self.partie.definitif().numero();
        let reveil = self.partie.coeur().reveil();
        // Après un gel, le numéro saute : une échéance passée pendant le gel arrive au
        // `Temps` qui suit le saut, pas avant.
        let dernier_saut = self
            .partie
            .journal()
            .lignes()
            .iter()
            .rev()
            .take(256)
            .find_map(|l| match l {
                Ligne::Saut { a, .. } => Some(a.numero()),
                Ligne::Arrivees { .. }
                | Ligne::Entree(_)
                | Ligne::Empreinte { .. }
                | Ligne::Pause { .. }
                | Ligne::Vue { .. }
                | Ligne::Partie { .. }
                | Ligne::Reprise { .. } => None,
            })
            .unwrap_or(0);
        let echeance = a.echeance.map(Pas::numero);
        if let Some(e) = echeance
            && echeance_vivante(e.max(dernier_saut), definitif, reveil)
        {
            return;
        }
        let sans = match echeance {
            Some(e) => format!("avec une échéance au pas {e} qui n'arrivera pas ({reveil:?})"),
            None => String::from("sans échéance"),
        };
        let mut presents = Vec::new();
        let mut absents = Vec::new();
        for (qui, exemples) in &a.joueurs {
            let present = self
                .tels
                .iter()
                .any(|t| t.appareil == qui.fil() && t.present == Oui::Oui);
            if present {
                presents.push((Some(*qui), exemples.clone()));
            } else {
                absents.push(qui.fil());
            }
        }
        if !a.ecran.is_empty() {
            presents.push((None, a.ecran.clone()));
        }
        if !absents.is_empty() {
            self.e5.echouer(format!(
                "au pas {} : la partie attend {} appareil(s) absent(s), {sans}",
                self.pas,
                absents.len()
            ));
            return;
        }
        // `libre` : rien n'avance seul, par nature (un éditeur). Sans attendu, il
        // n'y a rien à essayer ; un attendu, lui, doit avoir un exemple accepté.
        if presents.is_empty() && a.libre.is_some() {
            return;
        }
        if presents.is_empty() {
            self.e5.echouer(format!(
                "au pas {} : la partie n'attend personne, {sans} : elle est figée",
                self.pas
            ));
            return;
        }
        let instantane = match self.partie.definitive().instantane_epreuve() {
            Ok(i) => i,
            Err(p) => {
                self.panne_outil(format!("E5, instantané : {p}"));
                return;
            }
        };
        let place = self.partie.definitif().numero().saturating_add(1);
        for (qui, exemples) in presents {
            let mut accepte = false;
            for (k, ex) in exemples.iter().enumerate() {
                let quand = QuandFil {
                    fait: place,
                    fraction: 0,
                    recu: place,
                    vu: place.saturating_sub(1),
                    id: (1 << 61) | u64::try_from(k).unwrap_or(0),
                };
                let entree = match qui {
                    Some(de) => EntreeFil::Geste {
                        de: de.fil(),
                        octets: ex,
                        quand,
                    },
                    None => EntreeFil::Ecran {
                        quoi: jeu_abi::EcranFil::Action(ex),
                        quand,
                    },
                };
                let e = octets(&entree);
                let appel = octets(&Appel::Entree {
                    ordre: 0,
                    place,
                    entree: &e,
                });
                let Some(copie) = self.copie() else {
                    return;
                };
                if let Err(p) = copie.restaurer(&instantane) {
                    self.panne_outil(format!("E5, restaurer la copie : {p}"));
                    return;
                }
                match copie.appeler(SorteAppel::Ecriture, &appel, BUDGETS.entree) {
                    Ok(r) => {
                        if matches!(
                            decoder::<Retour<'_>>(&r),
                            Ok(Retour::Issue(IssueFil::Acceptee { .. }))
                        ) {
                            accepte = true;
                            break;
                        }
                    }
                    Err(Echec::Faute(_)) => {}
                    Err(Echec::Panne(p)) => {
                        self.panne_outil(format!("E5, l'essai sur la copie : {p}"));
                        return;
                    }
                }
            }
            if !accepte {
                let qui = match qui {
                    Some(_) => "d'un joueur présent",
                    None => "de l'écran",
                };
                self.e5.echouer(format!(
                    "au pas {} : aucun exemple {qui} n'est accepté, {sans}",
                    self.pas
                ));
            }
        }
    }

    /// E17 : aucun flottant non fini dans la forme canonique (CAT-05, SQ-68).
    fn verifier_les_flottants(&mut self) {
        match self.partie.definitive().canonique() {
            Ok(c) => self.flottants_de(&c),
            Err(Echec::Faute(f)) => self.faute(IdControle::E17, "Canonique", &f),
            Err(Echec::Panne(p)) => self.panne_outil(format!("E17, canonique : {p}")),
        }
    }

    fn flottants_de(&mut self, canonique: &[u8]) {
        self.e17.evaluer();
        match jeu_abi::lire_canonique(canonique) {
            Some(c) if c.non_finis == 0 => {}
            Some(c) => self.e17.echouer(format!(
                "au pas {} : {} flottant(s) non fini(s) dans la forme canonique",
                self.pas, c.non_finis
            )),
            None => self.e17.echouer(format!(
                "au pas {} : la forme canonique n'a pas ses bords",
                self.pas
            )),
        }
    }

    /// E8 : `vue(état, r) = vue(projection(état, r), r)` pour chaque regard, et les
    /// octets de chaque secret que `r` ne voit pas cherchés dans sa vue (CAT-46).
    fn verifier_secrets(&mut self) {
        let regards = self.regards();
        let canonique = match self.partie.definitive().canonique() {
            Ok(c) => c,
            Err(Echec::Faute(f)) => {
                self.faute(IdControle::E8, "Canonique", &f);
                return;
            }
            Err(Echec::Panne(p)) => {
                self.panne_outil(format!("E8, canonique : {p}"));
                return;
            }
        };
        if self.projection.is_none() {
            self.projection = self.instance_nue();
        }
        // L'instance de projection part de la mémoire de la définitive, puis `Charger`
        // remplace l'état par sa projection : seul l'état projeté diffère. Ce que le
        // module tiendrait hors de l'état (un `static`) n'est pas l'affaire de E8.
        let instantane = match self.partie.definitive().instantane_epreuve() {
            Ok(i) => i,
            Err(p) => {
                self.panne_outil(format!("E8, instantané : {p}"));
                return;
            }
        };
        if let Some(inst) = self.projection.as_mut()
            && let Err(p) = inst.restaurer(&instantane)
        {
            self.panne_outil(format!("E8, restaurer l'instance de projection : {p}"));
            return;
        }
        for regard in regards {
            let def = self.partie.definitive();
            let projection =
                crate::sonde::recoller(|morceau| def.lire(&Appel::Projeter { regard, morceau }));
            let vue = def.lire(&Appel::Regarder {
                regards: vec![regard],
            });
            let (projection, vue) = match (projection, vue) {
                (Ok(p), Ok(v)) => (p, v),
                (Err(Echec::Faute(f)), _) | (_, Err(Echec::Faute(f))) => {
                    self.faute(IdControle::E8, "Projeter", &f);
                    return;
                }
                (Err(Echec::Panne(p)), _) | (_, Err(Echec::Panne(p))) => {
                    self.panne_outil(format!("E8, projeter : {p}"));
                    return;
                }
            };
            let Some(inst) = self.projection.as_mut() else {
                return;
            };
            let charger = octets(&Appel::Charger {
                projection: &projection,
            });
            let lue = inst
                .appeler(SorteAppel::Ecriture, &charger, BUDGETS.entree)
                .and_then(|_| {
                    inst.appeler(
                        SorteAppel::Lecture,
                        &octets(&Appel::Regarder {
                            regards: vec![regard],
                        }),
                        BUDGETS.lecture,
                    )
                });
            self.e8.evaluer();
            let pas = self.pas;
            match lue {
                Ok(v) => {
                    if hacher(&v) != hacher(&vue) {
                        self.e8.echouer(format!(
                            "au pas {pas}, regard {regard:?} : la vue de l'état n'est pas celle de sa projection"
                        ));
                    }
                }
                Err(e) => self.e8.echouer(format!(
                    "au pas {pas}, regard {regard:?} : la projection ne se recharge pas : {e}"
                )),
            }
            let vue_seule = controles::octets_de_la_vue(&vue);
            if let Some(caches) = controles::secrets_caches(&canonique, &projection) {
                self.faits.secrets_non_cherches = self
                    .faits
                    .secrets_non_cherches
                    .saturating_add(caches.trop_courts);
                for s in caches.cherches {
                    if controles::trouve(&vue_seule, &s) {
                        self.e8.echouer(format!(
                            "au pas {pas}, regard {regard:?} : un secret qu'il ne voit pas ({} octets) est dans sa vue",
                            s.len()
                        ));
                        break;
                    }
                }
            }
        }
    }

    /// Les arrivées absurdes de E13 : octets au hasard dans chaque champ (SAL-117,
    /// REM-58, REM-59).
    fn absurdes(&mut self) {
        if !self.absurde.une_sur(2) {
            return;
        }
        self.e13.evaluer();
        let connus: Vec<u64> = self.tels.iter().map(|t| t.appareil).collect();
        let a =
            controles::arrivee_absurde(&mut self.absurde, &connus, &self.echantillons, self.pas);
        match self.arriver(a) {
            Some(_) | None => {}
        }
    }

    /// Un point de reprise, noté au script.
    fn prendre_point(&mut self, garder: Oui) {
        self.script.push(Op::Point);
        match self.partie.point() {
            Ok(p) => {
                if garder == Oui::Oui || (self.points.len() < 4 && self.tp.une_sur(3)) {
                    self.points.push(p.clone());
                }
                self.dernier_point = Some(p);
            }
            Err(p) => self.panne_outil(format!("point : {p}")),
        }
    }

    /// E15 : les octets vivants, et les appareils que l'état retient.
    fn mesurer_la_memoire(&mut self) {
        let def = self.partie.definitive();
        let vivants = match def.lire(&Appel::Memoire) {
            Ok(r) => match decoder::<Retour<'_>>(&r) {
                Ok(Retour::Memoire { vivants, .. }) => vivants,
                Ok(_) | Err(_) => return,
            },
            Err(Echec::Faute(f)) => {
                self.faute(IdControle::E15, "Memoire", &f);
                return;
            }
            Err(Echec::Panne(p)) => {
                self.panne_outil(format!("E15, mémoire : {p}"));
                return;
            }
        };
        let canonique = match def.canonique() {
            Ok(c) => c,
            Err(Echec::Faute(f)) => {
                self.faute(IdControle::E15, "Canonique", &f);
                return;
            }
            Err(Echec::Panne(p)) => {
                self.panne_outil(format!("E15, canonique : {p}"));
                return;
            }
        };
        let retenus = controles::appareils_retenus(&canonique, &self.appareils);
        let presents = self.tels.iter().filter(|t| t.present == Oui::Oui).count();
        self.memoire.push(Echantillon {
            pas: self.pas,
            vivants,
            retenus,
            presents: u64::try_from(presents).unwrap_or(0),
        });
    }

    /// La fin de la soirée : la forme canonique finale, les reprises de E14, le
    /// bilan.
    fn finir(mut self) -> Bilan {
        self.script.push(Op::Canonique);
        let canonique_finale = match self.partie.canonique() {
            Ok(c) => {
                if self.reglage.sorte == Sorte::Principale {
                    self.flottants_de(&c);
                }
                Some(c)
            }
            Err(Echec::Faute(_)) => None,
            Err(Echec::Panne(p)) => {
                self.panne_outil(format!("canonique finale : {p}"));
                None
            }
        };
        let journal = self.partie.journal().clone();
        if self.reglage.e14 == Oui::Oui && self.stop.is_none() {
            self.reprises(&journal, canonique_finale.as_deref());
        }
        let regards = {
            let mut r = vec![RegardFil::Ecran];
            r.extend(self.appareils.iter().map(|a| RegardFil::Appareil(*a)));
            r
        };
        let duree = self.reglage.duree;
        let pannes_sans_objet = self
            .programme
            .iter()
            .filter(|p| p.du_plan == Oui::Oui && p.etat == Avancement::SansObjet)
            .map(|p| p.panne)
            .collect();
        let pannes = self
            .programme
            .iter()
            .filter(|p| p.du_plan == Oui::Oui && p.etat != Avancement::SansObjet)
            .map(|p| {
                let (oui, note) = match p.etat {
                    Avancement::Constatee => (Oui::Oui, String::from("constatée")),
                    Avancement::Manquee if !p.note.is_empty() => (Oui::Non, p.note.clone()),
                    Avancement::Attend => (Oui::Non, String::from("jamais commencée")),
                    Avancement::EnCours { fin } if fin > duree => {
                        (Oui::Non, String::from("pas finie à la fin de la soirée"))
                    }
                    Avancement::Manquee | Avancement::EnCours { .. } | Avancement::SansObjet => {
                        (Oui::Non, String::from("jamais constatée"))
                    }
                };
                (p.panne, oui, note)
            })
            .collect();
        Bilan {
            ouverture: self.ouverture,
            journal,
            script: self.script,
            fautes: self.fautes,
            pannes_outil: self.pannes_outil,
            faits: self.faits,
            obs: self.obs,
            e5: self.e5,
            e8: self.e8,
            e13: self.e13,
            e14: self.e14,
            pannes,
            pannes_sans_objet,
            memoire: self.memoire,
            regards,
            echantillons: self.echantillons,
            abandon: self.stop,
            pas_joues: self.pas,
            arrivees: self.arrivees,
            e17: self.e17,
            gels: self.gels,
        }
    }

    /// E14 : depuis chaque point gardé, la reprise rejoue le journal entier et
    /// retrouve la même forme canonique, et la même empreinte qu'après la dernière
    /// écriture.
    fn reprises(&mut self, journal: &Journal, canonique: Option<&[u8]>) {
        let empreinte = self.obs.borrow().derniere_ecriture;
        let reference = self.obs.borrow().entrees.clone();
        for point in self.points.clone() {
            let obs = Obs::new(
                Verifs {
                    releve: Oui::Oui,
                    ..Verifs::RIEN
                },
                BUDGETS.lecture,
            );
            {
                let mut o = obs.borrow_mut();
                let p = partie_au(journal, point.indice());
                o.partie_reprise = p;
                o.prochaine_partie = p.saturating_add(1);
            }
            self.e14.evaluer();
            let pas = point.pas().numero();
            match Partie::reprendre(
                Sonde::new(self.module.fabrique(), &obs),
                self.ouverture.clone(),
                &point,
                journal,
            ) {
                Ok(mut r) => {
                    if let Some((p, o)) =
                        premiere_entree_differente(&reference, &obs.borrow().entrees)
                    {
                        self.e14.echouer(format!(
                            "la reprise depuis le pas {pas} diverge à l'entrée d'ordre {o} de la partie {p}"
                        ));
                    }
                    match (r.canonique(), canonique) {
                        (Ok(c), Some(attendue)) if c == attendue => {}
                        (Ok(_), Some(_)) => self.e14.echouer(format!(
                            "la reprise depuis le pas {pas} n'a pas la forme canonique de la partie"
                        )),
                        (Ok(_), None) => {}
                        (Err(e), _) => self.e14.echouer(format!(
                            "la reprise depuis le pas {pas} : forme canonique illisible : {e}"
                        )),
                    }
                    if let (Some(a), Some(b)) = (obs.borrow().derniere_ecriture, empreinte)
                        && a != b
                    {
                        self.e14.echouer(format!(
                            "la reprise depuis le pas {pas} n'a pas l'empreinte de la partie"
                        ));
                    }
                }
                Err(e) => self
                    .e14
                    .echouer(format!("la reprise depuis le pas {pas} échoue : {e:?}")),
            }
        }
    }
}

/// Un coup d'un bot, possédé.
#[derive(Debug, Clone)]
pub(crate) enum Coup {
    Geste(Vec<u8>),
    Tenu(Vec<u8>),
    Action(Vec<u8>),
    Touche(jeu_abi::Touche),
    Composant(ComposantArrivee),
    Rien,
}

impl Coup {
    fn de(c: &CoupFil<'_>) -> Coup {
        match c {
            CoupFil::Geste(o) => Coup::Geste(o.to_vec()),
            CoupFil::Tenu(o) => Coup::Tenu(o.to_vec()),
            CoupFil::Action(o) => Coup::Action(o.to_vec()),
            CoupFil::Touche(t) => Coup::Touche(*t),
            CoupFil::Composant(ComposantBotFil::Saisie { champ, texte, fin }) => {
                Coup::Composant(ComposantArrivee::Saisie {
                    champ: *champ,
                    texte: String::from(*texte),
                    fin: *fin,
                })
            }
            CoupFil::Composant(ComposantBotFil::Deplace(d)) => {
                Coup::Composant(ComposantArrivee::Deplace(*d))
            }
            // Les fichiers d'un bot passent par le sélecteur du commun, que la phase 1
            // n'a pas : le coup est ignoré, et cela se voit au rapport du jeu qui en
            // aurait besoin.
            CoupFil::Composant(ComposantBotFil::Fichiers(_)) | CoupFil::Rien => Coup::Rien,
        }
    }
}

/// Sert aux demandes : la forme possédée d'une demande, depuis son fil.
#[expect(dead_code, reason = "gardée pour les demandes d'images de la phase 2")]
fn demande(d: &DemandeFil<'_>) -> DemandeArrivee {
    DemandeArrivee::depuis_fil(d)
}

#[cfg(test)]
mod tests {
    use jeu_abi::ReveilFil;

    use super::echeance_vivante;

    /// CAT-78 : une échéance que le réveil n'atteint jamais, ou passée depuis
    /// longtemps, ne dispense pas E5 de juger l'attente.
    #[test]
    fn une_echeance_morte_ne_dispense_pas_e5() {
        assert!(echeance_vivante(100, 50, ReveilFil::A(100)));
        assert!(echeance_vivante(100, 50, ReveilFil::ChaquePas));
        assert!(echeance_vivante(100, 101, ReveilFil::A(100)));
        assert!(!echeance_vivante(100, 50, ReveilFil::Jamais));
        assert!(!echeance_vivante(100, 400, ReveilFil::ChaquePas));
    }
}
