//! Une soirée en mémoire (PHASE-3, § 8.1, lot 4b) : la vraie [`Autorite`] et de vrais
//! [`Appareil`]s, reliés par un relais en mémoire et un réseau simulé, à horloge simulée
//! qui saute d'échéance en échéance, sur une graine fixe (SAL-09). Rien n'y est réécrit
//! pour la simulation : ce sont les cœurs de la soirée qui tournent (SAL-10).
//!
//! - [`RelaisEnMemoire`] : un modèle du relais (`soiree-relais`, lot 5) sans entrées-
//!   sorties ni fils : il route les trames qu'il ne sait pas lire, tient la garde en
//!   mémoire, dit à l'autorité qui s'ouvre et qui se ferme, et aux appareils quand
//!   l'autorité part et revient (SAL-23). Une autorité plus récente remplace l'ancienne
//!   (P36). Il garde aussi l'historique de toutes les cases écrites, pour qu'un test
//!   vérifie la chaîne entière de la soirée.
//! - [`Soiree`] : le réseau (une latence par sens, l'ordre de chaque lien gardé, comme
//!   TCP), la page de l'écran réduite à son signal d'image et à des réponses aux
//!   demandes, et les pannes : coupure et retour d'un téléphone, rechargement, onglet
//!   jumeau, autorité tuée puis relancée depuis la garde, poignée de main rejouée par un
//!   invité.
//! - [`verifier_l_historique`] : la chaîne entière des cases écrites, et chaque geste
//!   compté au plus une fois dans le journal qu'elles portent (SAL-57).
//!
//! En mode banc, l'autorité calcule les coups des bots et les appareils les jouent par
//! le vrai chemin (`VersAppareil::Banc`). Le relais réel et ses WebSocket viennent au
//! lot 5 (`soiree-banc soiree-native`).

use std::collections::{BTreeMap, BTreeSet};

use jeu_abi::{Capacites, DemandeFil, EchecDemande, IdDemande, IdGeste, Oui, ReponseFil};
use liaison::{
    AdresseIp, Case, CauseFermeture, CodeSalle, DuRelais, Lien, RefusRelais, Role, VersRelais,
};
use partie::persistance::{InstantaneGarde, decoder_journal, decoder_point};
use partie::{Fabrique, Instance, IssueJournal, Ligne, LigneEntree, Partie, QuoiPlace};

use crate::appareil::Appareil;
use crate::autorite::{Autorite, Cles, Comptes, ReglagesSalle};
use crate::chiffre::{
    AVANT_LA_PREMIERE_CASE, CleDeGarde, ClePublique, EnteteCase, Fragment, Hasard, PaireAutorite,
    SecretDEntree, empreinte_case, ouvrir_case,
};
use crate::ecran::{DeLaPage, VersPage};
use crate::garde::{Registre, decoder_contenu_journal, decoder_contenu_point, numero};

/// Un pas de 1/60 s, en microsecondes, arrondi : la cadence des images de la page.
pub const PAS_US: u64 = 16_667;

// ---------------------------------------------------------------------------
// Le hasard à graine fixe
// ---------------------------------------------------------------------------

/// Un hasard à graine fixe : BLAKE3 en mode extensible (SAL-09).
pub struct Graine(blake3::OutputReader);

impl Graine {
    /// Le hasard de cette graine, pour cet usage.
    pub fn new(usage: &str, graine: u64) -> Graine {
        let mut h = blake3::Hasher::new();
        h.update(b"soiree graine de banc ");
        h.update(usage.as_bytes());
        h.update(&graine.to_le_bytes());
        Graine(h.finalize_xof())
    }

    /// Un entier.
    pub fn entier(&mut self) -> u64 {
        let mut o = [0u8; 8];
        self.0.fill(&mut o);
        u64::from_le_bytes(o)
    }

    /// Un entier dans `[0, n)` ; 0 si `n` est nul.
    pub fn sous(&mut self, n: u64) -> u64 {
        self.entier().checked_rem(n).unwrap_or(0)
    }

    /// Un entier dans `[a, b]`.
    pub fn entre(&mut self, a: u64, b: u64) -> u64 {
        a.saturating_add(self.sous(b.saturating_sub(a).saturating_add(1)))
    }
}

impl Hasard for Graine {
    fn remplir(&mut self, octets: &mut [u8]) {
        self.0.fill(octets);
    }
}

// ---------------------------------------------------------------------------
// Le relais en mémoire
// ---------------------------------------------------------------------------

/// Ce qu'une connexion est pour le relais.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoleRelais {
    Inconnu,
    Autorite,
    Garde,
    Appareil,
    /// Une autorité ou une garde remplacée : plus rien d'elle ne compte (SAL-20).
    Passee,
}

/// Un modèle du relais, en mémoire (voir le module).
#[derive(Debug, Default)]
pub struct RelaisEnMemoire {
    salle: Option<CodeSalle>,
    prochain: u32,
    roles: BTreeMap<Lien, RoleRelais>,
    autorite: Option<Lien>,
    garde: Option<Lien>,
    preuve: Option<[u8; 32]>,
    cases: BTreeMap<u64, (Case, Vec<u8>)>,
    historique: BTreeMap<u64, (Case, Vec<u8>)>,
    sorties: Vec<(Lien, Vec<u8>)>,
    fermes: Vec<Lien>,
}

impl RelaisEnMemoire {
    /// Un relais neuf.
    pub fn new() -> RelaisEnMemoire {
        RelaisEnMemoire::default()
    }

    /// Une connexion s'ouvre : son lien, numéroté dans l'ordre d'ouverture (SAL-19).
    pub fn ouvrir(&mut self) -> Lien {
        self.prochain = self.prochain.saturating_add(1);
        let l = Lien(self.prochain);
        self.roles.insert(l, RoleRelais::Inconnu);
        l
    }

    /// Ce que le relais envoie, à quel lien.
    pub fn sorties(&mut self) -> Vec<(Lien, Vec<u8>)> {
        core::mem::take(&mut self.sorties)
    }

    /// Les connexions que le relais a fermées de lui-même (`Couper`, refus).
    pub fn fermes(&mut self) -> Vec<Lien> {
        core::mem::take(&mut self.fermes)
    }

    /// La garde, telle qu'elle est maintenant.
    pub fn cases(&self) -> Vec<(Case, Vec<u8>)> {
        self.cases.values().cloned().collect()
    }

    /// Toutes les cases écrites dans la soirée, effacées comprises, par numéro.
    pub fn historique(&self) -> Vec<(Case, Vec<u8>)> {
        self.historique.values().cloned().collect()
    }

    /// Le lien de l'autorité, s'il y en a une.
    pub const fn autorite(&self) -> Option<Lien> {
        self.autorite
    }

    fn envoyer(&mut self, a: Lien, m: &DuRelais<'_>) {
        if let Ok(o) = m.encoder() {
            self.sorties.push((a, o));
        }
    }

    fn appareils(&self) -> Vec<Lien> {
        self.roles
            .iter()
            .filter(|(_, r)| **r == RoleRelais::Appareil)
            .map(|(l, _)| *l)
            .collect()
    }

    fn accueillir(&mut self, lien: Lien) {
        self.envoyer(
            lien,
            &DuRelais::Accueil {
                epoque: [1; 16],
                lien,
                adresses: Vec::new(),
            },
        );
    }

    fn refuser(&mut self, lien: Lien, r: RefusRelais) {
        self.envoyer(lien, &DuRelais::Refus(r));
        self.roles.insert(lien, RoleRelais::Passee);
        self.fermes.push(lien);
    }

    /// Une trame d'un bout, sur sa connexion.
    #[expect(
        clippy::too_many_lines,
        reason = "chaque trame d'un bout se traite ici, d'un seul match"
    )]
    pub fn recevoir(&mut self, de: Lien, t_us: u64, octets: &[u8]) {
        let role = self.roles.get(&de).copied().unwrap_or(RoleRelais::Passee);
        let Ok(m) = VersRelais::decoder(octets) else {
            return;
        };
        match (m, role) {
            (VersRelais::Bonjour { salle, role: r, .. }, RoleRelais::Inconnu) => {
                if self.salle.is_some_and(|s| s != salle) {
                    return self.refuser(de, RefusRelais::SalleInconnue);
                }
                match r {
                    Role::Autorite { preuve } => {
                        if self.preuve.is_some_and(|p| p != preuve) {
                            return self.refuser(de, RefusRelais::PreuveFausse);
                        }
                        self.salle = Some(salle);
                        self.preuve = Some(preuve);
                        // La plus récente gagne (P36).
                        for ancien in [self.autorite, self.garde].into_iter().flatten() {
                            self.refuser(ancien, RefusRelais::Remplacee);
                        }
                        self.garde = None;
                        self.autorite = Some(de);
                        self.roles.insert(de, RoleRelais::Autorite);
                        self.accueillir(de);
                        for l in self.appareils() {
                            self.envoyer(
                                de,
                                &DuRelais::Ouvert {
                                    lien: l,
                                    adresse_jointe: AdresseIp::V4([127, 0, 0, 1]),
                                },
                            );
                            self.envoyer(l, &DuRelais::AutoritePresente);
                        }
                    }
                    Role::Garde { preuve, autorite } => {
                        if self.preuve != Some(preuve) || self.autorite != Some(autorite) {
                            return self.refuser(de, RefusRelais::Remplacee);
                        }
                        self.garde = Some(de);
                        self.roles.insert(de, RoleRelais::Garde);
                        self.accueillir(de);
                    }
                    Role::Appareil => {
                        if self.salle.is_none_or(|s| s != salle) {
                            return self.refuser(de, RefusRelais::SalleInconnue);
                        }
                        self.roles.insert(de, RoleRelais::Appareil);
                        self.accueillir(de);
                        match self.autorite {
                            Some(a) => {
                                self.envoyer(
                                    a,
                                    &DuRelais::Ouvert {
                                        lien: de,
                                        adresse_jointe: AdresseIp::V4([127, 0, 0, 1]),
                                    },
                                );
                                self.envoyer(de, &DuRelais::AutoritePresente);
                            }
                            None => self.envoyer(de, &DuRelais::AutoriteAbsente),
                        }
                    }
                }
            }
            (VersRelais::Trame { classe, trame }, RoleRelais::Appareil) => {
                if let Some(a) = self.autorite {
                    self.envoyer(
                        a,
                        &DuRelais::De {
                            lien: de,
                            classe,
                            trame,
                            recu_us: t_us,
                            renvoye_us: t_us,
                        },
                    );
                }
            }
            (
                VersRelais::Vers {
                    lien,
                    classe,
                    trame,
                },
                RoleRelais::Autorite,
            ) => {
                if self.roles.get(&lien) == Some(&RoleRelais::Appareil) {
                    self.envoyer(lien, &DuRelais::Pour { classe, trame });
                }
            }
            (VersRelais::Couper { lien, .. }, RoleRelais::Autorite) => {
                if self.roles.get(&lien) == Some(&RoleRelais::Appareil) {
                    let _: Option<RoleRelais> = self.roles.remove(&lien);
                    self.fermes.push(lien);
                    self.envoyer(
                        de,
                        &DuRelais::Ferme {
                            lien,
                            cause: CauseFermeture::Coupe,
                        },
                    );
                }
            }
            (VersRelais::Echo { t_envoi_us }, _) => self.envoyer(
                de,
                &DuRelais::Echo {
                    t_envoi_us,
                    t_relais_us: t_us,
                },
            ),
            (VersRelais::AuRevoir, _) => self.fermer(de),
            (VersRelais::Ecrire { case, octets }, RoleRelais::Garde) => {
                self.cases.insert(numero(case), (case, octets.to_vec()));
                self.historique
                    .insert(numero(case), (case, octets.to_vec()));
                self.envoyer(de, &DuRelais::Ecrit { case });
            }
            (VersRelais::Lister, RoleRelais::Garde) => {
                let cases = self.cases.values().map(|(c, _)| *c).collect();
                self.envoyer(de, &DuRelais::Cases(cases));
            }
            (VersRelais::Lire { case }, RoleRelais::Garde) => {
                if let Some((c, o)) = self.cases.get(&numero(case)).cloned()
                    && c == case
                {
                    self.envoyer(de, &DuRelais::Lu { case, octets: &o });
                }
            }
            (VersRelais::Effacer { avant }, RoleRelais::Garde) => {
                self.cases.retain(|&n, _| n >= avant);
            }
            // Une garde passée n'écrit plus (SAL-20) ; le reste ne vient pas de ce rôle.
            (
                VersRelais::Bonjour { .. }
                | VersRelais::Trame { .. }
                | VersRelais::Vers { .. }
                | VersRelais::Couper { .. }
                | VersRelais::Ecrire { .. }
                | VersRelais::Lister
                | VersRelais::Lire { .. }
                | VersRelais::Effacer { .. }
                | VersRelais::AjouterLigne { .. }
                | VersRelais::Limites { .. },
                _,
            ) => {}
        }
    }

    /// Une connexion se ferme.
    pub fn fermer(&mut self, lien: Lien) {
        let role = self.roles.remove(&lien);
        match role {
            Some(RoleRelais::Appareil) => {
                if let Some(a) = self.autorite {
                    self.envoyer(
                        a,
                        &DuRelais::Ferme {
                            lien,
                            cause: CauseFermeture::Fermee,
                        },
                    );
                }
            }
            Some(RoleRelais::Autorite) => {
                self.autorite = None;
                for l in self.appareils() {
                    self.envoyer(l, &DuRelais::AutoriteAbsente);
                }
            }
            Some(RoleRelais::Garde) => self.garde = None,
            Some(RoleRelais::Inconnu | RoleRelais::Passee) | None => {}
        }
    }
}

// ---------------------------------------------------------------------------
// La soirée
// ---------------------------------------------------------------------------

/// Les clés de la soirée, en octets : de quoi relancer une autorité avec les mêmes.
#[derive(Clone)]
struct ClesBrutes {
    privee: [u8; 32],
    secret: [u8; 16],
    garde: [u8; 32],
}

impl ClesBrutes {
    fn cles(&self) -> Option<Cles> {
        Some(Cles {
            paire: PaireAutorite::depuis_privee(self.privee).ok()?,
            secret: SecretDEntree::depuis_octets(self.secret),
            garde: CleDeGarde::depuis_octets(self.garde),
        })
    }
}

/// Le sens d'une trame en route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Sens {
    /// D'un bout vers le relais.
    VersRelais,
    /// Du relais vers un bout.
    DuRelais,
}

/// Ce que la soirée relie à une connexion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bout {
    TempsReel,
    Garde,
    Telephone(usize),
    Invite,
}

/// Un téléphone de la soirée.
pub struct Telephone {
    /// Son cœur.
    pub appareil: Appareil<Graine>,
    /// Sa connexion au relais.
    pub lien: Option<Lien>,
    /// L'écart de son horloge sur celle de l'ordinateur, en microsecondes.
    decalage_us: u64,
    /// Quand il revient, s'il est coupé.
    revient_us: Option<u64>,
    /// Quand il part, s'il est un onglet jumeau.
    part_us: Option<u64>,
    /// Le premier message de poignée de main qu'il a envoyé : ce qu'un invité qui écoute
    /// le Wi-Fi capte (SAL-17).
    pub premiere_poignee: Option<Vec<u8>>,
    /// Son jeton, pour retrouver son appareil.
    pub jeton: [u8; 32],
}

/// Ce que la soirée a fait et constaté.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bilan {
    /// Les pas joués par l'horloge.
    pub pas: u64,
    /// Les autorités tuées puis relancées depuis la garde.
    pub redemarrages: u32,
    /// Les coupures d'un téléphone.
    pub coupures: u32,
    /// Les rechargements d'un téléphone.
    pub rechargements: u32,
    /// Les onglets jumeaux ouverts.
    pub jumeaux: u32,
    /// Les poignées de main rejouées par un invité.
    pub poignees_rejouees: u32,
    /// Les gestes émis par les téléphones.
    pub gestes_emis: u64,
    /// Les gestes accusés.
    pub gestes_accuses: u64,
    /// Ce que les autorités successives ont compté, additionné.
    pub autorite: ComptesSomme,
    /// Les demandes de la page auxquelles la soirée a répondu.
    pub demandes: u64,
    /// L'arrêt d'une autorité, s'il y en a eu un.
    pub arret: Option<String>,
}

/// Les comptes des autorités, additionnés.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComptesSomme {
    /// Les gestes placés.
    pub gestes_places: u64,
    /// Les gestes reçus de nouveau, ignorés.
    pub gestes_en_double: u64,
    /// Les gestes hors de la suite attendue.
    pub gestes_hors_suite: u64,
    /// Les gestes que la partie n'a pas placés.
    pub gestes_refuses: u64,
    /// Les cases de journal.
    pub cases_journal: u64,
    /// Les points.
    pub points: u64,
    /// Les liens remplacés.
    pub remplaces: u64,
    /// Les trames illisibles.
    pub trames_illisibles: u64,
    /// Les poignées de main inachevées.
    pub poignees_inachevees: u64,
    /// Les vues envoyées.
    pub vues: u64,
    /// Les coups du banc envoyés.
    pub coups_banc: u64,
    /// Les coups du bot de l'écran entrés.
    pub coups_ecran: u64,
}

impl ComptesSomme {
    fn ajouter(&mut self, c: Comptes) {
        self.gestes_places = self.gestes_places.saturating_add(c.gestes_places);
        self.gestes_en_double = self.gestes_en_double.saturating_add(c.gestes_en_double);
        self.gestes_hors_suite = self.gestes_hors_suite.saturating_add(c.gestes_hors_suite);
        self.gestes_refuses = self.gestes_refuses.saturating_add(c.gestes_refuses);
        self.cases_journal = self.cases_journal.saturating_add(c.cases_journal);
        self.points = self.points.saturating_add(c.points);
        self.remplaces = self.remplaces.saturating_add(c.remplaces);
        self.trames_illisibles = self.trames_illisibles.saturating_add(c.trames_illisibles);
        self.poignees_inachevees = self
            .poignees_inachevees
            .saturating_add(c.poignees_inachevees);
        self.vues = self.vues.saturating_add(c.vues);
        self.coups_banc = self.coups_banc.saturating_add(c.coups_banc);
        self.coups_ecran = self.coups_ecran.saturating_add(c.coups_ecran);
    }
}

/// Les pannes qu'une soirée tire de sa graine, et la latence du réseau.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pannes {
    /// La durée moyenne entre deux pannes, en microsecondes ; aucune si nulle.
    pub tous_les_us: u64,
    /// Les autorités tuées en font-elles partie ?
    pub autorite: Oui,
    /// La latence d'un téléphone au relais, dans un sens, en microsecondes : de, à.
    pub latence_us: (u64, u64),
    /// Le temps que le relais met à traiter ce que la garde lui envoie (écrire une case,
    /// puis répondre `Ecrit` aussitôt), en microsecondes : de, à. La case n'est écrite
    /// qu'au bout de ce temps. Non mesuré (M11, lot 5) : quelques ms sont plausibles
    /// sur un SSD (PHASE-3, § 2.4).
    pub garde_us: (u64, u64),
}

impl Pannes {
    /// Aucune panne, une latence nulle : pour les tests qui mènent la soirée à la main.
    pub const AUCUNE: Pannes = Pannes {
        tous_les_us: 0,
        autorite: Oui::Non,
        latence_us: (0, 0),
        garde_us: (0, 0),
    };

    /// Les pannes d'une soirée éprouvée : une toutes les 4 s en moyenne, autorité
    /// comprise ; la latence du monde simulé de l'épreuve, de 16 à 66 ms par sens ; une
    /// case de garde écrite en 1 à 20 ms (choisi, en attendant M11).
    pub const SOIREE: Pannes = Pannes {
        tous_les_us: 4_000_000,
        autorite: Oui::Oui,
        latence_us: (16_000, 66_000),
        garde_us: (1_000, 20_000),
    };
}

/// Une soirée en mémoire, générique sur la fabrique des instances.
pub struct Soiree<F: Fabrique, M: FnMut() -> F> {
    fabriquer: M,
    reglages: ReglagesSalle,
    cles: ClesBrutes,
    fragment_secret: [u8; 16],
    publique: ClePublique,
    /// Le relais.
    pub relais: RelaisEnMemoire,
    /// L'autorité, quand elle vit.
    pub autorite: Option<Autorite<F, Graine>>,
    liens_autorite: Option<(Lien, Lien)>,
    relance_us: Option<u64>,
    relance_dernier: Option<u64>,
    /// Les téléphones.
    pub telephones: Vec<Telephone>,
    bouts: BTreeMap<Lien, Bout>,
    /// Les trames en route : (instant d'arrivée, numéro) → (sens, lien, octets).
    en_route: BTreeMap<(u64, u64), (Sens, Lien, Vec<u8>)>,
    dernier_par_sens: BTreeMap<(Sens, Lien), u64>,
    sequence: u64,
    hasard: Graine,
    pannes: Pannes,
    prochaine_panne_us: u64,
    redite_us: Option<u64>,
    /// L'heure simulée de l'ordinateur, en microsecondes.
    pub t_us: u64,
    prochaine_image_us: u64,
    debut_us: u64,
    graine: u64,
    relances: u64,
    /// Ce que la soirée a fait et constaté.
    pub bilan: Bilan,
}

/// L'heure de l'ordinateur au début d'une soirée simulée : le 2023-11-14, comme
/// l'ouverture de l'épreuve.
pub const DEBUT_US: u64 = 1_700_000_000_000_000;

impl<F, M> Soiree<F, M>
where
    F: Fabrique,
    <F::I as Instance>::Instantane: InstantaneGarde + PartialEq + Clone,
    M: FnMut() -> F,
{
    /// Une soirée de cette graine : une autorité qui démarre, aucun téléphone.
    pub fn new(fabriquer: M, graine: u64, banc: Oui, pannes: Pannes) -> Soiree<F, M> {
        let mut hasard = Graine::new("soiree", graine);
        let mut cles_hasard = Graine::new("cles", graine);
        let paire = PaireAutorite::tirer(&mut cles_hasard).ok();
        let secret = SecretDEntree::tirer(&mut cles_hasard);
        let garde = CleDeGarde::tirer(&mut cles_hasard);
        let privee = paire.as_ref().map_or([0; 32], |p| *p.privee());
        let publique = paire
            .as_ref()
            .map_or(ClePublique([0; 32]), PaireAutorite::publique);
        let mut salle_octets = [0u8; 4];
        hasard.remplir(&mut salle_octets);
        let mut reglages = ReglagesSalle::soiree(CodeSalle(salle_octets));
        reglages.banc = banc;
        let premiere_panne = if pannes.tous_les_us == 0 {
            u64::MAX
        } else {
            DEBUT_US.saturating_add(hasard.entre(pannes.tous_les_us / 2, pannes.tous_les_us))
        };
        let mut s = Soiree {
            fabriquer,
            reglages,
            cles: ClesBrutes {
                privee,
                secret: *secret.octets(),
                garde: *garde.octets(),
            },
            fragment_secret: *secret.octets(),
            publique,
            relais: RelaisEnMemoire::new(),
            autorite: None,
            liens_autorite: None,
            relance_us: None,
            relance_dernier: None,
            telephones: Vec::new(),
            bouts: BTreeMap::new(),
            en_route: BTreeMap::new(),
            dernier_par_sens: BTreeMap::new(),
            sequence: 0,
            hasard,
            pannes,
            prochaine_panne_us: premiere_panne,
            redite_us: None,
            t_us: DEBUT_US,
            prochaine_image_us: DEBUT_US,
            debut_us: DEBUT_US,
            graine,
            relances: 0,
            bilan: Bilan::default(),
        };
        s.lancer_l_autorite(None);
        s
    }

    /// La période de redite des tenus des téléphones à venir (P27).
    pub const fn avec_redite(&mut self, redite_us: Option<u64>) {
        self.redite_us = redite_us;
    }

    /// Le code de salle.
    pub const fn salle(&self) -> CodeSalle {
        self.reglages.salle
    }

    /// La clé de garde G : pour relire la garde dans un test.
    pub fn cle_de_garde(&self) -> CleDeGarde {
        CleDeGarde::depuis_octets(self.cles.garde)
    }

    /// Le fragment du QR.
    pub fn fragment(&self) -> Fragment {
        Fragment {
            secret: SecretDEntree::depuis_octets(self.fragment_secret),
            autorite: self.publique,
        }
    }

    fn lancer_l_autorite(&mut self, dernier_n: Option<u64>) {
        let Some(cles) = self.cles.cles() else {
            return;
        };
        self.relances = self.relances.saturating_add(1);
        let hasard = Graine::new("autorite", self.graine.wrapping_add(self.relances));
        let tr = self.relais.ouvrir();
        let garde = self.relais.ouvrir();
        self.bouts.insert(tr, Bout::TempsReel);
        self.bouts.insert(garde, Bout::Garde);
        self.liens_autorite = Some((tr, garde));
        let a = Autorite::demarrer((self.fabriquer)(), cles, self.reglages, hasard, dernier_n);
        self.autorite = Some(a);
        self.vider_l_autorite();
    }

    /// Tue l'autorité, sans rien lui laisser finir : ce qu'elle n'a pas envoyé est perdu.
    /// Elle se relance depuis la garde `apres_us` plus tard.
    pub fn tuer_l_autorite(&mut self, apres_us: u64) {
        let Some(a) = self.autorite.take() else {
            return;
        };
        self.bilan.autorite.ajouter(a.comptes());
        let dernier = a.dernier_ecrit();
        if let Some((tr, garde)) = self.liens_autorite.take() {
            // Ce qui part encore d'elle sur le temps réel ne part pas. Ce qu'elle a déjà
            // donné à la garde, le relais l'écrit jusqu'au bout : une case peut être
            // écrite sans que l'autorité ait reçu son `Ecrit`. La garde ne se ferme
            // qu'après, dans l'ordre de son lien.
            self.en_route
                .retain(|_, (sens, l, _)| !(*sens == Sens::VersRelais && *l == tr));
            self.relais.fermer(tr);
            let fin = self
                .dernier_par_sens
                .get(&(Sens::VersRelais, garde))
                .copied()
                .unwrap_or(0)
                .max(self.t_us);
            self.sequence = self.sequence.saturating_add(1);
            self.en_route
                .insert((fin, self.sequence), (Sens::VersRelais, garde, Vec::new()));
            let _: Option<Bout> = self.bouts.remove(&tr);
            let _: Option<Bout> = self.bouts.remove(&garde);
        }
        self.relance_dernier = dernier;
        self.relance_us = Some(self.t_us.saturating_add(apres_us));
        self.bilan.redemarrages = self.bilan.redemarrages.saturating_add(1);
    }

    /// L'autorité vit-elle ?
    pub const fn autorite_vit(&self) -> bool {
        self.autorite.is_some()
    }

    /// Ajoute un téléphone, qui se relie tout de suite. Rend son rang.
    pub fn ajouter_telephone(&mut self) -> usize {
        let k = self.telephones.len();
        let mut h = Graine::new(
            "telephone",
            self.graine.wrapping_mul(1000).wrapping_add(k as u64),
        );
        let mut jeton = [0u8; 32];
        h.remplir(&mut jeton);
        let onglet = h.entier();
        let decalage_us = h.sous(60_000_000);
        let fragment = self.fragment();
        let a = Appareil::new(
            fragment,
            self.reglages.salle,
            (jeton, onglet),
            Capacites::AUCUNE,
            self.redite_us,
            h,
        );
        self.telephones.push(Telephone {
            appareil: a,
            lien: None,
            decalage_us,
            revient_us: None,
            part_us: None,
            premiere_poignee: None,
            jeton,
        });
        self.relier(k);
        k
    }

    /// Un onglet jumeau du téléphone `k` : même jeton, un autre onglet. Rend son rang.
    pub fn jumeau(&mut self, k: usize, pendant_us: Option<u64>) -> Option<usize> {
        let jeton = self.telephones.get(k)?.jeton;
        let n = self.telephones.len();
        let mut h = Graine::new(
            "jumeau",
            self.graine.wrapping_mul(1000).wrapping_add(n as u64),
        );
        let onglet = h.entier();
        let a = Appareil::new(
            self.fragment(),
            self.reglages.salle,
            (jeton, onglet),
            Capacites::AUCUNE,
            self.redite_us,
            h,
        );
        self.telephones.push(Telephone {
            appareil: a,
            lien: None,
            decalage_us: 0,
            revient_us: None,
            part_us: pendant_us.map(|d| self.t_us.saturating_add(d)),
            premiere_poignee: None,
            jeton,
        });
        self.bilan.jumeaux = self.bilan.jumeaux.saturating_add(1);
        self.relier(n);
        Some(n)
    }

    fn relier(&mut self, k: usize) {
        let l = self.relais.ouvrir();
        self.bouts.insert(l, Bout::Telephone(k));
        if let Some(t) = self.telephones.get_mut(k) {
            t.lien = Some(l);
            t.appareil.relier();
        }
        self.vider_le_telephone(k);
    }

    /// Coupe le téléphone `k` ; il revient `apres_us` plus tard, si c'est dit.
    pub fn couper(&mut self, k: usize, apres_us: Option<u64>) {
        let Some(t) = self.telephones.get_mut(k) else {
            return;
        };
        t.appareil.couper();
        t.revient_us = apres_us.map(|d| self.t_us.saturating_add(d));
        if let Some(l) = t.lien.take() {
            self.en_route.retain(|_, (_, x, _)| *x != l);
            let _: Option<Bout> = self.bouts.remove(&l);
            self.relais.fermer(l);
        }
        self.vider_le_relais();
    }

    /// Recharge la page du téléphone `k` : même jeton, même onglet, mêmes gestes en file.
    pub fn recharger(&mut self, k: usize, apres_us: u64) {
        self.couper(k, Some(apres_us));
        if let Some(t) = self.telephones.get_mut(k) {
            t.appareil.recharger();
        }
        self.bilan.rechargements = self.bilan.rechargements.saturating_add(1);
    }

    /// Le téléphone `k` reprend sa place après un remplacement (« Reprendre ici ») : un
    /// lien neuf.
    pub fn reprendre_ici(&mut self, k: usize) {
        let Some(t) = self.telephones.get_mut(k) else {
            return;
        };
        t.appareil.reprendre_ici();
        t.revient_us = None;
        if let Some(l) = t.lien.take() {
            let _: Option<Bout> = self.bouts.remove(&l);
            self.relais.fermer(l);
            self.vider_le_relais();
        }
        self.relier(k);
    }

    /// Un invité qui a capté la première poignée de main du téléphone `k` la renvoie,
    /// sur un lien à lui (SAL-17). Rend son lien.
    pub fn rejouer_la_poignee(&mut self, k: usize) -> Option<Lien> {
        let premiere = self.telephones.get(k)?.premiere_poignee.clone()?;
        let l = self.relais.ouvrir();
        self.bouts.insert(l, Bout::Invite);
        let salle = self.reglages.salle;
        for m in [
            VersRelais::Bonjour {
                protocole: liaison::PROTOCOLE_RELAIS,
                salle,
                role: Role::Appareil,
            },
            VersRelais::Trame {
                classe: liaison::Classe::Fiable,
                trame: &premiere,
            },
        ] {
            if let Ok(o) = m.encoder() {
                self.envoyer(Sens::VersRelais, l, o);
            }
        }
        self.bilan.poignees_rejouees = self.bilan.poignees_rejouees.saturating_add(1);
        Some(l)
    }

    /// L'heure de l'horloge du téléphone `k` maintenant.
    pub fn heure_du_telephone(&self, k: usize) -> u64 {
        self.telephones
            .get(k)
            .map_or(self.t_us, |t| self.t_us.saturating_add(t.decalage_us))
    }

    /// Envoie ce que le téléphone `k` a à envoyer : après un appel fait à la main sur
    /// son appareil.
    pub fn pousser(&mut self, k: usize) {
        self.vider_le_telephone(k);
    }

    /// Retient les trames du lien du téléphone `k`, dans un sens, jusqu'à `t_us` : un
    /// message coincé dans la connexion (SAL-19), ou une connexion qui ne livre plus.
    pub fn retenir(&mut self, k: usize, vers_le_relais: Oui, jusqu_a_us: u64) {
        let Some(l) = self.telephones.get(k).and_then(|t| t.lien) else {
            return;
        };
        let sens = match vers_le_relais {
            Oui::Oui => Sens::VersRelais,
            Oui::Non => Sens::DuRelais,
        };
        let d = self.dernier_par_sens.entry((sens, l)).or_insert(0);
        *d = (*d).max(jusqu_a_us);
        // Ce qui était déjà en route attend aussi.
        let a_retenir: Vec<(u64, u64)> = self
            .en_route
            .iter()
            .filter(|(_, (s, x, _))| *s == sens && *x == l)
            .map(|(k, _)| *k)
            .filter(|(t, _)| *t < jusqu_a_us)
            .collect();
        for cle in a_retenir {
            if let Some(v) = self.en_route.remove(&cle) {
                self.en_route.insert((jusqu_a_us, cle.1), v);
            }
        }
    }

    /// Une seconde tentative du même téléphone, sur un lien neuf : même jeton, même
    /// onglet, comme une page qui retente sa connexion pendant que la première reste
    /// coincée (SAL-19). Rend son rang.
    pub fn retenter(&mut self, k: usize) -> Option<usize> {
        let tel = self.telephones.get(k)?;
        let (jeton, onglet) = (tel.jeton, tel.appareil.onglet());
        let n = self.telephones.len();
        let h = Graine::new(
            "tentative",
            self.graine.wrapping_mul(1000).wrapping_add(n as u64),
        );
        let a = Appareil::new(
            self.fragment(),
            self.reglages.salle,
            (jeton, onglet),
            Capacites::AUCUNE,
            self.redite_us,
            h,
        );
        self.telephones.push(Telephone {
            appareil: a,
            lien: None,
            decalage_us: 0,
            revient_us: None,
            part_us: None,
            premiere_poignee: None,
            jeton,
        });
        self.relier(n);
        Some(n)
    }

    /// Les téléphones ne jouent plus les coups du banc : la fin de la soirée, où tout
    /// ce qui attend doit être accusé.
    pub fn faire_taire(&mut self) {
        for t in &mut self.telephones {
            t.appareil.taire_le_banc();
        }
    }

    // -----------------------------------------------------------------------
    // Le réseau
    // -----------------------------------------------------------------------

    fn latence(&mut self, sens: Sens, l: Lien) -> u64 {
        match (self.bouts.get(&l), sens) {
            (Some(Bout::Telephone(_) | Bout::Invite), _) => {
                let (a, b) = self.pannes.latence_us;
                self.hasard.entre(a, b)
            }
            // Le relais traite ce que la garde lui demande, dans l'ordre, au bout du
            // temps d'écrire : il écrit la case, puis répond `Ecrit` aussitôt. Une case
            // n'est donc dans la garde (et dans l'historique) qu'au moment de son `Ecrit`
            // (SAL-56).
            (Some(Bout::Garde), Sens::VersRelais) => {
                let (a, b) = self.pannes.garde_us;
                self.hasard.entre(a, b)
            }
            (Some(Bout::TempsReel | Bout::Garde) | None, _) => 0,
        }
    }

    /// Met une trame en route : vers le relais depuis le bout `l`, ou du relais vers lui.
    /// L'ordre de chaque sens d'un lien est gardé, comme sur TCP.
    fn envoyer(&mut self, sens: Sens, l: Lien, octets: Vec<u8>) {
        let lat = self.latence(sens, l);
        let mut arrivee = self.t_us.saturating_add(lat);
        let dernier = self.dernier_par_sens.entry((sens, l)).or_insert(0);
        arrivee = arrivee.max(*dernier);
        *dernier = arrivee;
        self.sequence = self.sequence.saturating_add(1);
        self.en_route
            .insert((arrivee, self.sequence), (sens, l, octets));
    }

    fn vider_l_autorite(&mut self) {
        let (Some(a), Some((tr, garde))) = (self.autorite.as_mut(), self.liens_autorite) else {
            return;
        };
        let s = a.sorties();
        for o in s.temps_reel {
            self.envoyer(Sens::VersRelais, tr, o);
        }
        for o in s.garde {
            self.envoyer(Sens::VersRelais, garde, o);
        }
        for o in s.page {
            self.page(&o);
        }
    }

    fn vider_le_telephone(&mut self, k: usize) {
        let Some(t) = self.telephones.get_mut(k) else {
            return;
        };
        let Some(l) = t.lien else {
            let _: Vec<Vec<u8>> = t.appareil.sorties();
            return;
        };
        let sorties = t.appareil.sorties();
        for o in sorties {
            if let Some(t) = self.telephones.get_mut(k)
                && t.premiere_poignee.is_none()
                && let Ok(VersRelais::Trame { trame, .. }) = VersRelais::decoder(&o)
                && trame.len() == crate::chiffre::TAILLE_POIGNEE
            {
                t.premiere_poignee = Some(trame.to_vec());
            }
            self.envoyer(Sens::VersRelais, l, o);
        }
    }

    fn vider_le_relais(&mut self) {
        for (l, o) in self.relais.sorties() {
            self.envoyer(Sens::DuRelais, l, o);
        }
        for l in self.relais.fermes() {
            // Le relais a fermé ce lien : le bout l'apprend après ce qui était en route
            // vers lui (le refus), par une trame vide.
            if matches!(self.bouts.get(&l), Some(Bout::Telephone(_))) {
                self.envoyer(Sens::DuRelais, l, Vec::new());
            }
        }
    }

    /// La page de l'écran : elle répond à chaque demande, faute de quoi la servir (les
    /// fichiers, le rendu et les images viennent avec l'écran, lots 9 à 11).
    fn page(&mut self, octets: &[u8]) {
        let Ok(VersPage::Demande { id, quoi }) = VersPage::decoder(octets) else {
            return;
        };
        self.bilan.demandes = self.bilan.demandes.saturating_add(1);
        let quoi = reponse_par_defaut(&quoi);
        let m = DeLaPage::Reponse { id, quoi };
        if let (Ok(o), Some(a)) = (m.encoder(), self.autorite.as_mut()) {
            a.page(self.t_us, &o);
        }
        let _: IdDemande = id;
    }

    fn livrer(&mut self, sens: Sens, l: Lien, octets: &[u8]) {
        if sens == Sens::VersRelais {
            // Une trame vide vers le relais : la connexion se ferme (la garde d'une
            // autorité tuée, après ses écritures).
            if octets.is_empty() {
                self.relais.fermer(l);
            } else {
                self.relais.recevoir(l, self.t_us, octets);
            }
            self.vider_le_relais();
            return;
        }
        match self.bouts.get(&l).copied() {
            Some(Bout::TempsReel) => {
                if let Some(a) = self.autorite.as_mut() {
                    a.temps_reel(self.t_us, octets);
                }
                self.vider_l_autorite();
            }
            Some(Bout::Garde) => {
                if let Some(a) = self.autorite.as_mut() {
                    a.garde(self.t_us, octets);
                }
                self.vider_l_autorite();
            }
            Some(Bout::Telephone(k)) => {
                let Some(t) = self.telephones.get_mut(k) else {
                    return;
                };
                if t.lien != Some(l) {
                    return;
                }
                if octets.is_empty() {
                    // Le relais a fermé ce lien, après ce qu'il y envoyait encore.
                    t.lien = None;
                    t.appareil.couper();
                    let _: Option<Bout> = self.bouts.remove(&l);
                    if !t.appareil.fini() && t.revient_us.is_none() && t.part_us.is_none() {
                        t.revient_us = Some(self.t_us.saturating_add(1_000_000));
                    }
                    return;
                }
                let local = self.t_us.saturating_add(t.decalage_us);
                t.appareil.recevoir(local, octets);
                self.vider_le_telephone(k);
            }
            Some(Bout::Invite) | None => {}
        }
    }

    // -----------------------------------------------------------------------
    // Le temps
    // -----------------------------------------------------------------------

    /// La soirée avance jusqu'à `fin_us`, d'échéance en échéance : les trames qui
    /// arrivent, les images de la page, les réveils des téléphones, les pannes.
    pub fn avancer_jusqua(&mut self, fin_us: u64) {
        loop {
            let trame = self.en_route.keys().next().map(|(t, _)| *t);
            let prochaine = [
                trame,
                Some(self.prochaine_image_us),
                self.relance_us,
                Some(self.prochaine_panne_us),
            ]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(u64::MAX);
            if prochaine > fin_us {
                self.t_us = self.t_us.max(fin_us);
                return;
            }
            self.t_us = self.t_us.max(prochaine);
            if trame == Some(prochaine) {
                if let Some(((t, n), _)) = self.en_route.iter().next().map(|(k, v)| (*k, v.0))
                    && let Some((sens, l, octets)) = self.en_route.remove(&(t, n))
                {
                    self.livrer(sens, l, &octets);
                }
                continue;
            }
            if self.relance_us == Some(prochaine) {
                self.relance_us = None;
                let dernier = self.relance_dernier.take();
                self.lancer_l_autorite(dernier);
                continue;
            }
            if self.prochaine_panne_us == prochaine {
                self.panne();
                continue;
            }
            self.image();
        }
    }

    /// Une image de la page : l'autorité joue, les téléphones se réveillent.
    fn image(&mut self) {
        let t = self.prochaine_image_us;
        self.prochaine_image_us = t.saturating_add(PAS_US);
        self.bilan.pas = self.bilan.pas.saturating_add(1);
        if let Some(a) = self.autorite.as_mut() {
            let m = DeLaPage::Image {
                t_us: t,
                periode_us: 16_667,
            };
            if let Ok(o) = m.encoder() {
                a.page(t, &o);
            }
            if let Some(arret) = a.arret() {
                self.bilan.arret = Some(format!("{arret:?}"));
            }
        }
        self.vider_l_autorite();
        for k in 0..self.telephones.len() {
            let Some(tel) = self.telephones.get_mut(k) else {
                continue;
            };
            if tel.part_us.is_some_and(|p| t >= p) {
                tel.part_us = None;
                self.couper(k, None);
                continue;
            }
            if tel.revient_us.is_some_and(|r| t >= r) {
                tel.revient_us = None;
                self.relier(k);
                continue;
            }
            let local = t.saturating_add(tel.decalage_us);
            tel.appareil.reveiller(local);
            self.vider_le_telephone(k);
        }
    }

    /// Une panne tirée de la graine.
    fn panne(&mut self) {
        self.prochaine_panne_us = self.t_us.saturating_add(self.hasard.entre(
            self.pannes.tous_les_us / 2,
            self.pannes.tous_les_us.saturating_mul(3) / 2,
        ));
        let n = self.telephones.len() as u64;
        let sorte = self.hasard.sous(10);
        let k = usize::try_from(self.hasard.sous(n.max(1))).unwrap_or(0);
        let apres = self.hasard.entre(300_000, 3_000_000);
        match sorte {
            0 | 1 if self.pannes.autorite == Oui::Oui && self.autorite.is_some() => {
                let apres = self.hasard.entre(200_000, 2_000_000);
                self.tuer_l_autorite(apres);
            }
            2 | 3 if n > 0 => {
                self.bilan.coupures = self.bilan.coupures.saturating_add(1);
                self.couper(k, Some(apres));
            }
            4 | 5 if n > 0 => self.recharger(k, apres),
            6 if n > 0 => {
                let _: Option<usize> = self.jumeau(k, Some(apres));
            }
            7 if n > 0 => {
                let _: Option<Lien> = self.rejouer_la_poignee(k);
            }
            _ => {
                // Un téléphone remplacé reprend sa place.
                if let Some(k) = self.telephones.iter().position(|t| t.appareil.fini()) {
                    self.reprendre_ici(k);
                }
            }
        }
    }

    /// Joue une soirée de `duree_us` avec ses pannes, puis une fin calme : les pannes
    /// s'arrêtent, chaque téléphone revient et se tait, et tout ce qui attend doit être
    /// accusé. Rend le bilan.
    pub fn jouer(&mut self, telephones: usize, duree_us: u64) -> Bilan {
        for _ in 0..telephones {
            let _: usize = self.ajouter_telephone();
        }
        let fin = self.debut_us.saturating_add(duree_us);
        self.avancer_jusqua(fin);
        self.calmer();
        self.bilan()
    }

    /// La fin calme : plus de pannes, l'autorité relancée si elle est morte, chaque
    /// téléphone (sauf les jumeaux) revenu et muet, puis trois secondes.
    pub fn calmer(&mut self) {
        self.prochaine_panne_us = u64::MAX;
        self.avancer_jusqua(self.t_us.saturating_add(3_000_000));
        self.faire_taire();
        for k in 0..self.telephones.len() {
            let jumeau = self
                .telephones
                .iter()
                .take(k)
                .any(|t| self.telephones.get(k).is_some_and(|u| u.jeton == t.jeton));
            if jumeau {
                if self.telephones.get(k).is_some_and(|t| t.lien.is_some()) {
                    self.couper(k, None);
                }
                continue;
            }
            let revenir = self
                .telephones
                .get(k)
                .is_some_and(|t| t.appareil.fini() || (t.lien.is_none() && t.revient_us.is_none()));
            if revenir {
                self.reprendre_ici(k);
            }
        }
        self.avancer_jusqua(self.t_us.saturating_add(4_000_000));
    }

    /// Les cases que la garde de l'autorité a envoyées au relais et qu'il n'a pas fini
    /// d'écrire : ni dans la garde, ni dans l'historique, ni accusées par `Ecrit`.
    pub fn cases_en_ecriture(&self) -> usize {
        let Some((_, garde)) = self.liens_autorite else {
            return 0;
        };
        self.en_route
            .values()
            .filter(|(s, l, o)| {
                *s == Sens::VersRelais
                    && *l == garde
                    && matches!(VersRelais::decoder(o), Ok(VersRelais::Ecrire { .. }))
            })
            .count()
    }

    /// Ce qui est accusé à un téléphone sans être dans la chaîne écrite `h` : rien ne
    /// devrait l'être (SAL-56). Un téléphone à qui un geste est accusé et que le registre
    /// de l'autorité ne connaît pas est une faute aussi. Rend une ligne par faute.
    pub fn accuses_non_ecrits(&self, h: &Historique) -> Vec<String> {
        let registre = self.autorite.as_ref().and_then(Autorite::registre);
        let telephones: Vec<TelephoneAccuse> = self
            .telephones
            .iter()
            .map(|t| TelephoneAccuse {
                jeton: t.jeton,
                onglet: t.appareil.onglet(),
                accuse: t.appareil.accuse(),
            })
            .collect();
        accuses_hors_de_l_historique(registre.as_ref(), &telephones, h)
    }

    /// Livre ce qui est en route, sans qu'aucune image ne passe : la partie n'avance pas.
    pub fn vider_le_reseau(&mut self) {
        while let Some(((t, n), _)) = self.en_route.iter().next().map(|(k, v)| (*k, v.0)) {
            self.t_us = self.t_us.max(t);
            if let Some((sens, l, octets)) = self.en_route.remove(&(t, n)) {
                self.livrer(sens, l, &octets);
            }
        }
    }

    /// L'autorité écrit tout son journal, le relais le reçoit, puis une partie neuve
    /// reprend de la garde telle que le relais la tient (`Partie::reprendre`). Rend la
    /// forme canonique et l'empreinte de la partie vivante, puis de la reprise : elles
    /// doivent être égales (PHASE-3, § 2.4).
    pub fn comparer_a_la_garde(&mut self) -> Result<([u8; 32], [u8; 32]), String> {
        let t = self.t_us;
        let a = self.autorite.as_mut().ok_or("aucune autorité")?;
        a.ecrire_la_garde(t);
        self.vider_l_autorite();
        self.vider_le_reseau();
        let a = self.autorite.as_mut().ok_or("aucune autorité")?;
        let dernier = a.dernier_ecrit();
        let p = a.partie_mut().ok_or("aucune partie")?;
        let vivante = forme_et_empreinte(p)?;
        let reprise = reprendre_de_la_garde(
            (self.fabriquer)(),
            &self.cle_de_garde(),
            self.reglages.salle,
            &self.relais.cases(),
            dernier,
        )?;
        if vivante != reprise {
            return Err(format!("vivante {vivante:?}, reprise {reprise:?}"));
        }
        Ok(vivante)
    }

    /// Le bilan de la soirée jusqu'ici.
    pub fn bilan(&self) -> Bilan {
        let mut b = self.bilan.clone();
        if let Some(a) = self.autorite.as_ref() {
            b.autorite.ajouter(a.comptes());
        }
        for t in &self.telephones {
            b.gestes_emis = b.gestes_emis.saturating_add(t.appareil.comptes().gestes);
            b.gestes_accuses = b.gestes_accuses.saturating_add(t.appareil.accuse());
        }
        b
    }
}

fn hacher(o: &[u8]) -> [u8; 32] {
    *blake3::hash(o).as_bytes()
}

/// La forme canonique (hachée) et l'empreinte d'une partie : ce qu'une reprise depuis la
/// garde doit retrouver (PHASE-3, § 2.4).
pub fn forme_et_empreinte<F: Fabrique>(p: &mut Partie<F>) -> Result<([u8; 32], [u8; 32]), String>
where
    <F::I as Instance>::Instantane: InstantaneGarde + PartialEq + Clone,
{
    Ok((
        hacher(&p.canonique().map_err(|e| format!("canonique : {e:?}"))?),
        p.empreinte().map_err(|e| format!("empreinte : {e}"))?,
    ))
}

/// Une partie neuve, reprise de la garde `cases` (`garde::relire`, puis
/// `Partie::reprendre`) : sa forme canonique hachée et son empreinte. Le banc s'en sert
/// sur la garde que le vrai relais a écrite sur disque (lot 5).
pub fn reprendre_de_la_garde<F: Fabrique>(
    fabrique: F,
    g: &CleDeGarde,
    salle: CodeSalle,
    cases: &[(Case, Vec<u8>)],
    dernier: Option<u64>,
) -> Result<([u8; 32], [u8; 32]), String>
where
    <F::I as Instance>::Instantane: InstantaneGarde + PartialEq + Clone,
{
    let lue = crate::garde::relire::<<F::I as Instance>::Instantane>(g, salle, cases, dernier)
        .map_err(|e| format!("la garde ne se relit pas : {e:?}"))?;
    let mut r = Partie::reprendre(fabrique, lue.ouverture, &lue.point, &lue.journal)
        .map_err(|e| format!("la reprise échoue : {e:?}"))?;
    forme_et_empreinte(&mut r)
}

/// Ce qu'un téléphone dit de ses accusés, pour [`accuses_hors_de_l_historique`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelephoneAccuse {
    /// Son jeton.
    pub jeton: [u8; 32],
    /// Son onglet.
    pub onglet: u64,
    /// Le dernier geste qui lui est accusé.
    pub accuse: u64,
}

/// Ce qui est accusé à un téléphone sans être dans la chaîne écrite `h` : rien ne devrait
/// l'être (SAL-56). Un téléphone à qui un geste est accusé et que le `registre` de
/// l'autorité ne connaît pas est une faute aussi. Rend une ligne par faute. Le jugement
/// unique de la soirée en mémoire et de la soirée native (lot 5).
pub fn accuses_hors_de_l_historique(
    registre: Option<&Registre>,
    telephones: &[TelephoneAccuse],
    h: &Historique,
) -> Vec<String> {
    let mut fautes = Vec::new();
    for (k, t) in telephones.iter().enumerate() {
        if t.accuse == 0 {
            continue;
        }
        let Some(appareil) = registre
            .and_then(|r| r.par_jeton(&crate::garde::empreinte_jeton(&t.jeton)))
            .map(|i| i.appareil)
        else {
            fautes.push(format!(
                "téléphone {k} : {} geste(s) accusés, et son appareil n'est pas au registre",
                t.accuse
            ));
            continue;
        };
        let absents = (1..=t.accuse)
            .filter(|&n| {
                !h.gestes
                    .contains(&crate::autorite::id_geste(appareil, t.onglet, n).fil())
            })
            .count();
        if absents > 0 {
            fautes.push(format!(
                "téléphone {k} : {absents} geste(s) accusés mais pas écrits"
            ));
        }
    }
    fautes
}

/// La réponse de la page du banc à une demande : un refus, sauf pour ce qui se fait
/// sans rien rendre (un téléchargement, une ligne).
pub fn reponse_par_defaut<'a>(d: &DemandeFil<'_>) -> ReponseFil<'a> {
    match d {
        DemandeFil::Image { .. } => ReponseFil::Image(Err(EchecDemande::Refuse)),
        DemandeFil::Rendu { .. } => ReponseFil::Rendu(Err(EchecDemande::Refuse)),
        DemandeFil::Lire { .. } => ReponseFil::Lu(Err(EchecDemande::Refuse)),
        DemandeFil::Telecharger { .. } | DemandeFil::Ligne { .. } => ReponseFil::Fait(Ok(())),
    }
}

// ---------------------------------------------------------------------------
// La vérification de la chaîne entière
// ---------------------------------------------------------------------------

/// Ce que dit la chaîne entière des cases d'une soirée.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Historique {
    /// Les cases, et les points parmi elles.
    pub cases: u64,
    /// Les points.
    pub points: u64,
    /// Les lignes du journal, de la première à la dernière écrite.
    pub lignes: u64,
    /// Les gestes dans le journal écrit, chacun par son identité.
    pub gestes: BTreeSet<u64>,
    /// Les gestes qui y sont plus d'une fois : aucun ne devrait (SAL-57).
    pub en_double: Vec<u64>,
    /// Les entrées écartées parce que le jeu a fauté : aucune ne devrait, dans une
    /// soirée d'un jeu sain (lot 5).
    pub ecartees: u64,
    /// Les tenus de téléphone placés.
    pub tenus: u64,
    /// Les sauts du numéro de pas (la partie n'a pas suivi l'horloge, D14).
    pub sauts: u64,
    /// Les octets de chaque geste placé, tels que le jeu les a encodés : ce que le relais
    /// ne doit jamais voir en clair (`le_relais_ne_voit_aucun_secret`, lot 5).
    pub octets_des_gestes: BTreeSet<Vec<u8>>,
}

/// Vérifie la chaîne entière des cases écrites dans une soirée (l'historique du relais) :
/// chacune s'ouvre sous l'empreinte de la précédente depuis la première, le journal
/// qu'elles portent est d'un seul tenant (chaque morceau commence où le précédent finit,
/// chaque point à la fin du journal écrit), et chaque geste y est au plus une fois.
pub fn verifier_l_historique<S: InstantaneGarde + Clone>(
    g: &CleDeGarde,
    salle: CodeSalle,
    historique: &[(Case, Vec<u8>)],
) -> Result<Historique, String> {
    let mut h = Historique::default();
    let mut precedente = AVANT_LA_PREMIERE_CASE;
    let mut fin: Option<usize> = None;
    for (rang, (case, octets)) in historique.iter().enumerate() {
        let attendu = u64::try_from(rang).unwrap_or(u64::MAX);
        if numero(*case) != attendu {
            return Err(format!("la case {attendu} manque (lu {case:?})"));
        }
        let entete = EnteteCase {
            salle,
            case: *case,
            precedente,
        };
        let clair = ouvrir_case(g, &entete, octets)
            .map_err(|e| format!("la case {case:?} ne s'ouvre pas à sa place : {e:?}"))?;
        precedente = empreinte_case(octets);
        h.cases = h.cases.saturating_add(1);
        match case {
            Case::Point(_) => {
                h.points = h.points.saturating_add(1);
                let c = decoder_contenu_point(&clair)
                    .map_err(|e| format!("le point {case:?} ne se lit pas : {e:?}"))?;
                let indice = decoder_point::<S>(&c.point)
                    .map_err(|e| format!("le point {case:?} ne se relit pas : {e:?}"))?
                    .indice();
                match fin {
                    None => fin = Some(indice),
                    Some(f) if f == indice => {}
                    Some(f) => {
                        return Err(format!(
                            "le point {case:?} est à l'indice {indice}, le journal écrit finit à {f}"
                        ));
                    }
                }
            }
            Case::Journal(_) => {
                let c = decoder_contenu_journal(&clair)
                    .map_err(|e| format!("le journal {case:?} ne se lit pas : {e:?}"))?;
                let j = decoder_journal(&c.lignes)
                    .map_err(|e| format!("les lignes de {case:?} ne se lisent pas : {e:?}"))?;
                if fin != Some(j.base()) {
                    return Err(format!(
                        "le morceau {case:?} commence à {}, le journal écrit finit à {fin:?}",
                        j.base()
                    ));
                }
                fin = Some(j.fin());
                h.lignes = h
                    .lignes
                    .saturating_add(u64::try_from(j.lignes().len()).unwrap_or(u64::MAX));
                for l in j.lignes() {
                    match l {
                        Ligne::Arrivees { arrivees, .. } => {
                            for a in arrivees {
                                match &a.quoi {
                                    QuoiPlace::Geste { id, octets, .. } => {
                                        let id: IdGeste = *id;
                                        if !h.gestes.insert(id.fil()) {
                                            h.en_double.push(id.fil());
                                        }
                                        h.octets_des_gestes.insert(octets.clone());
                                    }
                                    QuoiPlace::Tenu { .. } => {
                                        h.tenus = h.tenus.saturating_add(1);
                                    }
                                    QuoiPlace::Manette { .. }
                                    | QuoiPlace::Ecran(_)
                                    | QuoiPlace::Presence(_)
                                    | QuoiPlace::Reponse { .. } => {}
                                }
                            }
                        }
                        Ligne::Entree(LigneEntree {
                            issue: IssueJournal::Ecartee { .. },
                            ..
                        }) => h.ecartees = h.ecartees.saturating_add(1),
                        Ligne::Saut { .. } => h.sauts = h.sauts.saturating_add(1),
                        Ligne::Entree(_)
                        | Ligne::Empreinte { .. }
                        | Ligne::Pause { .. }
                        | Ligne::Vue { .. }
                        | Ligne::Partie { .. }
                        | Ligne::Reprise { .. } => {}
                    }
                }
            }
        }
    }
    Ok(h)
}
