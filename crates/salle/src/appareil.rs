//! Un appareil (PHASE-3, § 2.10) : le cœur du téléphone, sans sa page.
//!
//! **Sans entrées-sorties**, comme l'autorité : la coque (la page du téléphone au lot 12,
//! le banc natif aux lots 4b et 5) lui donne les trames du relais, l'heure et un hasard,
//! et envoie ce qu'il rend. Il tient :
//!
//! - **le lien** : le bonjour au relais, la poignée de main Noise, puis le bonjour chiffré
//!   à l'autorité, son premier message de transport (SAL-17) ; une autorité qui revient
//!   (`AutoritePresente`) refait la poignée de main ;
//! - **les gestes** : numérotés dans l'onglet à partir de 1, gardés en file jusqu'à leur
//!   accusé durable (SAL-56), renvoyés après chaque accueil au-delà du dernier placé ;
//!   une file pleine se dit ([`Envoi::FilePleine`], SAL-07) ;
//! - **le tenu** : le dernier gagne, numéroté au-dessus de ce que l'accueil dit reçu
//!   (SAL-38, REM-14), redit à sa période (P27) ;
//! - **l'heure de l'autorité**, estimée par écho au plus petit aller-retour (SAL-128) :
//!   `fait` et la trace d'un geste en viennent ;
//! - **ce qu'il reçoit** : la vue entière (SAL-28), les avis, les refus typés et
//!   définitifs (SAL-42), l'attente.
//!
//! Ce que la page conserve d'un rechargement (`sessionStorage`, P26 : le jeton, l'onglet,
//! les gestes en file) reste à travers [`Appareil::recharger`] ; le reste repart de zéro.
//!
//! **En mode banc** (PHASE-3, § 8.1, lots 4b et 5), l'appareil joue tels quels les coups
//! que l'autorité lui envoie (`VersAppareil::Banc`), par numérotation, file, chiffrement
//! et relais. Au lot 12, ils passeront par la page.

use jeu_abi::{Capacites, Instant, Pas};
use liaison::{
    BATTEMENT_MS, Classe, CodeSalle, DuRelais, PROTOCOLE_RELAIS, RefusRelais, Role, VersRelais,
};

use crate::chiffre::{Fragment, Hasard, Initiateur, Session};
use crate::fil::{
    AttenteCommun, CoupBanc, LimitesSalle, PROTOCOLE, RefusSalle, TraceTelephone, VersAppareil,
    VersAutorite,
};
use crate::horloge::{Estimation, instant_de_la_partie};
use crate::livraison::Reception;

/// La file d'un appareil avant le premier accueil, qui seul dit `limites.file` : celle de
/// Sidequest, la plus petite des trois jeux (choisi).
pub const FILE_AVANT_ACCUEIL: usize = 32;

/// Le canal remplaçable des tenus : le dernier gagne (SAL-70).
pub const CANAL_TENU: u8 = 0;

/// Le bonjour chiffré se redit s'il reste sans accueil 2 s (choisi) : un accueil perdu
/// pousse l'appareil à répéter sa demande, et rejoindre est idempotent (SAL-29).
pub const BONJOUR_REDIT_US: u64 = 2_000_000;

/// Un coup du bot que l'autorité envoie en mode banc, gardé pour que la page le joue
/// (lot 12, `crate::page::JoueurDePage`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoupRecu {
    /// Un geste.
    Geste(Vec<u8>),
    /// Un état tenu.
    Tenu(Vec<u8>),
}

/// Le plus de gestes que la page reprend de `sessionStorage` : la plus grande file qu'un
/// jeu peut déclarer (`limites.file`, un `u16`).
pub const FILE_MAX_GARDEE: usize = u16::MAX as usize;

/// La file gardée à travers un rechargement (P26).
#[derive(serde::Serialize, serde::Deserialize)]
struct FileGardee {
    prochain_geste: u64,
    accuse: u64,
    file: Vec<(u64, Vec<u8>, Instant, Pas, TraceTelephone)>,
}

/// Pourquoi un geste n'est pas parti.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Envoi {
    /// La file a `limites.file` gestes qui attendent leur accusé (SAL-07).
    FilePleine,
    /// Le geste dépasse `limites.geste`.
    TropLong,
}

/// Où en est le lien de l'appareil.
enum Lien {
    /// Aucun lien au relais.
    Hors,
    /// Le bonjour envoyé au relais, sans réponse encore.
    Bonjour,
    /// Relié au relais ; l'autorité est absente, ou la poignée de main pas commencée.
    Relie,
    /// La poignée de main commencée.
    Poignee(Box<Initiateur>),
    /// La session ouverte ; accueilli, ou le bonjour chiffré envoyé.
    Session {
        session: Box<Session>,
        accueil: Option<u32>,
    },
    /// Un refus final : l'appareil ne se reconnecte pas seul (SAL-18).
    Fini,
}

/// Un geste qui attend son accusé.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EnFile {
    numero: u64,
    octets: Vec<u8>,
    fait: Instant,
    vu: Pas,
    trace: TraceTelephone,
}

/// Ce que l'appareil a compté.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComptesAppareil {
    /// Les gestes émis.
    pub gestes: u64,
    /// Les gestes envoyés, renvois compris.
    pub envois: u64,
    /// Les gestes refusés faute de place (SAL-07).
    pub files_pleines: u64,
    /// Les tenus envoyés, redites comprises.
    pub tenus: u64,
    /// Les accueils reçus.
    pub accueils: u64,
    /// Les vues reçues.
    pub vues: u64,
    /// Les poignées de main commencées.
    pub poignees: u64,
    /// Les trames de l'autorité qui ne s'ouvrent pas.
    pub illisibles: u64,
}

/// Le cœur d'un téléphone.
pub struct Appareil<H: Hasard> {
    fragment: Fragment,
    salle: CodeSalle,
    jeton: [u8; 32],
    onglet: u64,
    capacites: Capacites,
    hasard: H,
    lien: Lien,
    file: Vec<EnFile>,
    prochain_geste: u64,
    accuse: u64,
    tenu: Option<Vec<u8>>,
    numero_tenu: u64,
    redite_us: Option<u64>,
    prochaine_redite_us: u64,
    limites: Option<LimitesSalle>,
    horloge: Estimation,
    prochain_echo_us: u64,
    vue: Option<(Pas, Vec<u8>)>,
    avis: Vec<Vec<u8>>,
    refus: Option<RefusSalle>,
    refus_relais: Option<RefusRelais>,
    attente: Option<AttenteCommun>,
    sorties: Vec<Vec<u8>>,
    comptes: ComptesAppareil,
    banc: bool,
    /// Quand redire le bonjour, s'il reste sans accueil.
    redire_bonjour_us: Option<u64>,
    /// Le module du jeu et les ressources de la page, reçus de l'autorité (lot 12).
    reception: Reception,
    /// Depuis quand un écho attend sa réponse : la coupure se juge dessus (P28).
    echo_attendu_depuis: Option<u64>,
    /// La redite du tenu est tue jusqu'au prochain tenu (l'arrière-plan, REM-11).
    redite_tue: bool,
    /// Les coups du banc gardés pour la page, au lieu d'être joués tels quels (lot 12).
    coups: Option<Vec<CoupRecu>>,
    /// Le lien que le relais a donné à la connexion courante (son accueil) : le banc y
    /// retrouve les trames du téléphone dans l'enregistrement du relais (M7).
    lien_relais: Option<u32>,
    /// Les allers-retours de ses échos, (reçu à, durée) en microsecondes, que le banc
    /// relève (*correction du lot 12* : un téléphone qui rejoint ne retarde personne) ; au
    /// plus [`ALLERS_RETOURS_GARDES`] en attente d'être relevés.
    allers_retours: Vec<(u64, u64)>,
}

/// Le plus d'allers-retours d'écho qu'un appareil garde pour le banc avant qu'il les relève
/// (choisi : quatre minutes d'échos toutes les 250 ms ; au-delà, les suivants se perdent).
pub const ALLERS_RETOURS_GARDES: usize = 1024;

impl<H: Hasard> Appareil<H> {
    /// Un appareil neuf : le fragment du QR, le code de salle de l'adresse, son jeton
    /// (tiré une fois, gardé dans son `localStorage`) et son onglet. `redite_us` : la
    /// période de redite du tenu (`liaison::periode_de_redite`), aucune sinon.
    pub fn new(
        fragment: Fragment,
        salle: CodeSalle,
        (jeton, onglet): ([u8; 32], u64),
        capacites: Capacites,
        redite_us: Option<u64>,
        hasard: H,
    ) -> Appareil<H> {
        Appareil {
            fragment,
            salle,
            jeton,
            onglet,
            capacites,
            hasard,
            lien: Lien::Hors,
            file: Vec::new(),
            prochain_geste: 1,
            accuse: 0,
            tenu: None,
            numero_tenu: 0,
            redite_us,
            prochaine_redite_us: 0,
            limites: None,
            horloge: Estimation::default(),
            prochain_echo_us: 0,
            vue: None,
            avis: Vec::new(),
            refus: None,
            refus_relais: None,
            attente: None,
            sorties: Vec::new(),
            comptes: ComptesAppareil::default(),
            banc: true,
            redire_bonjour_us: None,
            reception: Reception::default(),
            echo_attendu_depuis: None,
            redite_tue: false,
            allers_retours: Vec::new(),
            coups: None,
            lien_relais: None,
        }
    }

    /// Les coups du banc passent par la page (lot 12) : l'appareil les garde
    /// ([`Appareil::prendre_les_coups`]) au lieu de les envoyer tels quels.
    pub fn coups_par_la_page(&mut self) {
        self.coups.get_or_insert_with(Vec::new);
    }

    /// Les coups du banc reçus depuis le dernier appel, pour la page.
    pub fn prendre_les_coups(&mut self) -> Vec<CoupRecu> {
        self.coups.as_mut().map(core::mem::take).unwrap_or_default()
    }

    /// L'appareil ne joue plus les coups que le banc lui envoie : la fin d'une soirée du
    /// banc, où tout ce qui attend doit être accusé.
    pub fn taire_le_banc(&mut self) {
        self.banc = false;
    }

    /// La page se recharge : même jeton, même onglet, mêmes gestes en file
    /// (`sessionStorage`, P26) ; le lien, la session, la vue, l'horloge et la
    /// numérotation des tenus repartent de zéro. L'accueil réaligne les tenus (SAL-38).
    pub fn recharger(&mut self) {
        self.lien = Lien::Hors;
        self.tenu = None;
        self.numero_tenu = 0;
        self.prochaine_redite_us = 0;
        self.limites = None;
        self.horloge = Estimation::default();
        self.prochain_echo_us = 0;
        self.vue = None;
        self.avis.clear();
        self.refus = None;
        self.refus_relais = None;
        self.attente = None;
        self.sorties.clear();
        // La page rechargée a perdu le module : il revient par la connexion (P18).
        self.reception = Reception::default();
        self.echo_attendu_depuis = None;
        self.redite_tue = false;
    }

    /// Ce que la page garde dans `sessionStorage` à travers un rechargement (P26, SAL-56) :
    /// le prochain numéro de geste, le dernier accusé et les gestes en file, encodés.
    pub fn garder_la_file(&self) -> Vec<u8> {
        let g = FileGardee {
            prochain_geste: self.prochain_geste,
            accuse: self.accuse,
            file: self
                .file
                .iter()
                .map(|g| (g.numero, g.octets.clone(), g.fait, g.vu, g.trace))
                .collect(),
        };
        let mut o = Vec::new();
        let _: Result<(), _> = jeu_abi::encoder(&g, &mut o);
        o
    }

    /// Reprend ce que [`Appareil::garder_la_file`] a gardé, au chargement d'une page
    /// rechargée. Des octets qui ne se relisent pas, ou plus de gestes que
    /// [`FILE_MAX_GARDEE`], ne changent rien : un appareil sans file reste un appareil
    /// qui marche (SAL-32). Rend vrai si la file est reprise.
    pub fn reprendre_la_file(&mut self, octets: &[u8]) -> bool {
        let Ok(g) = jeu_abi::decoder::<FileGardee>(octets) else {
            return false;
        };
        if g.file.len() > FILE_MAX_GARDEE || g.file.iter().any(|f| f.0 >= g.prochain_geste) {
            return false;
        }
        self.prochain_geste = g.prochain_geste.max(1);
        self.accuse = g.accuse;
        self.file = g
            .file
            .into_iter()
            .map(|(numero, octets, fait, vu, trace)| EnFile {
                numero,
                octets,
                fait,
                vu,
                trace,
            })
            .collect();
        true
    }

    /// Le module du jeu reçu entier et vérifié, et son empreinte (lot 12).
    pub fn module(&self) -> Option<([u8; 32], &[u8])> {
        self.reception.module()
    }

    /// L'empreinte du module que la salle joue, dite par l'accueil.
    pub const fn module_annonce(&self) -> Option<[u8; 32]> {
        self.reception.empreinte()
    }

    /// Les ressources de la page reçues (polices, images), par clé d'asset.
    pub fn ressources(&self) -> &[(u64, Vec<u8>)] {
        self.reception.ressources()
    }

    /// Les morceaux du module reçus, sur le total attendu.
    pub fn progres_du_module(&self) -> (usize, usize) {
        self.reception.progres()
    }

    /// Les modules reçus dont le BLAKE3 n'était pas celui annoncé.
    pub const fn modules_faux(&self) -> u64 {
        self.reception.faux
    }

    /// Le lien que le relais a donné à la connexion courante.
    pub const fn lien_relais(&self) -> Option<u32> {
        self.lien_relais
    }

    /// L'écho reste-t-il sans réponse depuis `COUPURE_US` (P28) ?
    pub fn coupe(&self, t_us: u64) -> bool {
        self.accueilli()
            && self
                .echo_attendu_depuis
                .is_some_and(|t0| t_us.saturating_sub(t0) >= crate::page::COUPURE_US)
    }

    /// La période de redite du tenu, connue quand la page a lu la déclaration du jeu
    /// (`liaison::periode_de_redite`, P27) : un téléphone ne la sait pas avant le module.
    pub const fn regler_la_redite(&mut self, redite_us: Option<u64>) {
        self.redite_us = redite_us;
    }

    /// L'arrière-plan : plus rien ne se redit jusqu'au prochain tenu (REM-11).
    pub fn taire_la_redite(&mut self) {
        self.redite_tue = true;
    }

    /// « Reprendre ici » : après un remplacement, l'appareil peut se relier de nouveau,
    /// sur un geste du joueur et jamais de lui-même (SAL-18).
    pub fn reprendre_ici(&mut self) {
        if matches!(self.lien, Lien::Fini) {
            self.lien = Lien::Hors;
            self.refus = None;
        }
    }

    /// Ce qu'il faut envoyer au relais, depuis le dernier appel.
    pub fn sorties(&mut self) -> Vec<Vec<u8>> {
        core::mem::take(&mut self.sorties)
    }

    /// Ce que l'appareil a compté.
    pub const fn comptes(&self) -> ComptesAppareil {
        self.comptes
    }

    /// Son onglet.
    pub const fn onglet(&self) -> u64 {
        self.onglet
    }

    /// Le dernier geste accusé.
    pub const fn accuse(&self) -> u64 {
        self.accuse
    }

    /// Le dernier numéro de geste émis.
    pub const fn dernier_geste(&self) -> u64 {
        self.prochain_geste.saturating_sub(1)
    }

    /// Les gestes qui attendent leur accusé.
    pub fn en_file(&self) -> usize {
        self.file.len()
    }

    /// Les allers-retours de ses échos depuis le dernier relevé : (reçu à, durée), en
    /// microsecondes de son heure locale (le banc seulement).
    pub fn prendre_les_allers_retours(&mut self) -> Vec<(u64, u64)> {
        core::mem::take(&mut self.allers_retours)
    }

    /// Le plus de gestes que la file tient (SAL-07) : la limite de l'accueil, ou
    /// [`FILE_AVANT_ACCUEIL`] avant lui.
    pub fn file_max(&self) -> usize {
        self.limites
            .map_or(FILE_AVANT_ACCUEIL, |l| usize::from(l.file))
    }

    /// Le numéro et les octets de chaque geste qui attend son accusé : ce qui voyage
    /// dans chaque `VersAutorite::Geste`. Le banc s'en sert pour chercher ces gestes en
    /// clair dans ce que le relais voit (`le_relais_ne_voit_aucun_secret`, lot 5).
    pub fn gestes_en_file(&self) -> impl Iterator<Item = (u64, &[u8])> + '_ {
        self.file.iter().map(|g| (g.numero, g.octets.as_slice()))
    }

    /// Le dernier numéro de tenu envoyé.
    pub const fn numero_tenu(&self) -> u64 {
        self.numero_tenu
    }

    /// La dernière vue reçue, et son pas.
    pub fn vue(&self) -> Option<(Pas, &[u8])> {
        self.vue.as_ref().map(|(p, v)| (*p, v.as_slice()))
    }

    /// Les avis reçus.
    pub fn avis(&self) -> &[Vec<u8>] {
        &self.avis
    }

    /// Le refus final de l'autorité, s'il y en a un.
    pub const fn refus(&self) -> Option<RefusSalle> {
        self.refus
    }

    /// Le refus du relais, s'il y en a un.
    pub const fn refus_relais(&self) -> Option<RefusRelais> {
        self.refus_relais
    }

    /// Ce que le commun fait attendre.
    pub const fn attente(&self) -> Option<AttenteCommun> {
        self.attente
    }

    /// Le numéro de la session de son dernier accueil.
    pub const fn session(&self) -> Option<u32> {
        match self.lien {
            Lien::Session { accueil, .. } => accueil,
            Lien::Hors | Lien::Bonjour | Lien::Relie | Lien::Poignee(_) | Lien::Fini => None,
        }
    }

    /// L'appareil est-il accueilli par l'autorité ?
    pub const fn accueilli(&self) -> bool {
        matches!(
            self.lien,
            Lien::Session {
                accueil: Some(_),
                ..
            }
        )
    }

    /// L'appareil a-t-il reçu un refus final ?
    pub const fn fini(&self) -> bool {
        matches!(self.lien, Lien::Fini)
    }

    /// L'appareil a-t-il un lien au relais (ou en demande-t-il un) ?
    pub const fn relie(&self) -> bool {
        !matches!(self.lien, Lien::Hors | Lien::Fini)
    }

    fn vers_relais(&mut self, m: &VersRelais<'_>) {
        if let Ok(o) = m.encoder() {
            self.sorties.push(o);
        }
    }

    fn vers_autorite(&mut self, classe: Classe, m: &VersAutorite<'_>) {
        let Ok(clair) = m.encoder() else {
            return;
        };
        let Lien::Session { session, .. } = &mut self.lien else {
            return;
        };
        if let Ok(trame) = session.sceller(classe, &clair) {
            self.vers_relais(&VersRelais::Trame {
                classe,
                trame: &trame,
            });
        }
    }

    /// Ouvre un lien au relais : le bonjour d'un appareil.
    pub fn relier(&mut self) {
        if matches!(self.lien, Lien::Fini) {
            return;
        }
        self.lien = Lien::Bonjour;
        self.vers_relais(&VersRelais::Bonjour {
            protocole: PROTOCOLE_RELAIS,
            salle: self.salle,
            role: Role::Appareil,
        });
    }

    /// Le lien au relais est perdu : rien de ce qui attend n'est perdu.
    pub fn couper(&mut self) {
        if !matches!(self.lien, Lien::Fini) {
            self.lien = Lien::Hors;
        }
    }

    fn commencer_la_poignee(&mut self) {
        match Initiateur::commencer(&self.fragment, &mut self.hasard) {
            Ok((initiateur, premier)) => {
                self.comptes.poignees = self.comptes.poignees.saturating_add(1);
                self.lien = Lien::Poignee(Box::new(initiateur));
                self.vers_relais(&VersRelais::Trame {
                    classe: Classe::Fiable,
                    trame: &premier,
                });
            }
            Err(_) => self.lien = Lien::Relie,
        }
    }

    /// Une trame du relais.
    pub fn recevoir(&mut self, t_us: u64, octets: &[u8]) {
        let Ok(m) = DuRelais::decoder(octets) else {
            return;
        };
        match m {
            DuRelais::Accueil { lien, .. } => {
                if matches!(self.lien, Lien::Bonjour) {
                    self.lien = Lien::Relie;
                }
                self.lien_relais = Some(lien.0);
            }
            DuRelais::Refus(r) => {
                self.refus_relais = Some(r);
                self.lien = Lien::Hors;
            }
            DuRelais::AutoriteAbsente => {
                self.attente = Some(AttenteCommun::Reprise);
                if !matches!(self.lien, Lien::Hors | Lien::Bonjour | Lien::Fini) {
                    self.lien = Lien::Relie;
                }
            }
            DuRelais::AutoritePresente => {
                if !matches!(self.lien, Lien::Hors | Lien::Bonjour | Lien::Fini) {
                    self.commencer_la_poignee();
                }
            }
            DuRelais::Pour { classe, trame } => self.pour(t_us, classe, trame),
            DuRelais::Ouvert { .. }
            | DuRelais::De { .. }
            | DuRelais::Ferme { .. }
            | DuRelais::Encombre { .. }
            | DuRelais::Echo { .. }
            | DuRelais::Ecrit { .. }
            | DuRelais::Cases(_)
            | DuRelais::Lu { .. } => {}
        }
    }

    fn pour(&mut self, t_us: u64, classe: Classe, trame: &[u8]) {
        match core::mem::replace(&mut self.lien, Lien::Relie) {
            Lien::Poignee(initiateur) => {
                if let Ok(session) = initiateur.finir(trame) {
                    self.lien = Lien::Session {
                        session: Box::new(session),
                        accueil: None,
                    };
                    self.redire_le_bonjour();
                    self.redire_bonjour_us = Some(t_us.saturating_add(BONJOUR_REDIT_US));
                } else {
                    // Une réponse que seule l'autorité du QR sait écrire : celle-ci ne
                    // l'est pas (P12). On recommence.
                    self.comptes.illisibles = self.comptes.illisibles.saturating_add(1);
                    self.commencer_la_poignee();
                }
            }
            Lien::Session {
                mut session,
                accueil,
            } => {
                let ouvert = session.ouvrir(classe, trame);
                self.lien = Lien::Session { session, accueil };
                match ouvert {
                    Ok(clair) => self.message(t_us, &clair),
                    Err(_) => self.comptes.illisibles = self.comptes.illisibles.saturating_add(1),
                }
            }
            autre @ (Lien::Hors | Lien::Bonjour | Lien::Relie | Lien::Fini) => self.lien = autre,
        }
    }

    fn message(&mut self, t_us: u64, clair: &[u8]) {
        let Ok(m) = VersAppareil::decoder(clair) else {
            self.comptes.illisibles = self.comptes.illisibles.saturating_add(1);
            return;
        };
        match m {
            VersAppareil::Accueil {
                session,
                dernier_geste,
                dernier_tenu,
                limites,
                module,
                ..
            } => {
                if let Some(m) = module {
                    self.reception.annoncer(m);
                }
                self.accueil(t_us, session, dernier_geste, dernier_tenu, limites);
            }
            VersAppareil::Refus(r) => {
                self.refus = Some(r);
                self.lien = Lien::Fini;
            }
            VersAppareil::Vue { pas, montre } => {
                self.comptes.vues = self.comptes.vues.saturating_add(1);
                self.attente = None;
                self.vue = Some((pas, montre.to_vec()));
            }
            VersAppareil::Avis { octets } => self.avis.push(octets.to_vec()),
            VersAppareil::Ecarte { numero } => self.file.retain(|g| g.numero != numero),
            VersAppareil::Accuse { jusqu_a } => {
                self.accuse = self.accuse.max(jusqu_a);
                self.file.retain(|g| g.numero > jusqu_a);
            }
            VersAppareil::Attente(a) => self.attente = Some(a),
            VersAppareil::Echo { t_us: t, echo_us } => {
                if echo_us == 0 {
                    // Un écho de l'autorité : on y répond tout de suite (SAL-129).
                    self.vers_autorite(Classe::Fiable, &VersAutorite::Echo { t_us, echo_us: t });
                } else {
                    self.horloge.noter(echo_us, t, t_us);
                    self.echo_attendu_depuis = None;
                    if self.allers_retours.len() < ALLERS_RETOURS_GARDES
                        && let Some(d) = t_us.checked_sub(echo_us)
                    {
                        self.allers_retours.push((t_us, d));
                    }
                }
            }
            VersAppareil::Banc(CoupBanc::Geste(o)) => {
                if self.banc {
                    match self.coups.as_mut() {
                        Some(c) => c.push(CoupRecu::Geste(o.to_vec())),
                        None => {
                            let _: Result<u64, Envoi> = self.geste(t_us, o);
                        }
                    }
                }
            }
            VersAppareil::Banc(CoupBanc::Tenu(o)) => {
                if self.banc {
                    match self.coups.as_mut() {
                        Some(c) => c.push(CoupRecu::Tenu(o.to_vec())),
                        None => self.tenir(t_us, o),
                    }
                }
            }
            VersAppareil::Module {
                empreinte,
                morceau,
                total,
                octets,
            } => {
                if let Some(n) = self.reception.morceau(empreinte, morceau, total, octets) {
                    self.vers_autorite(
                        Classe::Fiable,
                        &VersAutorite::ModuleRecu {
                            empreinte,
                            morceau: n,
                        },
                    );
                }
            }
            VersAppareil::Ressource { cle, octets } => {
                if let Some((empreinte, morceau)) = self.reception.ressource(cle, octets) {
                    self.vers_autorite(
                        Classe::Fiable,
                        &VersAutorite::ModuleRecu { empreinte, morceau },
                    );
                }
            }
        }
    }

    fn accueil(
        &mut self,
        t_us: u64,
        session: u32,
        dernier_geste: u64,
        dernier_tenu: Option<u64>,
        limites: LimitesSalle,
    ) {
        if let Lien::Session { accueil, .. } = &mut self.lien {
            *accueil = Some(session);
        }
        self.comptes.accueils = self.comptes.accueils.saturating_add(1);
        self.limites = Some(limites);
        self.attente = None;
        // Les tenus se numérotent au-dessus de ce que l'autorité a reçu (SAL-38).
        self.numero_tenu = self.numero_tenu.max(dernier_tenu.unwrap_or(0));
        // Les gestes au-delà du dernier placé repartent, dans l'ordre (SAL-56).
        let a_renvoyer: Vec<EnFile> = self
            .file
            .iter()
            .filter(|g| g.numero > dernier_geste)
            .cloned()
            .collect();
        for g in a_renvoyer {
            self.envoyer_le_geste(&g);
        }
        // L'état tenu entier, avec un numéro neuf (REM-13).
        if self.tenu.is_some() {
            self.envoyer_le_tenu(t_us);
        }
        self.prochain_echo_us = t_us;
        self.echo_attendu_depuis = None;
        self.echo(t_us);
    }

    fn envoyer_le_geste(&mut self, g: &EnFile) {
        self.comptes.envois = self.comptes.envois.saturating_add(1);
        self.vers_autorite(
            Classe::Fiable,
            &VersAutorite::Geste {
                onglet: self.onglet,
                numero: g.numero,
                octets: &g.octets,
                fait: g.fait,
                vu: g.vu,
                trace: g.trace,
            },
        );
    }

    fn envoyer_le_tenu(&mut self, t_us: u64) {
        let Some(octets) = self.tenu.clone() else {
            return;
        };
        self.numero_tenu = self.numero_tenu.saturating_add(1);
        self.comptes.tenus = self.comptes.tenus.saturating_add(1);
        let fait = self.instant(t_us);
        self.prochaine_redite_us = self.redite_us.map_or(u64::MAX, |r| t_us.saturating_add(r));
        let numero = self.numero_tenu;
        self.vers_autorite(
            Classe::Remplacable(CANAL_TENU),
            &VersAutorite::Tenu {
                numero,
                octets: &octets,
                fait,
            },
        );
    }

    /// L'instant de la partie maintenant, tel que l'appareil l'estime ; le pas de sa vue
    /// tant qu'aucun écho n'est revenu.
    fn instant(&self, t_us: u64) -> Instant {
        self.horloge.autorite_us(t_us).map_or_else(
            || Instant::de(self.vue.as_ref().map_or(Pas::ZERO, |(p, _)| *p)),
            instant_de_la_partie,
        )
    }

    fn echo(&mut self, t_us: u64) {
        if !self.accueilli() || t_us < self.prochain_echo_us {
            return;
        }
        self.prochain_echo_us = t_us.saturating_add(u64::from(BATTEMENT_MS).saturating_mul(1000));
        if self.echo_attendu_depuis.is_none() {
            self.echo_attendu_depuis = Some(t_us);
        }
        self.vers_autorite(Classe::Fiable, &VersAutorite::Echo { t_us, echo_us: 0 });
    }

    /// Un geste : numéroté, mis en file, envoyé si l'appareil est accueilli. Rend son
    /// numéro.
    pub fn geste(&mut self, t_us: u64, octets: &[u8]) -> Result<u64, Envoi> {
        let (file, longueur) = self.limites.map_or((FILE_AVANT_ACCUEIL, u32::MAX), |l| {
            (usize::from(l.file), l.geste)
        });
        if u32::try_from(octets.len()).map_or(true, |n| n > longueur) {
            return Err(Envoi::TropLong);
        }
        if self.file.len() >= file {
            self.comptes.files_pleines = self.comptes.files_pleines.saturating_add(1);
            return Err(Envoi::FilePleine);
        }
        let numero = self.prochain_geste;
        self.prochain_geste = numero.saturating_add(1);
        self.comptes.gestes = self.comptes.gestes.saturating_add(1);
        let evenement = self.horloge.autorite_us(t_us).unwrap_or(0);
        let g = EnFile {
            numero,
            octets: octets.to_vec(),
            fait: self.instant(t_us),
            vu: self.vue.as_ref().map_or(Pas::ZERO, |(p, _)| *p),
            trace: TraceTelephone {
                evenement_us: evenement,
                envoi_us: evenement,
                incertitude_us: self
                    .horloge
                    .incertitude_us()
                    .map_or(u32::MAX, |i| u32::try_from(i).unwrap_or(u32::MAX)),
            },
        };
        if self.accueilli() {
            self.envoyer_le_geste(&g);
        }
        self.file.push(g);
        Ok(numero)
    }

    /// Renvoie chaque geste en file, déjà placé ou non : ce que ferait une page qui se
    /// trompe, ou un appareil hostile. Le banc s'en sert pour éprouver le
    /// dédoublonnage de l'autorité (SAL-57) ; une page ne l'appelle jamais.
    pub fn renvoyer_la_file(&mut self) {
        if !self.accueilli() {
            return;
        }
        let file = self.file.clone();
        for g in &file {
            self.envoyer_le_geste(g);
        }
    }

    /// L'état tenu change : le dernier gagne.
    pub fn tenir(&mut self, t_us: u64, octets: &[u8]) {
        self.redite_tue = false;
        self.tenu = Some(octets.to_vec());
        if self.accueilli() {
            self.envoyer_le_tenu(t_us);
        }
    }

    /// Le bonjour chiffré, le premier message de transport (SAL-17), dit ou redit sur la
    /// session ouverte. L'autorité y répond par le même accueil (SAL-29).
    pub fn redire_le_bonjour(&mut self) {
        let jeton = self.jeton;
        self.vers_autorite(
            Classe::Fiable,
            &VersAutorite::Bonjour {
                protocole: PROTOCOLE,
                jeton,
                onglet: self.onglet,
                capacites: self.capacites,
                module_connu: self.reception.module().map(|(e, _)| e),
            },
        );
    }

    /// Un réveil : l'écho, la redite du tenu, et le bonjour redit s'il reste sans accueil.
    pub fn reveiller(&mut self, t_us: u64) {
        if let Some(r) = self.redire_bonjour_us
            && t_us >= r
        {
            if matches!(self.lien, Lien::Session { accueil: None, .. }) {
                self.redire_le_bonjour();
                self.redire_bonjour_us = Some(t_us.saturating_add(BONJOUR_REDIT_US));
            } else {
                self.redire_bonjour_us = None;
            }
        }
        self.echo(t_us);
        if self.accueilli()
            && self.tenu.is_some()
            && !self.redite_tue
            && t_us >= self.prochaine_redite_us
        {
            self.envoyer_le_tenu(t_us);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chiffre::{ClePublique, SecretDEntree};
    use crate::simulation::Graine;

    fn appareil() -> Appareil<Graine> {
        Appareil::new(
            Fragment {
                secret: SecretDEntree::depuis_octets([1; 16]),
                autorite: ClePublique([2; 32]),
            },
            CodeSalle([1, 2, 3, 4]),
            ([3; 32], 9),
            Capacites::AUCUNE,
            None,
            Graine::new("essai", 1),
        )
    }

    /// P26, SAL-56 : les gestes non accusés survivent au rechargement de la page, par
    /// `sessionStorage` ; la numérotation reprend où elle était. Des octets qui ne se
    /// relisent pas ne changent rien (SAL-32).
    #[test]
    fn la_file_survit_au_rechargement_sal56() {
        let mut a = appareil();
        assert_eq!(a.geste(0, b"un"), Ok(1));
        assert_eq!(a.geste(0, b"deux"), Ok(2));
        let gardee = a.garder_la_file();
        let mut b = appareil();
        assert!(b.reprendre_la_file(&gardee));
        assert_eq!(b.en_file(), 2);
        assert_eq!(
            b.gestes_en_file().collect::<Vec<_>>(),
            [(1, &b"un"[..]), (2, &b"deux"[..])]
        );
        assert_eq!(b.geste(0, b"trois"), Ok(3));
        let mut c = appareil();
        assert!(!c.reprendre_la_file(&[0xff, 0xff, 0xff]));
        assert_eq!(c.en_file(), 0);
        assert_eq!(c.geste(0, b"x"), Ok(1));
    }
}
