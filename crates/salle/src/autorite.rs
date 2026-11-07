//! L'autorité de la salle (PHASE-3, § 2.3 à § 2.7) : la partie qui fait foi, ses deux
//! liens au relais, les sessions chiffrées des appareils, le registre, la garde.
//!
//! **Sans entrées-sorties** : la coque (le Worker, ou le banc natif) lui donne les
//! octets reçus de ses deux WebSocket et de la page, l'heure, et un hasard ; elle rend
//! les octets à envoyer ([`Sorties`]). Les tests natifs la font tourner sur wasmtime ou
//! sur le modèle du cœur, à horloge simulée et graine fixe (SAL-09, SAL-10).
//!
//! # Le démarrage
//!
//! L'autorité dit bonjour au relais sur le lien « temps réel », puis ouvre la garde sur
//! le second lien, la liste et la lit. Une garde vide ouvre une partie neuve, et l'écrit
//! aussitôt (son premier point) ; une garde lue reprend la partie
//! (`Partie::reprendre`) et le registre. Ce qui arrive des appareils entre-temps attend.
//! Une garde qui ne se relit pas, ou plus vieille que le dernier numéro que la coque sait
//! écrit, arrête l'autorité ([`Arret`]) : on ne joue pas sur une garde qu'on ne croit pas.
//!
//! # Les appareils
//!
//! - **Aucun effet avant le premier message de transport** (SAL-17) : une poignée de
//!   main ne fait qu'une session non confirmée. Le bonjour de l'appareil, chiffré, est
//!   le premier message qu'on écoute ; ni présence, ni remplacement, ni accueil avant.
//! - **Le lien le plus récent gagne**, dans l'ordre où le relais a ouvert les liens
//!   (SAL-18, SAL-19) : un bonjour tardif d'un lien plus ancien ne déloge pas le lien
//!   courant, et un lien qui a reçu son message final n'est plus écouté (SAL-21).
//! - **Rejoindre est idempotent** (SAL-29) : un bonjour répété sur le même lien redonne
//!   l'accueil, sans rien changer.
//! - **L'accueil réaligne les compteurs** (SAL-38) : le dernier geste placé de cet
//!   onglet, le dernier tenu reçu de cet appareil (`Partie::numero_tenu`).
//! - **Un geste compte exactement une fois** (SAL-57) : la salle sait, par onglet, le
//!   dernier geste placé ; elle ne l'accuse qu'une fois écrite la case de journal qui le
//!   contient (l'accusé durable, SAL-56), et cette case porte le registre qui le dit.
//! - L'aller-retour est mesuré par l'autorité elle-même (SAL-129).
//!
//! # Le mode banc
//!
//! Avec [`ReglagesSalle::banc`], l'autorité calcule les coups du bot de chaque appareil
//! accueilli (`Partie::bot`) et les lui envoie (`VersAppareil::Banc`) ; l'appareil natif
//! les joue par le vrai chemin (PHASE-3, § 8.1). Les coups du bot de l'écran entrent
//! comme des entrées de l'écran, sans page : la page n'existe qu'au lot 10a.

use std::collections::BTreeMap;

use jeu_abi::{
    Appareil, Capacites, CauseDepart, ComposantBotFil, ComposantFil, CoupFil, DemandeFil, EcranFil,
    IdGeste, Oui, Pas, Presence, RegardFil, ReponseFil, Retour, RoleAppareil, StyleFil, TexteCourt,
    decoder,
};
use liaison::{
    BATTEMENT_MS, Case, Classe, CodeSalle, DuRelais, Lien, PROTOCOLE_RELAIS, RaisonCoupure,
    RefusRelais, Role, SILENCE_MAX_MS, VersRelais,
};
use partie::persistance::InstantaneGarde;
use partie::{
    ARRIERES_PAR_TOUR, Arrivee, Avancee, BUDGETS_DE_SOIREE, ComposantArrivee, DemandeArrivee,
    EcranArrivee, Fabrique, Instance, Ligne, Ouverture, PanneOutil, Partie, Placement, QuoiPlace,
    RefusOuverture, RefusReprise, ReponseArrivee,
};

use crate::chiffre::{
    CleDeGarde, Fragment, Hasard, PaireAutorite, SecretDEntree, Session, TAILLE_POIGNEE, repondre,
};
use crate::ecran::{
    AppareilVu, CommandeEcran, DeLaPage, EtatSalle, PanneSalle, RapportPanne, VersPage,
};
use crate::fil::{
    LimitesSalle, PROTOCOLE, RefusSalle, VersAppareil, VersAutorite, protocole_d_un_bonjour,
};
use crate::garde::{
    Ecrivain, GardeLue, Inscrit, MARQUES_MAX, Marque, ONGLETS_RETENUS, OngletInscrit,
    PointEnScellement, RefusDeGarde, Registre, Suite, empreinte_jeton, relire,
};
use crate::horloge::{AllerRetour, Horloge};

/// Une case de journal toutes les 100 ms qui ont écrit (choisi, PHASE-3, § 2.4 ; M11).
pub const JOURNAL_TOUS_LES_US: u64 = 100_000;

/// Ce qu'un réveil chiffre au plus d'un point de reprise en train de se sceller (lot 10a,
/// correction après la vérification) : 128 Kio. Scellé d'un coup, un point de Punchline
/// (de 0,9 à 1,4 Mo) tenait le Worker de Firefox 158, onglet caché, de 9 à 24 ms
/// (`soiree-banc arriere-plan`, constaté le 2026-11-07), assez pour que le réveil suivant
/// manque deux pas et qu'un jeu qui ne rattrape qu'un pas saute ; une tranche en coûte
/// donc au plus environ 2 ms à ce rythme (calculé). Choisi.
pub const TRANCHE_DU_POINT: usize = 128 * 1024;

/// Une poignée de main inachevée, ou aucun bonjour, en 10 s : le lien est coupé (choisi,
/// § 2.5, § 2.6).
pub const POIGNEE_MAX_US: u64 = 10_000_000;

/// Les pas entre deux consultations du bot d'un appareil, en mode banc : 250 ms, comme
/// le téléphone sage de l'épreuve.
pub const PERIODE_BOT: u64 = 15;

/// Le canal remplaçable des vues : la dernière vue d'un appareil remplace celle qui
/// attend (SAL-112, SAL-113).
pub const CANAL_VUE: u8 = 0;

/// Les liens de plus que `places.appareils` : onglets jumeaux et reconnexions (choisi,
/// § 2.5).
pub const LIENS_EN_PLUS: u16 = 4;

/// Les clés de la soirée, que la coque garde dans `IndexedDB` avec le dernier numéro écrit
/// (P37). Leur `Debug` ne montre aucun secret.
#[derive(Debug)]
pub struct Cles {
    /// La paire statique de l'autorité ; sa clé publique est dans le QR.
    pub paire: PaireAutorite,
    /// Le secret d'entrée, dans le QR.
    pub secret: SecretDEntree,
    /// La clé de garde G : jamais dans le QR.
    pub garde: CleDeGarde,
}

/// Les réglages de la salle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReglagesSalle {
    /// Le code de salle.
    pub salle: CodeSalle,
    /// Le mode banc : les coups des bots (PHASE-3, § 8.1). Jamais en soirée réelle.
    pub banc: Oui,
    /// La cadence des cases de journal.
    pub journal_tous_les_us: u64,
    /// Les pas entre deux points écrits à la garde (`POINT_TOUS_LES`, 600, M12).
    pub point_tous_les: u64,
    /// Ce qu'un réveil chiffre au plus d'un point ([`TRANCHE_DU_POINT`]).
    pub tranche_du_point: usize,
    /// La poignée de main la plus longue.
    pub poignee_max_us: u64,
}

impl ReglagesSalle {
    /// Les réglages d'une soirée, aux valeurs choisies de PHASE-3.
    pub const fn soiree(salle: CodeSalle) -> ReglagesSalle {
        ReglagesSalle {
            salle,
            banc: Oui::Non,
            journal_tous_les_us: JOURNAL_TOUS_LES_US,
            point_tous_les: 600,
            tranche_du_point: TRANCHE_DU_POINT,
            poignee_max_us: POIGNEE_MAX_US,
        }
    }
}

/// Ce que l'autorité a à envoyer, en octets déjà encodés.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sorties {
    /// Sur le lien « temps réel » : des `VersRelais`.
    pub temps_reel: Vec<Vec<u8>>,
    /// Sur le lien de la garde : des `VersRelais`.
    pub garde: Vec<Vec<u8>>,
    /// À la page de l'écran : des [`VersPage`].
    pub page: Vec<Vec<u8>>,
}

/// Pourquoi l'autorité s'est arrêtée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arret {
    /// Le relais l'a refusée, elle ou sa garde ; `Remplacee` : une autorité plus récente
    /// tient la salle (P36).
    Relais(RefusRelais),
    /// La garde ne se relit pas.
    Garde(RefusDeGarde),
    /// La partie ne s'ouvre pas.
    Ouverture(RefusOuverture),
    /// La partie ne reprend pas.
    Reprise(RefusReprise),
    /// Le commun a lâché pendant la partie.
    Panne(PanneOutil),
}

/// D'où vient la partie d'une autorité.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Venue {
    /// Ouverte neuve : la garde était vide.
    Neuve,
    /// Reprise de la garde, relue jusqu'à cette case.
    DeLaGarde {
        /// Le numéro de la dernière case relue.
        derniere_case: u64,
    },
}

/// Ce que l'autorité a compté : pour les tests, le banc et le diagnostic.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Comptes {
    /// Les gestes placés.
    pub gestes_places: u64,
    /// Les gestes reçus de nouveau, déjà placés : ignorés (SAL-57).
    pub gestes_en_double: u64,
    /// Les gestes reçus au-delà du suivant attendu : ignorés, l'appareil les renverra.
    pub gestes_hors_suite: u64,
    /// Les gestes que la partie n'a pas placés (malformés, file pleine…).
    pub gestes_refuses: u64,
    /// Les accusés envoyés.
    pub accuses: u64,
    /// Les cases de journal écrites, et les points.
    pub cases_journal: u64,
    /// Les points écrits.
    pub points: u64,
    /// Les points qui ne tombaient pas au bord du journal écrit, et ne se sont pas écrits.
    pub points_sautes: u64,
    /// Les trames refusées au déchiffrement : leur lien est coupé (SAL-117).
    pub trames_illisibles: u64,
    /// Les poignées de main qui n'ont jamais donné de bonjour.
    pub poignees_inachevees: u64,
    /// Les liens remplacés par un plus récent (SAL-18).
    pub remplaces: u64,
    /// Les vues envoyées aux appareils.
    pub vues: u64,
    /// Les coups de bots envoyés aux appareils, en mode banc.
    pub coups_banc: u64,
    /// Les coups du bot de l'écran entrés dans la partie, en mode banc.
    pub coups_ecran: u64,
    /// Les messages qu'on n'a pas pu sceller (trop longs).
    pub trop_longs: u64,
    /// Les gestes écartés à la demande de la page, après deux arrêts muraux pendant eux
    /// (P39, lot 10a).
    pub gestes_ecartes: u64,
    /// Les marques F8 posées (SAL-162, lot 10a).
    pub marques: u64,
}

/// Une partie de la salle : ce qui ne change pas de forme entre ouvrir et reprendre.
struct EnJeu<F: Fabrique> {
    partie: Partie<F>,
    horloge: Horloge,
    ouverture: Ouverture,
    liens: BTreeMap<Lien, EtatLien>,
    fiches: Vec<Fiche>,
    ecrivain: Ecrivain,
    /// L'indice absolu jusqu'où le journal est écrit à la garde.
    journal_ecrit: usize,
    dernier_journal_us: u64,
    dernier_point_pas: u64,
    /// L'indice du point le plus récent écrit, pour tronquer le journal (P10).
    indice_du_point: usize,
    /// Le point en train de se sceller, une tranche par réveil, et son indice : aucune
    /// case de journal ne s'écrit avant lui, qui la précède dans la chaîne.
    point_en_cours: Option<(PointEnScellement, usize)>,
    /// La dernière scène de l'écran envoyée, par son empreinte, et depuis quand.
    scene: Option<[u8; 32]>,
    scene_pas: Pas,
    scene_depuis: Pas,
    prochain_bot_ecran: u64,
    session_suivante: u32,
    dernier_etat: Option<EtatSalle>,
    /// Les marques F8 de la soirée, au plus `MARQUES_MAX` (lot 10a).
    marques: Vec<Marque>,
    /// Les gestes écartés à la demande de la page, au plus `MARQUES_MAX` (P39).
    ecartes: Vec<u64>,
    /// Le registre a changé sans ligne nouvelle (une marque, un geste écarté) : la
    /// prochaine case de journal s'écrit même vide, pour qu'il soit gardé.
    a_ecrire: bool,
    /// D'où la partie vient.
    venue: Venue,
}

/// Où en est un lien d'appareil.
enum EtatLien {
    /// Ouvert par le relais : rien encore, ou la poignée de main attendue.
    Ouvert { depuis_us: u64 },
    /// La poignée de main faite ; la session n'est confirmée qu'à sa première trame
    /// ouverte, et l'appareil n'est accueilli qu'à son bonjour.
    Chiffre {
        session: Box<Session>,
        depuis_us: u64,
        accueil: Option<Accueilli>,
    },
    /// Il a reçu son message final, ou a été coupé : plus rien n'y est entendu (SAL-21).
    Fini,
}

/// Un appareil accueilli sur ce lien.
struct Accueilli {
    appareil: Appareil,
    onglet: u64,
    session: u32,
    /// L'empreinte de la dernière vue envoyée, et le pas où elle a changé.
    vue: Option<[u8; 32]>,
    vue_depuis: Pas,
    prochain_echo_us: u64,
}

/// Ce que la salle sait d'un appareil, de son jeton à ses onglets.
struct Fiche {
    jeton: [u8; 32],
    appareil: Appareil,
    /// Le lien courant, accueilli.
    lien: Option<Lien>,
    onglets: Vec<OngletVivant>,
    /// La dernière présence donnée à la partie : `None` si aucune (ou oublié).
    annonce: Option<Oui>,
    capacites: Capacites,
    aller_retour: AllerRetour,
    derniere_nouvelle_us: u64,
    prochain_bot: u64,
}

/// Un onglet : ses gestes placés, et ceux qui attendent d'être écrits.
struct OngletVivant {
    onglet: u64,
    /// Le dernier geste placé ou refusé : ceux d'avant ne se replacent pas.
    place: u64,
    /// Les gestes placés pas encore écrits, dans l'ordre, avec la case de journal qui
    /// les contient une fois écrite.
    en_vol: Vec<EnVol>,
    /// Le dernier accusé envoyé.
    accuse: u64,
}

#[derive(Clone, Copy)]
struct EnVol {
    numero: u64,
    id: IdGeste,
    case: Option<u64>,
}

impl OngletVivant {
    /// Le dernier geste écrit si les cases jusqu'à `n` le sont : avant le premier geste
    /// en vol qui n'est dans aucune d'elles.
    fn ecrit_si(&self, n: Option<u64>) -> u64 {
        self.en_vol
            .iter()
            .find(|e| match (e.case, n) {
                (Some(c), Some(n)) => c > n,
                (None, _) | (Some(_), None) => true,
            })
            .map_or(self.place, |e| e.numero.saturating_sub(1))
    }
}

/// La phase de l'autorité.
enum Phase<F: Fabrique> {
    Demarrage {
        fabrique: Option<F>,
        garde_ouverte: bool,
        a_lire: Option<Vec<Case>>,
        lues: Vec<(Case, Vec<u8>)>,
        /// Ce qui est arrivé sur le temps réel avant la partie.
        tampon: Vec<Vec<u8>>,
    },
    EnJeu(Box<EnJeu<F>>),
    Arretee(Arret),
}

/// L'autorité de la salle.
pub struct Autorite<F: Fabrique, H: Hasard> {
    cles: Cles,
    reglages: ReglagesSalle,
    hasard: H,
    dernier_n: Option<u64>,
    lien: Option<Lien>,
    phase: Phase<F>,
    sorties: Sorties,
    comptes: Comptes,
    /// Le geste que la page demande d'écarter : il a bloqué deux fois un Worker (P39).
    a_ecarter: Option<IdGeste>,
    /// Le retard du dernier réveil, que la coque mesure, dit au prochain battement.
    retard_ms: u32,
    /// L'horloge que la coque donne pour chronométrer les tours (le banc ; jamais une
    /// décision : rien de ce qu'elle rend ne va à la partie ni à la garde).
    chrono: Option<fn() -> f64>,
    /// Les durées du dernier tour, si `chrono` est donnée.
    durees: DureesDuTour,
}

/// Les durées du dernier réveil, en millisecondes, quand la coque a donné une horloge
/// ([`Autorite::chronometrer`]) : ce que le banc rapporte d'un tour long (lot 10a,
/// `arriere-plan` ; la mesure du point dans le Worker, M3, vient au lot 11).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DureesDuTour {
    /// `avancer_borne` et ce qui en découle.
    pub avancer: f64,
    /// Les vues de l'écran et des téléphones.
    pub vues: f64,
    /// La garde : le journal, et le point s'il est dû.
    pub garde: f64,
    /// Dans `garde`, l'instantané du point (`Partie::point`).
    pub instantane: f64,
    /// Dans `garde`, le point encodé et scellé.
    pub sceller: f64,
    /// Le tour entier.
    pub total: f64,
    /// Un point s'est écrit pendant ce tour.
    pub point: bool,
}

/// L'identité d'un geste (PHASE-3, § 2.7) : `blake3(appareil ‖ onglet ‖ numéro)`, tronqué
/// à 8 octets. Stable à travers les renvois et les reprises ; un appareil ne vise pas
/// l'espace d'un autre.
pub fn id_geste(appareil: Appareil, onglet: u64, numero: u64) -> IdGeste {
    let mut h = blake3::Hasher::new();
    h.update(&appareil.fil().to_le_bytes());
    h.update(&onglet.to_le_bytes());
    h.update(&numero.to_le_bytes());
    let empreinte = h.finalize();
    let (premiers, _) = empreinte
        .as_bytes()
        .split_first_chunk::<8>()
        .unwrap_or((&[0; 8], &[]));
    IdGeste::du_fil(u64::from_le_bytes(*premiers))
}

fn encoder_relais(m: &VersRelais<'_>) -> Option<Vec<u8>> {
    m.encoder().ok()
}

impl<F, H> Autorite<F, H>
where
    F: Fabrique,
    <F::I as Instance>::Instantane: InstantaneGarde + PartialEq + Clone,
    H: Hasard,
{
    /// Une autorité qui démarre : elle dit bonjour au relais. `dernier_n` est le dernier
    /// numéro de case que la coque sait écrit (gardé avec G, P37) ; `None` pour une salle
    /// neuve.
    pub fn demarrer(
        fabrique: F,
        cles: Cles,
        reglages: ReglagesSalle,
        hasard: H,
        dernier_n: Option<u64>,
    ) -> Autorite<F, H> {
        let mut a = Autorite {
            cles,
            reglages,
            hasard,
            dernier_n,
            lien: None,
            phase: Phase::Demarrage {
                fabrique: Some(fabrique),
                garde_ouverte: false,
                a_lire: None,
                lues: Vec::new(),
                tampon: Vec::new(),
            },
            sorties: Sorties::default(),
            comptes: Comptes::default(),
            a_ecarter: None,
            retard_ms: 0,
            chrono: None,
            durees: DureesDuTour::default(),
        };
        let preuve = a.cles.garde.preuve();
        a.vers_relais(&VersRelais::Bonjour {
            protocole: PROTOCOLE_RELAIS,
            salle: reglages.salle,
            role: Role::Autorite { preuve },
        });
        a
    }

    /// Ce qu'il faut envoyer, depuis le dernier appel.
    pub fn sorties(&mut self) -> Sorties {
        core::mem::take(&mut self.sorties)
    }

    /// Donne une horloge (en millisecondes) pour chronométrer chaque réveil : le banc
    /// seulement. Rien de ce qu'elle rend ne décide de quoi que ce soit.
    pub const fn chronometrer(&mut self, chrono: fn() -> f64) {
        self.chrono = Some(chrono);
    }

    /// Les durées du dernier réveil (zéro sans horloge).
    pub const fn durees_du_tour(&self) -> DureesDuTour {
        self.durees
    }

    fn heure_du_chrono(&self) -> f64 {
        self.chrono.map_or(0.0, |c| c())
    }

    /// Ce que l'autorité a compté.
    pub const fn comptes(&self) -> Comptes {
        self.comptes
    }

    /// Pourquoi elle s'est arrêtée, si elle l'est.
    pub const fn arret(&self) -> Option<&Arret> {
        match &self.phase {
            Phase::Arretee(a) => Some(a),
            Phase::Demarrage { .. } | Phase::EnJeu(_) => None,
        }
    }

    /// La partie, une fois ouverte ou reprise.
    pub fn partie(&self) -> Option<&Partie<F>> {
        match &self.phase {
            Phase::EnJeu(j) => Some(&j.partie),
            Phase::Demarrage { .. } | Phase::Arretee(_) => None,
        }
    }

    /// La partie, en écriture : pour les tests et le banc, qui en lisent la forme
    /// canonique. Rien ne s'y joue sans passer par l'autorité.
    pub fn partie_mut(&mut self) -> Option<&mut Partie<F>> {
        match &mut self.phase {
            Phase::EnJeu(j) => Some(&mut j.partie),
            Phase::Demarrage { .. } | Phase::Arretee(_) => None,
        }
    }

    /// Le dernier numéro de case que le relais dit écrit : la coque le garde avec G.
    pub fn dernier_ecrit(&self) -> Option<u64> {
        match &self.phase {
            Phase::EnJeu(j) => j.ecrivain.dernier_ecrit().or(self.dernier_n),
            Phase::Demarrage { .. } | Phase::Arretee(_) => self.dernier_n,
        }
    }

    /// D'où vient la partie, une fois en jeu : neuve, ou reprise de la garde relue
    /// jusqu'à une case (`soiree-banc soiree` compare sa forme canonique à celle que
    /// wasmtime reprend des mêmes cases).
    pub fn venue(&self) -> Option<Venue> {
        match &self.phase {
            Phase::EnJeu(j) => Some(j.venue),
            Phase::Demarrage { .. } | Phase::Arretee(_) => None,
        }
    }

    /// La minuterie de secours doit-elle jouer le pas tout de suite, sans attendre
    /// l'image (P6, lot 10a) ? Oui si, à `t_us + avance_us` (le prochain réveil de la
    /// minuterie, sa marge comprise), l'horloge aura atteint le second pas après la
    /// définitive : attendre ferait manquer deux pas d'un coup, et un jeu qui ne rattrape
    /// qu'un pas (`rattrapage: 1`) sauterait. Hors partie, non.
    pub fn secours_presse(&self, t_us: u64, avance_us: u64) -> bool {
        match &self.phase {
            Phase::EnJeu(j) => {
                let atteint = j.horloge.pas(t_us.saturating_add(avance_us)).numero();
                atteint >= j.partie.definitif().numero().saturating_add(2)
            }
            Phase::Demarrage { .. } | Phase::Arretee(_) => false,
        }
    }

    /// Les pas définitifs joués depuis le dernier point écrit : le pire moment d'une
    /// reprise est juste avant le suivant (M12).
    pub fn depuis_le_point(&self) -> Option<u64> {
        match &self.phase {
            Phase::EnJeu(j) => Some(
                j.partie
                    .definitif()
                    .numero()
                    .saturating_sub(j.dernier_point_pas),
            ),
            Phase::Demarrage { .. } | Phase::Arretee(_) => None,
        }
    }

    /// Chaque case envoyée à la garde est-elle écrite (`Ecrit`) ?
    pub fn garde_a_jour(&self) -> bool {
        match &self.phase {
            Phase::EnJeu(j) => {
                j.point_en_cours.is_none()
                    && j.ecrivain.dernier_ecrit().map(|n| n.saturating_add(1))
                        == Some(j.ecrivain.suite().prochain)
            }
            Phase::Demarrage { .. } | Phase::Arretee(_) => false,
        }
    }

    /// Le registre, tel qu'une case l'écrirait maintenant : pour les tests.
    pub fn registre(&self) -> Option<Registre> {
        match &self.phase {
            Phase::EnJeu(j) => Some(registre_de(
                &j.fiches,
                (&j.marques, &j.ecartes),
                j.ecrivain.dernier_ecrit(),
            )),
            Phase::Demarrage { .. } | Phase::Arretee(_) => None,
        }
    }

    fn vers_relais(&mut self, m: &VersRelais<'_>) {
        if let Some(o) = encoder_relais(m) {
            self.sorties.temps_reel.push(o);
        }
    }

    fn vers_garde(&mut self, m: &VersRelais<'_>) {
        if let Some(o) = encoder_relais(m) {
            self.sorties.garde.push(o);
        }
    }

    fn vers_page(&mut self, m: &VersPage<'_>) {
        if let Ok(o) = m.encoder() {
            self.sorties.page.push(o);
        }
    }

    fn arreter(&mut self, a: Arret) {
        let (quoi, detail) = match &a {
            Arret::Relais(r) => (PanneSalle::Relais(*r), format!("{r:?}")),
            Arret::Garde(r) => (PanneSalle::Garde, format!("{r:?}")),
            Arret::Ouverture(r) => (PanneSalle::Outil, format!("{r:?}")),
            Arret::Reprise(r) => (PanneSalle::Outil, format!("{r:?}")),
            Arret::Panne(p) => (PanneSalle::Outil, format!("{p}")),
        };
        self.vers_page(&VersPage::Panne(RapportPanne {
            quoi,
            detail: &detail,
        }));
        self.phase = Phase::Arretee(a);
    }

    // -----------------------------------------------------------------------
    // Ce qui arrive du relais
    // -----------------------------------------------------------------------

    /// Une trame reçue sur le lien « temps réel ».
    pub fn temps_reel(&mut self, t_us: u64, octets: &[u8]) {
        let Ok(m) = DuRelais::decoder(octets) else {
            return;
        };
        match m {
            DuRelais::Accueil { lien, adresses, .. } => {
                if self.lien.is_none() {
                    self.lien = Some(lien);
                    // De quoi écrire le QR (§ 2.6) : le fragment ne part qu'à la page.
                    let fragment = Fragment {
                        secret: SecretDEntree::depuis_octets(*self.cles.secret.octets()),
                        autorite: self.cles.paire.publique(),
                    }
                    .ecrire();
                    self.vers_page(&VersPage::Accueil {
                        salle: self.reglages.salle,
                        fragment: &fragment,
                        adresses,
                    });
                    let preuve = self.cles.garde.preuve();
                    self.vers_garde(&VersRelais::Bonjour {
                        protocole: PROTOCOLE_RELAIS,
                        salle: self.reglages.salle,
                        role: Role::Garde {
                            preuve,
                            autorite: lien,
                        },
                    });
                }
            }
            DuRelais::Refus(r) => self.arreter(Arret::Relais(r)),
            DuRelais::Ouvert { .. }
            | DuRelais::De { .. }
            | DuRelais::Ferme { .. }
            | DuRelais::Encombre { .. } => match &mut self.phase {
                Phase::Demarrage { tampon, .. } => tampon.push(octets.to_vec()),
                Phase::EnJeu(_) => self.d_un_appareil(t_us, &m),
                Phase::Arretee(_) => {}
            },
            DuRelais::Pour { .. }
            | DuRelais::AutoriteAbsente
            | DuRelais::AutoritePresente
            | DuRelais::Echo { .. }
            | DuRelais::Ecrit { .. }
            | DuRelais::Cases(_)
            | DuRelais::Lu { .. } => {}
        }
    }

    /// Une trame reçue sur le lien de la garde.
    pub fn garde(&mut self, t_us: u64, octets: &[u8]) {
        let Ok(m) = DuRelais::decoder(octets) else {
            return;
        };
        match m {
            DuRelais::Accueil { .. } => {
                if let Phase::Demarrage { garde_ouverte, .. } = &mut self.phase
                    && !*garde_ouverte
                {
                    *garde_ouverte = true;
                    self.vers_garde(&VersRelais::Lister);
                }
            }
            DuRelais::Refus(r) => self.arreter(Arret::Relais(r)),
            DuRelais::Cases(cases) => self.cases_listees(t_us, cases),
            DuRelais::Lu { case, octets } => self.case_lue(t_us, case, octets),
            DuRelais::Ecrit { case } => self.case_ecrite(case),
            DuRelais::Ouvert { .. }
            | DuRelais::De { .. }
            | DuRelais::Pour { .. }
            | DuRelais::Ferme { .. }
            | DuRelais::Encombre { .. }
            | DuRelais::AutoriteAbsente
            | DuRelais::AutoritePresente
            | DuRelais::Echo { .. } => {}
        }
    }

    fn cases_listees(&mut self, t_us: u64, cases: Vec<Case>) {
        let Phase::Demarrage { a_lire, .. } = &mut self.phase else {
            return;
        };
        if a_lire.is_some() {
            return;
        }
        if cases.is_empty() {
            *a_lire = Some(Vec::new());
            match self.dernier_n {
                // Une garde perdue n'ouvre pas une partie neuve en silence.
                Some(connue) => self.arreter(Arret::Garde(RefusDeGarde::PlusVieille {
                    lue: None,
                    connue,
                })),
                None => self.ouvrir_neuve(t_us),
            }
            return;
        }
        *a_lire = Some(cases.clone());
        for case in cases {
            self.vers_garde(&VersRelais::Lire { case });
        }
    }

    fn case_lue(&mut self, t_us: u64, case: Case, octets: &[u8]) {
        let complete = {
            let Phase::Demarrage {
                a_lire: Some(a_lire),
                lues,
                ..
            } = &mut self.phase
            else {
                return;
            };
            if !a_lire.contains(&case) || lues.iter().any(|(c, _)| *c == case) {
                return;
            }
            lues.push((case, octets.to_vec()));
            lues.len() == a_lire.len()
        };
        if complete {
            let lues = match &mut self.phase {
                Phase::Demarrage { lues, .. } => core::mem::take(lues),
                Phase::EnJeu(_) | Phase::Arretee(_) => return,
            };
            let lue = relire::<<F::I as Instance>::Instantane>(
                &self.cles.garde,
                self.reglages.salle,
                &lues,
                self.dernier_n,
            );
            match lue {
                Ok(l) => self.reprendre(t_us, l),
                Err(r) => self.arreter(Arret::Garde(r)),
            }
        }
    }

    fn prendre_la_fabrique(&mut self) -> Option<(F, Vec<Vec<u8>>)> {
        match &mut self.phase {
            Phase::Demarrage {
                fabrique, tampon, ..
            } => fabrique.take().map(|f| (f, core::mem::take(tampon))),
            Phase::EnJeu(_) | Phase::Arretee(_) => None,
        }
    }

    fn ouvrir_neuve(&mut self, t_us: u64) {
        let Some((fabrique, tampon)) = self.prendre_la_fabrique() else {
            return;
        };
        let mut graine = [0u8; 32];
        self.hasard.remplir(&mut graine);
        let salle = self.reglages.salle.to_string();
        let ouverture = Ouverture {
            graine,
            pas: 0,
            heure: t_us / 1000,
            salle: TexteCourt::essayer(&salle).unwrap_or(TexteCourt::VIDE),
            budgets: BUDGETS_DE_SOIREE,
            point_tous_les: self.reglages.point_tous_les,
        };
        let partie = match Partie::ouvrir(fabrique, ouverture.clone()) {
            Ok(p) => p,
            Err(r) => return self.arreter(Arret::Ouverture(r)),
        };
        let j = EnJeu::new(
            partie,
            ouverture,
            Vec::new(),
            Ecrivain::new(self.reglages.salle, Suite::neuve()),
            0,
        );
        self.entrer_en_jeu(t_us, j, tampon);
        // Le premier point : une garde existe dès l'ouverture.
        self.ecrire_le_point(t_us, Oui::Oui);
    }

    fn reprendre(&mut self, t_us: u64, l: GardeLue<<F::I as Instance>::Instantane>) {
        let Some((fabrique, tampon)) = self.prendre_la_fabrique() else {
            return;
        };
        let partie = match Partie::reprendre(fabrique, l.ouverture.clone(), &l.point, &l.journal) {
            Ok(p) => p,
            Err(r) => return self.arreter(Arret::Reprise(r)),
        };
        let fiches = l
            .registre
            .appareils
            .iter()
            .map(|i| Fiche {
                jeton: i.jeton,
                appareil: i.appareil,
                lien: None,
                onglets: i
                    .onglets
                    .iter()
                    .map(|o| OngletVivant {
                        onglet: o.onglet,
                        place: o.dernier_geste,
                        en_vol: Vec::new(),
                        accuse: o.dernier_geste,
                    })
                    .collect(),
                // Ce que la partie reprise sait, et non ce que l'autorité morte avait
                // seulement placé.
                annonce: partie.presence(i.appareil),
                capacites: Capacites::AUCUNE,
                aller_retour: AllerRetour::default(),
                derniere_nouvelle_us: t_us,
                prochain_bot: 0,
            })
            .collect();
        let journal_ecrit = l.journal.fin();
        let derniere_case = l.suite.prochain.saturating_sub(1);
        let mut j = EnJeu::new(
            partie,
            l.ouverture,
            fiches,
            Ecrivain::new(self.reglages.salle, l.suite),
            journal_ecrit,
        );
        j.indice_du_point = l.point.indice();
        j.dernier_point_pas = l.point.pas().numero();
        j.venue = Venue::DeLaGarde { derniere_case };
        j.marques.clone_from(&l.registre.marques);
        j.ecartes.clone_from(&l.registre.ecartes);
        self.entrer_en_jeu(t_us, j, tampon);
    }

    fn entrer_en_jeu(&mut self, t_us: u64, j: EnJeu<F>, tampon: Vec<Vec<u8>>) {
        let places = j.partie.declaration().places.appareils_max();
        self.phase = Phase::EnJeu(Box::new(j));
        self.vers_relais(&VersRelais::Limites {
            liens_max: places.saturating_add(LIENS_EN_PLUS),
        });
        for o in tampon {
            if let Ok(m) = DuRelais::decoder(&o) {
                self.d_un_appareil(t_us, &m);
            }
        }
    }

    fn case_ecrite(&mut self, case: Case) {
        let Phase::EnJeu(j) = &mut self.phase else {
            return;
        };
        let effacer = j.ecrivain.ecrit(case);
        let ecrit = j.ecrivain.dernier_ecrit();
        if let Some(avant) = effacer {
            self.vers_garde(&VersRelais::Effacer { avant });
        }
        let Phase::EnJeu(j) = &mut self.phase else {
            return;
        };
        if effacer.is_some() {
            let _: usize = j.partie.tronquer_journal_avant(j.indice_du_point);
        }
        // Les gestes de ces cases sont écrits : on les accuse (SAL-56).
        let mut accuses = Vec::new();
        for f in &mut j.fiches {
            for o in &mut f.onglets {
                o.en_vol
                    .retain(|e| e.case.is_none_or(|c| ecrit.is_none_or(|n| c > n)));
                let e = o.ecrit_si(ecrit);
                if e > o.accuse {
                    o.accuse = e;
                    if let Some(l) = f.lien {
                        accuses.push((l, o.onglet, e));
                    }
                }
            }
        }
        for (lien, onglet, jusqu_a) in accuses {
            let Phase::EnJeu(j) = &mut self.phase else {
                return;
            };
            let courant = matches!(
                j.liens.get(&lien),
                Some(EtatLien::Chiffre { accueil: Some(a), .. }) if a.onglet == onglet
            );
            if courant {
                self.comptes.accuses = self.comptes.accuses.saturating_add(1);
                self.envoyer(lien, Classe::Fiable, &VersAppareil::Accuse { jusqu_a });
            }
        }
    }

    // -----------------------------------------------------------------------
    // Les appareils
    // -----------------------------------------------------------------------

    fn jeu(&mut self) -> Option<&mut EnJeu<F>> {
        match &mut self.phase {
            Phase::EnJeu(j) => Some(j),
            Phase::Demarrage { .. } | Phase::Arretee(_) => None,
        }
    }

    /// Scelle et envoie un message à un appareil par son lien.
    fn envoyer(&mut self, lien: Lien, classe: Classe, m: &VersAppareil<'_>) {
        let Ok(clair) = m.encoder() else {
            return;
        };
        let Some(j) = self.jeu() else {
            return;
        };
        let Some(EtatLien::Chiffre { session, .. }) = j.liens.get_mut(&lien) else {
            return;
        };
        match session.sceller(classe, &clair) {
            Ok(trame) => self.vers_relais(&VersRelais::Vers {
                lien,
                classe,
                trame: &trame,
            }),
            Err(_) => self.comptes.trop_longs = self.comptes.trop_longs.saturating_add(1),
        }
    }

    /// Coupe un lien : il ne dira plus rien (SAL-21).
    fn couper(&mut self, lien: Lien, raison: RaisonCoupure) {
        self.vers_relais(&VersRelais::Couper { lien, raison });
        if let Some(j) = self.jeu() {
            j.liens.insert(lien, EtatLien::Fini);
            for f in &mut j.fiches {
                if f.lien == Some(lien) {
                    f.lien = None;
                }
            }
        }
    }

    /// Refuse, d'un message final, puis coupe.
    fn refuser(&mut self, lien: Lien, r: RefusSalle, raison: RaisonCoupure) {
        self.envoyer(lien, Classe::Fiable, &VersAppareil::Refus(r));
        self.couper(lien, raison);
    }

    fn arriver(&mut self, t_us: u64, a: Arrivee) -> Option<Placement> {
        let j = self.jeu()?;
        let recu = j.horloge.instant(t_us);
        match j.partie.arriver(a, recu) {
            Ok(p) => Some(p),
            Err(p) => {
                self.arreter(Arret::Panne(p));
                None
            }
        }
    }

    fn d_un_appareil(&mut self, t_us: u64, m: &DuRelais<'_>) {
        match m {
            DuRelais::Ouvert { lien, .. } => {
                if let Some(j) = self.jeu() {
                    j.liens
                        .entry(*lien)
                        .or_insert(EtatLien::Ouvert { depuis_us: t_us });
                }
            }
            DuRelais::De {
                lien,
                classe,
                trame,
                ..
            } => self.trame(t_us, *lien, *classe, trame),
            DuRelais::Ferme { lien, .. } => self.ferme(t_us, *lien),
            DuRelais::Encombre { .. }
            | DuRelais::Accueil { .. }
            | DuRelais::Refus(_)
            | DuRelais::Pour { .. }
            | DuRelais::AutoriteAbsente
            | DuRelais::AutoritePresente
            | DuRelais::Echo { .. }
            | DuRelais::Ecrit { .. }
            | DuRelais::Cases(_)
            | DuRelais::Lu { .. } => {}
        }
    }

    fn ferme(&mut self, t_us: u64, lien: Lien) {
        let Some(j) = self.jeu() else {
            return;
        };
        let _: Option<EtatLien> = j.liens.remove(&lien);
        let mut parti = None;
        for f in &mut j.fiches {
            if f.lien == Some(lien) {
                f.lien = None;
                if f.annonce == Some(Oui::Oui) {
                    f.annonce = Some(Oui::Non);
                    parti = Some(f.appareil);
                }
            }
        }
        if let Some(appareil) = parti {
            let _: Option<Placement> = self.arriver(
                t_us,
                Arrivee::Presence(Presence::Depart {
                    appareil,
                    cause: CauseDepart::Fermeture,
                }),
            );
        }
    }

    fn trame(&mut self, t_us: u64, lien: Lien, classe: Classe, trame: &[u8]) {
        let Some(j) = self.jeu() else {
            return;
        };
        let etat = j
            .liens
            .entry(lien)
            .or_insert(EtatLien::Ouvert { depuis_us: t_us });
        match etat {
            EtatLien::Fini => {}
            EtatLien::Ouvert { .. } => {
                if trame.len() != TAILLE_POIGNEE {
                    self.comptes.trames_illisibles =
                        self.comptes.trames_illisibles.saturating_add(1);
                    return self.couper(lien, RaisonCoupure::Illisible);
                }
                match repondre(&self.cles.paire, &self.cles.secret, trame, &mut self.hasard) {
                    Ok((session, second)) => {
                        // Une session non confirmée : rien n'en dépend encore (SAL-17).
                        if let Some(j) = self.jeu() {
                            j.liens.insert(
                                lien,
                                EtatLien::Chiffre {
                                    session: Box::new(session),
                                    depuis_us: t_us,
                                    accueil: None,
                                },
                            );
                        }
                        self.vers_relais(&VersRelais::Vers {
                            lien,
                            classe: Classe::Fiable,
                            trame: &second,
                        });
                    }
                    // Un autre secret d'entrée : un invité d'une autre soirée.
                    Err(_) => self.couper(lien, RaisonCoupure::Refuse),
                }
            }
            EtatLien::Chiffre { session, .. } => {
                if let Ok(clair) = session.ouvrir(classe, trame) {
                    self.message(t_us, lien, &clair);
                } else {
                    self.comptes.trames_illisibles =
                        self.comptes.trames_illisibles.saturating_add(1);
                    self.couper(lien, RaisonCoupure::Illisible);
                }
            }
        }
    }

    fn accueil_de(&mut self, lien: Lien) -> Option<(Appareil, u64)> {
        let j = self.jeu()?;
        match j.liens.get(&lien) {
            Some(EtatLien::Chiffre {
                accueil: Some(a), ..
            }) => Some((a.appareil, a.onglet)),
            Some(
                EtatLien::Chiffre { accueil: None, .. } | EtatLien::Ouvert { .. } | EtatLien::Fini,
            )
            | None => None,
        }
    }

    fn message(&mut self, t_us: u64, lien: Lien, clair: &[u8]) {
        let Ok(m) = VersAutorite::decoder(clair) else {
            return match protocole_d_un_bonjour(clair) {
                Some(p) if p != PROTOCOLE => self.refuser(
                    lien,
                    RefusSalle::Protocole {
                        autorite: PROTOCOLE,
                    },
                    RaisonCoupure::Refuse,
                ),
                Some(_) | None => {
                    self.comptes.trames_illisibles =
                        self.comptes.trames_illisibles.saturating_add(1);
                    self.couper(lien, RaisonCoupure::Illisible);
                }
            };
        };
        let accueilli = self.accueil_de(lien);
        match (m, accueilli) {
            (
                VersAutorite::Bonjour {
                    protocole,
                    jeton,
                    onglet,
                    capacites,
                    ..
                },
                _,
            ) => self.bonjour(t_us, lien, protocole, &jeton, onglet, capacites),
            (_, None) => {
                // Rien avant le bonjour (SAL-17).
                self.couper(lien, RaisonCoupure::Illisible);
            }
            (
                VersAutorite::Geste {
                    onglet,
                    numero,
                    octets,
                    fait,
                    vu,
                    ..
                },
                Some((appareil, o)),
            ) => {
                self.nouvelle(t_us, appareil);
                if onglet == o {
                    self.geste(t_us, appareil, onglet, numero, (octets, fait, vu));
                }
            }
            (
                VersAutorite::Tenu {
                    numero,
                    octets,
                    fait,
                },
                Some((appareil, _)),
            ) => {
                self.nouvelle(t_us, appareil);
                let demi = self.demi(appareil);
                let _: Option<Placement> = self.arriver(
                    t_us,
                    Arrivee::Tenu {
                        de: appareil,
                        octets: octets.to_vec(),
                        numero,
                        fait,
                        demi_aller_retour: demi,
                    },
                );
            }
            (VersAutorite::Echo { t_us: t, echo_us }, Some((appareil, _))) => {
                self.nouvelle(t_us, appareil);
                self.echo(t_us, lien, appareil, t, echo_us);
            }
            (VersAutorite::ModuleRecu { .. }, Some((appareil, _))) => {
                self.nouvelle(t_us, appareil);
            }
        }
    }

    fn fiche(&mut self, appareil: Appareil) -> Option<&mut Fiche> {
        self.jeu()?
            .fiches
            .iter_mut()
            .find(|f| f.appareil == appareil)
    }

    fn demi(&mut self, appareil: Appareil) -> jeu_abi::Duree {
        self.fiche(appareil)
            .map_or(jeu_abi::Duree::ZERO, |f| f.aller_retour.demi())
    }

    /// Un appareil a donné signe de vie : s'il était dit parti par son silence, il
    /// revient.
    fn nouvelle(&mut self, t_us: u64, appareil: Appareil) {
        let Some(f) = self.fiche(appareil) else {
            return;
        };
        f.derniere_nouvelle_us = t_us;
        if f.annonce == Some(Oui::Non) {
            f.annonce = Some(Oui::Oui);
            let capacites = f.capacites;
            let _: Option<Placement> = self.arriver(
                t_us,
                Arrivee::Presence(Presence::Retour {
                    appareil,
                    capacites,
                }),
            );
        }
    }

    fn echo(&mut self, t_us: u64, lien: Lien, appareil: Appareil, t: u64, echo_us: u64) {
        let Some(j) = self.jeu() else {
            return;
        };
        let maintenant = j.horloge.partie_us(t_us);
        if echo_us != 0 {
            // La réponse de l'appareil à notre écho : l'aller-retour, mesuré ici.
            if let Some(f) = self.fiche(appareil) {
                f.aller_retour.noter(echo_us, maintenant);
            }
            return;
        }
        self.envoyer(
            lien,
            Classe::Fiable,
            &VersAppareil::Echo {
                t_us: maintenant,
                echo_us: t,
            },
        );
    }

    #[expect(
        clippy::too_many_lines,
        reason = "le bonjour se lit d'un trait : protocole, inscription, remplacement, présence, accueil"
    )]
    fn bonjour(
        &mut self,
        t_us: u64,
        lien: Lien,
        protocole: u32,
        jeton: &[u8; 32],
        onglet: u64,
        capacites: Capacites,
    ) {
        if protocole != PROTOCOLE {
            return self.refuser(
                lien,
                RefusSalle::Protocole {
                    autorite: PROTOCOLE,
                },
                RaisonCoupure::Refuse,
            );
        }
        let e = empreinte_jeton(jeton);
        let mut neuf = [0u8; 8];
        self.hasard.remplir(&mut neuf);
        let Some(j) = self.jeu() else {
            return;
        };
        let places = usize::from(j.partie.declaration().places.appareils_max());
        // Un bonjour répété sur le même lien (SAL-29) : le même accueil, rien d'autre.
        let deja = matches!(
            j.liens.get(&lien),
            Some(EtatLien::Chiffre {
                accueil: Some(_),
                ..
            })
        );
        let indice = if let Some(i) = j.fiches.iter().position(|f| f.jeton == e) {
            i
        } else {
            {
                let presents = j
                    .fiches
                    .iter()
                    .filter(|f| f.annonce == Some(Oui::Oui) || f.lien.is_some())
                    .count();
                if presents >= places {
                    return self.refuser(lien, RefusSalle::Pleine, RaisonCoupure::Refuse);
                }
                let mut n = u64::from_le_bytes(neuf);
                while n == 0 || j.fiches.iter().any(|f| f.appareil.fil() == n) {
                    n = n.wrapping_add(1);
                }
                j.fiches.push(Fiche {
                    jeton: e,
                    appareil: Appareil::du_fil(n),
                    lien: None,
                    onglets: Vec::new(),
                    annonce: None,
                    capacites,
                    aller_retour: AllerRetour::default(),
                    derniere_nouvelle_us: t_us,
                    prochain_bot: 0,
                });
                j.fiches.len().saturating_sub(1)
            }
        };
        let Some(f) = j.fiches.get(indice) else {
            return;
        };
        let appareil = f.appareil;
        let ancien = f.lien;
        // Le lien le plus récent gagne, dans l'ordre où le relais les a ouverts (SAL-19).
        if let Some(ancien) = ancien
            && ancien != lien
        {
            self.comptes.remplaces = self.comptes.remplaces.saturating_add(1);
            if lien < ancien {
                return self.refuser(lien, RefusSalle::Remplace, RaisonCoupure::Remplace);
            }
            self.refuser(ancien, RefusSalle::Remplace, RaisonCoupure::Remplace);
        }
        let Some(j) = self.jeu() else {
            return;
        };
        let session = match j.liens.get(&lien) {
            Some(EtatLien::Chiffre {
                accueil: Some(a), ..
            }) if deja => a.session,
            Some(_) | None => {
                let s = j.session_suivante;
                j.session_suivante = s.wrapping_add(1);
                s
            }
        };
        let presence = {
            let Some(f) = j.fiches.get_mut(indice) else {
                return;
            };
            f.lien = Some(lien);
            f.derniere_nouvelle_us = t_us;
            let p = match f.annonce {
                None => Some(Presence::Arrivee {
                    appareil,
                    role: RoleAppareil::Telephone,
                    capacites,
                }),
                Some(Oui::Non) => Some(Presence::Retour {
                    appareil,
                    capacites,
                }),
                Some(Oui::Oui) if f.capacites != capacites => Some(Presence::Capacites {
                    appareil,
                    capacites,
                }),
                Some(Oui::Oui) => None,
            };
            f.annonce = Some(Oui::Oui);
            f.capacites = capacites;
            p
        };
        let dernier_geste = j
            .fiches
            .get(indice)
            .and_then(|f| f.onglets.iter().find(|o| o.onglet == onglet))
            .map_or(0, |o| o.place);
        let dernier_tenu = j.partie.numero_tenu(appareil);
        let pas = j.partie.definitif();
        let heure = j.horloge.partie_us(t_us);
        let limites = j.partie.declaration().limites;
        if let Some(EtatLien::Chiffre { accueil, .. }) = j.liens.get_mut(&lien) {
            *accueil = Some(Accueilli {
                appareil,
                onglet,
                session,
                // La vue entière part au prochain tour (SAL-28).
                vue: None,
                vue_depuis: pas,
                prochain_echo_us: t_us,
            });
        }
        if let Some(p) = presence.filter(|_| !deja) {
            let _: Option<Placement> = self.arriver(t_us, Arrivee::Presence(p));
        }
        self.envoyer(
            lien,
            Classe::Fiable,
            &VersAppareil::Accueil {
                session,
                dernier_geste,
                dernier_tenu,
                pas,
                heure,
                module: None,
                limites: LimitesSalle {
                    geste: limites.geste,
                    file: limites.file,
                },
            },
        );
        // Ce qui est déjà écrit s'accuse aussitôt : l'accusé d'avant a pu se perdre avec
        // une autorité morte, et l'appareil le garderait sinon pour toujours (SAL-56).
        let Some(j) = self.jeu() else {
            return;
        };
        let ecrit = j.ecrivain.dernier_ecrit();
        let jusqu_a = j
            .fiches
            .get_mut(indice)
            .and_then(|f| f.onglets.iter_mut().find(|o| o.onglet == onglet))
            .map(|o| {
                o.accuse = o.accuse.max(o.ecrit_si(ecrit));
                o.accuse
            });
        if let Some(jusqu_a) = jusqu_a.filter(|&n| n > 0) {
            self.comptes.accuses = self.comptes.accuses.saturating_add(1);
            self.envoyer(lien, Classe::Fiable, &VersAppareil::Accuse { jusqu_a });
        }
    }

    fn geste(
        &mut self,
        t_us: u64,
        appareil: Appareil,
        onglet: u64,
        numero: u64,
        (octets, fait, vu): (&[u8], jeu_abi::Instant, Pas),
    ) {
        let demi = self.demi(appareil);
        let Some(f) = self.fiche(appareil) else {
            return;
        };
        let o = if let Some(i) = f.onglets.iter().position(|o| o.onglet == onglet) {
            i
        } else {
            {
                // Un onglet ne compte qu'une fois qu'il a livré quelque chose (SAL-37).
                if f.onglets.len() >= ONGLETS_RETENUS {
                    let _: OngletVivant = f.onglets.remove(0);
                }
                f.onglets.push(OngletVivant {
                    onglet,
                    place: 0,
                    en_vol: Vec::new(),
                    accuse: 0,
                });
                f.onglets.len().saturating_sub(1)
            }
        };
        let Some(ov) = f.onglets.get_mut(o) else {
            return;
        };
        if numero <= ov.place {
            self.comptes.gestes_en_double = self.comptes.gestes_en_double.saturating_add(1);
            return;
        }
        if numero != ov.place.saturating_add(1) {
            self.comptes.gestes_hors_suite = self.comptes.gestes_hors_suite.saturating_add(1);
            return;
        }
        ov.place = numero;
        let id = id_geste(appareil, onglet, numero);
        if self.a_ecarter == Some(id) {
            return self.ecarter(appareil, numero, id);
        }
        let place = self.arriver(
            t_us,
            Arrivee::Geste {
                de: appareil,
                octets: octets.to_vec(),
                fait,
                vu,
                id,
                demi_aller_retour: demi,
            },
        );
        match place {
            Some(Placement::Place { .. }) => {
                self.comptes.gestes_places = self.comptes.gestes_places.saturating_add(1);
                if let Some(ov) = self
                    .fiche(appareil)
                    .and_then(|f| f.onglets.iter_mut().find(|o| o.onglet == onglet))
                {
                    ov.en_vol.push(EnVol {
                        numero,
                        id,
                        case: None,
                    });
                }
            }
            Some(
                Placement::Malforme(_)
                | Placement::HorsScene
                | Placement::Perimee
                | Placement::FilePleine
                | Placement::Ecartee,
            ) => {
                self.comptes.gestes_refuses = self.comptes.gestes_refuses.saturating_add(1);
            }
            None => {}
        }
    }

    /// Écarte un geste qui a bloqué deux Workers (P39) : il n'entre pas dans la partie,
    /// son appareil le retire de sa file (`Ecarte`), le registre le compte comme placé
    /// (la prochaine case l'écrit, même vide), et la page le dit comme une panne de
    /// l'outil, l'entrée nommée : le carburant aurait dû l'arrêter (HIS-06).
    fn ecarter(&mut self, appareil: Appareil, numero: u64, id: IdGeste) {
        self.a_ecarter = None;
        self.comptes.gestes_ecartes = self.comptes.gestes_ecartes.saturating_add(1);
        if let Some(j) = self.jeu() {
            j.a_ecrire = true;
            if j.ecartes.len() >= MARQUES_MAX {
                let _: u64 = j.ecartes.remove(0);
            }
            j.ecartes.push(id.fil());
        }
        if let Some(lien) = self.fiche(appareil).and_then(|f| f.lien) {
            self.envoyer(lien, Classe::Fiable, &VersAppareil::Ecarte { numero });
        }
        let detail = format!(
            "entrée écartée après deux arrêts muraux : le geste {:016x} (appareil {}, numéro {numero}) ; le carburant aurait dû l'arrêter",
            id.fil(),
            appareil.fil()
        );
        self.vers_page(&VersPage::Panne(RapportPanne {
            quoi: PanneSalle::Outil,
            detail: &detail,
        }));
    }

    // -----------------------------------------------------------------------
    // La page
    // -----------------------------------------------------------------------

    /// Un message de la page de l'écran.
    pub fn page(&mut self, t_us: u64, octets: &[u8]) {
        let Ok(m) = DeLaPage::decoder(octets) else {
            return;
        };
        match m {
            DeLaPage::Ecran { quoi, vu, .. } => {
                let quoi = ecran_possede(&quoi);
                let _: Option<Placement> = self.arriver(t_us, Arrivee::Ecran { quoi, vu });
            }
            DeLaPage::Image { .. } => self.reveiller(t_us),
            DeLaPage::Reponse { id, quoi } => {
                let reponse = reponse_possedee(&quoi);
                let _: Option<Placement> = self.arriver(t_us, Arrivee::Reponse { id, reponse });
            }
            DeLaPage::Commande(CommandeEcran::Reprendre { ecarter }) => {
                // Un Worker neuf, après deux arrêts muraux pendant le même geste : ce
                // geste sera écarté quand son appareil le renverra (P39). Pendant le
                // démarrage, il n'y a rien à reprendre.
                if ecarter.is_some() {
                    self.a_ecarter = ecarter;
                }
                if let Some(j) = self.jeu()
                    && ecarter.is_none()
                    && let Err(p) = j.partie.repartir()
                {
                    self.arreter(Arret::Panne(p));
                }
            }
            DeLaPage::Commande(CommandeEcran::Marque) => self.marquer(),
            DeLaPage::Commande(CommandeEcran::ExporterLaCle) => {
                let g = *self.cles.garde.octets();
                self.vers_page(&VersPage::Cle(g));
            }
            // Les fichiers, changer de jeu et une soirée neuve viennent avec l'écran (lots
            // 10b et 11) ; `Demarrer`, `Figer` et `Retarder` sont l'affaire de la coque.
            DeLaPage::Fichiers(_)
            | DeLaPage::Demarrer { .. }
            | DeLaPage::Commande(
                CommandeEcran::ChangerDeJeu { .. }
                | CommandeEcran::NouvelleSoiree
                | CommandeEcran::Figer
                | CommandeEcran::Retarder { .. },
            ) => {}
        }
    }

    /// F8 : une marque au journal, à la fin du dernier pas joué (SAL-162). Elle part à
    /// la garde avec la prochaine case, même sans ligne nouvelle.
    fn marquer(&mut self) {
        let Some(j) = self.jeu() else {
            return;
        };
        let indice = u64::try_from(j.partie.fin_du_dernier_pas()).unwrap_or(u64::MAX);
        let pas = j.partie.definitif();
        if j.marques.len() >= MARQUES_MAX {
            let _: Marque = j.marques.remove(0);
        }
        j.marques.push(Marque { indice, pas });
        j.a_ecrire = true;
        self.comptes.marques = self.comptes.marques.saturating_add(1);
    }

    /// Le retard du réveil qui vient, mesuré par la coque, en millisecondes : il part au
    /// battement de la page (§ 2.3, M6).
    pub const fn noter_retard(&mut self, retard_ms: u32) {
        self.retard_ms = retard_ms;
    }

    // -----------------------------------------------------------------------
    // Le tour
    // -----------------------------------------------------------------------

    /// Un réveil : le signal d'une image, ou la minuterie. La partie avance jusqu'au pas
    /// que l'horloge a atteint, d'un tour borné (P7) ; puis les vues, les coups du banc,
    /// les échos, les silences, la garde.
    pub fn reveiller(&mut self, t_us: u64) {
        let debut = self.heure_du_chrono();
        self.durees = DureesDuTour::default();
        let Some(j) = self.jeu() else {
            return;
        };
        let avant = j.partie.definitif();
        let cible = j.horloge.pas(t_us);
        let avancee = match j.partie.avancer_borne(cible, ARRIERES_PAR_TOUR) {
            Ok(a) => a,
            Err(p) => return self.arreter(Arret::Panne(p)),
        };
        self.traiter(&avancee);
        let avance = self.heure_du_chrono();
        self.vues(t_us, avant);
        let vu = self.heure_du_chrono();
        if self.reglages.banc == Oui::Oui {
            self.banc(t_us);
        }
        self.echos_et_silences(t_us);
        let avant_la_garde = self.heure_du_chrono();
        self.garder(t_us);
        let garde = self.heure_du_chrono();
        self.etat_salle();
        if self.chrono.is_some() {
            self.durees.avancer = avance - debut;
            self.durees.vues = vu - avance;
            self.durees.garde = garde - avant_la_garde;
            self.durees.total = self.heure_du_chrono() - debut;
        }
        let retard_ms = core::mem::take(&mut self.retard_ms);
        if let Some(j) = self.jeu() {
            let pas = j.partie.definitif();
            self.vers_page(&VersPage::Battement { pas, retard_ms });
        }
    }

    fn traiter(&mut self, a: &Avancee) {
        for (appareil, octets) in &a.avis {
            if let Some(lien) = self.fiche(*appareil).and_then(|f| f.lien) {
                self.envoyer(lien, Classe::Fiable, &VersAppareil::Avis { octets });
            }
        }
        for (id, d) in &a.lancer {
            let quoi = demande_fil(d);
            self.vers_page(&VersPage::Demande { id: *id, quoi });
        }
        for appareil in a.oublier.iter().chain(&a.retirer) {
            // Il reviendra comme un neuf : une arrivée, et non un retour.
            if let Some(f) = self.fiche(*appareil) {
                f.annonce = None;
            }
        }
    }

    fn vues(&mut self, t_us: u64, avant: Pas) {
        let Some(j) = self.jeu() else {
            return;
        };
        let definitif = j.partie.definitif();
        let accueillis: Vec<(Lien, Appareil, Option<[u8; 32]>)> = j
            .liens
            .iter()
            .filter_map(|(l, e)| match e {
                EtatLien::Chiffre {
                    accueil: Some(a), ..
                } => Some((*l, a.appareil, a.vue)),
                EtatLien::Chiffre { accueil: None, .. }
                | EtatLien::Ouvert { .. }
                | EtatLien::Fini => None,
            })
            .collect();
        let due = accueillis.iter().any(|(_, _, v)| v.is_none());
        if definitif == avant && !due && j.scene.is_some() {
            return;
        }
        let mut regards = vec![RegardFil::Ecran];
        regards.extend(
            accueillis
                .iter()
                .map(|(_, a, _)| RegardFil::Appareil(a.fil())),
        );
        let vues = match j.partie.regarder(&regards) {
            Ok(v) => v,
            Err(p) => return self.arreter(Arret::Panne(p)),
        };
        for v in vues {
            let h = *blake3::hash(&v.octets).as_bytes();
            match v.regard {
                RegardFil::Ecran => {
                    let Some(j) = self.jeu() else {
                        return;
                    };
                    if j.scene == Some(h) {
                        continue;
                    }
                    j.scene = Some(h);
                    j.scene_pas = v.pas;
                    j.scene_depuis = definitif;
                    let t = t_us;
                    self.vers_page(&VersPage::Scene {
                        pas: v.pas,
                        t_us: t,
                        octets: &v.octets,
                    });
                }
                RegardFil::Appareil(n) => {
                    let a = Appareil::du_fil(n);
                    for (lien, _, _) in accueillis.iter().filter(|(_, x, _)| *x == a) {
                        let Some(j) = self.jeu() else {
                            return;
                        };
                        let Some(EtatLien::Chiffre {
                            accueil: Some(acc), ..
                        }) = j.liens.get_mut(lien)
                        else {
                            continue;
                        };
                        if acc.vue == Some(h) {
                            continue;
                        }
                        acc.vue = Some(h);
                        acc.vue_depuis = definitif;
                        self.comptes.vues = self.comptes.vues.saturating_add(1);
                        self.envoyer(
                            *lien,
                            Classe::Remplacable(CANAL_VUE),
                            &VersAppareil::Vue {
                                pas: v.pas,
                                montre: &v.octets,
                            },
                        );
                    }
                }
            }
        }
    }

    fn banc(&mut self, t_us: u64) {
        let Some(j) = self.jeu() else {
            return;
        };
        let pas = j.partie.definitif().numero();
        let mut a_jouer = Vec::new();
        for f in &mut j.fiches {
            let Some(lien) = f.lien else {
                continue;
            };
            if pas < f.prochain_bot {
                continue;
            }
            f.prochain_bot = pas.saturating_add(PERIODE_BOT);
            let depuis = match j.liens.get(&lien) {
                Some(EtatLien::Chiffre {
                    accueil: Some(a), ..
                }) => pas.saturating_sub(a.vue_depuis.numero()),
                Some(_) | None => continue,
            };
            a_jouer.push((lien, f.appareil, depuis));
        }
        let ecran = pas >= j.prochain_bot_ecran;
        if ecran {
            j.prochain_bot_ecran = pas.saturating_add(PERIODE_BOT);
        }
        for (lien, appareil, depuis) in a_jouer {
            for coup in self.coups(RegardFil::Appareil(appareil.fil()), depuis) {
                let banc = match &coup {
                    CoupPossede::Geste(o) => crate::fil::CoupBanc::Geste(o),
                    CoupPossede::Tenu(o) => crate::fil::CoupBanc::Tenu(o),
                    CoupPossede::Ecran(_) => continue,
                };
                self.comptes.coups_banc = self.comptes.coups_banc.saturating_add(1);
                self.envoyer(lien, Classe::Fiable, &VersAppareil::Banc(banc));
            }
        }
        if ecran {
            let Some(j) = self.jeu() else {
                return;
            };
            let depuis = pas.saturating_sub(j.scene_depuis.numero());
            let vu = j.scene_pas;
            for coup in self.coups(RegardFil::Ecran, depuis) {
                if let CoupPossede::Ecran(quoi) = coup {
                    self.comptes.coups_ecran = self.comptes.coups_ecran.saturating_add(1);
                    let _: Option<Placement> = self.arriver(t_us, Arrivee::Ecran { quoi, vu });
                }
            }
        }
    }

    fn coups(&mut self, regard: RegardFil, depuis: u64) -> Vec<CoupPossede> {
        let mut g = [0u8; 8];
        self.hasard.remplir(&mut g);
        let Some(j) = self.jeu() else {
            return Vec::new();
        };
        let lu = match j
            .partie
            .bot(regard, StyleFil::Sage, u64::from_le_bytes(g), depuis)
        {
            Ok(l) => l,
            Err(p) => {
                self.arreter(Arret::Panne(p));
                return Vec::new();
            }
        };
        let Some(octets) = lu else {
            return Vec::new();
        };
        let Ok(Retour::Bot(coups)) = decoder::<Retour<'_>>(&octets) else {
            return Vec::new();
        };
        coups.iter().filter_map(CoupPossede::de).collect()
    }

    fn echos_et_silences(&mut self, t_us: u64) {
        let poignee = self.reglages.poignee_max_us;
        let Some(j) = self.jeu() else {
            return;
        };
        let maintenant = j.horloge.partie_us(t_us);
        let battement = u64::from(BATTEMENT_MS).saturating_mul(1000);
        let silence = u64::from(SILENCE_MAX_MS).saturating_mul(1000);
        let mut echos = Vec::new();
        let mut couper = Vec::new();
        for (lien, e) in &mut j.liens {
            match e {
                EtatLien::Chiffre {
                    accueil: Some(a), ..
                } => {
                    if t_us >= a.prochain_echo_us {
                        a.prochain_echo_us = t_us.saturating_add(battement);
                        echos.push(*lien);
                    }
                }
                EtatLien::Chiffre {
                    accueil: None,
                    depuis_us,
                    ..
                }
                | EtatLien::Ouvert { depuis_us } => {
                    if t_us.saturating_sub(*depuis_us) > poignee {
                        couper.push(*lien);
                    }
                }
                EtatLien::Fini => {}
            }
        }
        let mut silencieux = Vec::new();
        for f in &mut j.fiches {
            if f.annonce == Some(Oui::Oui) && t_us.saturating_sub(f.derniere_nouvelle_us) > silence
            {
                f.annonce = Some(Oui::Non);
                silencieux.push(f.appareil);
            }
        }
        for lien in echos {
            self.envoyer(
                lien,
                Classe::Fiable,
                &VersAppareil::Echo {
                    t_us: maintenant,
                    echo_us: 0,
                },
            );
        }
        for lien in couper {
            self.comptes.poignees_inachevees = self.comptes.poignees_inachevees.saturating_add(1);
            self.couper(lien, RaisonCoupure::PoigneeInachevee);
        }
        for appareil in silencieux {
            let _: Option<Placement> = self.arriver(
                t_us,
                Arrivee::Presence(Presence::Depart {
                    appareil,
                    cause: CauseDepart::Silence,
                }),
            );
        }
    }

    // -----------------------------------------------------------------------
    // La garde
    // -----------------------------------------------------------------------

    fn garder(&mut self, t_us: u64) {
        let tranche = self.reglages.tranche_du_point;
        let debut = self.heure_du_chrono();
        let Some(j) = self.jeu() else {
            return;
        };
        // Un point se scelle : une tranche par réveil, et rien d'autre ne s'écrit avant
        // qu'il soit fini et envoyé.
        if let Some((p, _)) = j.point_en_cours.as_mut() {
            if p.avancer(tranche) {
                self.finir_le_point();
            }
            if self.chrono.is_some() {
                self.durees.sceller = self.heure_du_chrono() - debut;
            }
            return;
        }
        let Some(j) = self.jeu() else {
            return;
        };
        let fin = j.partie.fin_du_dernier_pas();
        let point_du = j.partie.pas_en_cours().is_none()
            && j.partie.definitif().numero()
                >= j.dernier_point_pas
                    .saturating_add(self.reglages.point_tous_les);
        if point_du {
            self.ecrire_le_point(t_us, Oui::Non);
            return;
        }
        let Some(j) = self.jeu() else {
            return;
        };
        if (fin > j.journal_ecrit || j.a_ecrire)
            && t_us.saturating_sub(j.dernier_journal_us) >= self.reglages.journal_tous_les_us
        {
            self.ecrire_le_journal(t_us);
        }
    }

    /// Finit le pas en cours et écrit tout le journal à la garde, sans avancer : avant
    /// d'arrêter l'autorité proprement, et pour qu'un test compare la partie reprise de la
    /// garde à celle-ci.
    pub fn ecrire_la_garde(&mut self, t_us: u64) {
        let Some(j) = self.jeu() else {
            return;
        };
        match j.partie.finir_le_pas() {
            Ok(a) => self.traiter(&a),
            Err(p) => return self.arreter(Arret::Panne(p)),
        }
        self.ecrire_le_journal(t_us);
    }

    /// Écrit les lignes du journal jusqu'à la fin du dernier pas, s'il y en a.
    fn ecrire_le_journal(&mut self, t_us: u64) {
        // Un point en train de se sceller précède dans la chaîne toute case de journal :
        // il se finit d'abord, d'un coup.
        self.finir_le_point();
        let Phase::EnJeu(j) = &mut self.phase else {
            return;
        };
        let fin = j.partie.fin_du_dernier_pas();
        if fin <= j.journal_ecrit && !j.a_ecrire {
            return;
        }
        j.a_ecrire = false;
        let base = j.partie.journal().base();
        let lignes = j.partie.journal().lignes();
        let debut = j.journal_ecrit.saturating_sub(base);
        let Some(morceau) = lignes.get(debut..fin.saturating_sub(base)) else {
            return;
        };
        let n = j.ecrivain.suite().prochain;
        // Les gestes de ce morceau seront écrits avec lui.
        for l in morceau {
            if let Ligne::Arrivees { arrivees, .. } = l {
                for a in arrivees {
                    if let QuoiPlace::Geste { de, id, .. } = &a.quoi
                        && let Some(f) = j.fiches.iter_mut().find(|f| f.appareil == *de)
                    {
                        for o in &mut f.onglets {
                            for e in &mut o.en_vol {
                                if e.id == *id && e.case.is_none() {
                                    e.case = Some(n);
                                }
                            }
                        }
                    }
                }
            }
        }
        let registre = registre_de(&j.fiches, (&j.marques, &j.ecartes), Some(n));
        let pas = j.partie.journal().pas();
        let ecrite = j.ecrivain.journal(
            &self.cles.garde,
            (j.journal_ecrit, pas, morceau),
            &registre,
            &mut self.hasard,
        );
        j.journal_ecrit = fin;
        j.dernier_journal_us = t_us;
        match ecrite {
            Ok((case, octets)) => {
                self.comptes.cases_journal = self.comptes.cases_journal.saturating_add(1);
                self.vers_garde(&VersRelais::Ecrire {
                    case,
                    octets: &octets,
                });
            }
            Err(_) => self.arreter(Arret::Garde(RefusDeGarde::Scellee(Case::Journal(n)))),
        }
    }

    /// Écrit un point, après les lignes qui le précèdent : son indice est la fin du
    /// journal écrit. `toujours` : même si un pas est en cours (jamais ; à l'ouverture).
    fn ecrire_le_point(&mut self, t_us: u64, toujours: Oui) {
        self.ecrire_le_journal(t_us);
        let Phase::EnJeu(j) = &mut self.phase else {
            return;
        };
        if toujours == Oui::Non && j.partie.pas_en_cours().is_some() {
            return;
        }
        let debut = self.chrono.map_or(0.0, |c| c());
        let Phase::EnJeu(j) = &mut self.phase else {
            return;
        };
        let point = match j.partie.point() {
            Ok(p) => p,
            Err(p) => return self.arreter(Arret::Panne(p)),
        };
        let pris = self.heure_du_chrono();
        let Phase::EnJeu(j) = &mut self.phase else {
            return;
        };
        if point.indice() != j.journal_ecrit {
            // Le point ne tombe pas au bord du journal écrit : il ne s'écrit pas, et le
            // suivant viendra `point_tous_les` pas plus tard (sans reprendre un instantané
            // à chaque tour).
            j.dernier_point_pas = j.partie.definitif().numero();
            self.comptes.points_sautes = self.comptes.points_sautes.saturating_add(1);
            return;
        }
        let n = j.ecrivain.suite().prochain;
        let registre = registre_de(&j.fiches, (&j.marques, &j.ecartes), Some(n));
        let commence = j.ecrivain.commencer_point(
            &self.cles.garde,
            &j.ouverture,
            &point,
            &registre,
            &mut self.hasard,
        );
        j.dernier_point_pas = point.pas().numero();
        match commence {
            Ok(p) => j.point_en_cours = Some((p, point.indice())),
            Err(_) => return self.arreter(Arret::Garde(RefusDeGarde::Scellee(Case::Point(n)))),
        }
        if self.chrono.is_some() {
            self.durees.instantane = pris - debut;
            self.durees.sceller = self.heure_du_chrono() - pris;
            self.durees.point = true;
        }
        // À l'ouverture, le point s'écrit tout de suite ; en partie, il se scelle une
        // tranche par réveil, à partir du suivant (`garder`), pour que ce tour, qui a déjà
        // joué un pas, reste court.
        if toujours == Oui::Oui {
            self.finir_le_point();
        }
    }

    /// Finit d'un coup le point en train de se sceller, s'il y en a un, et l'envoie.
    fn finir_le_point(&mut self) {
        let Phase::EnJeu(j) = &mut self.phase else {
            return;
        };
        let Some((p, indice)) = j.point_en_cours.take() else {
            return;
        };
        let case = p.case();
        match j.ecrivain.finir_point(p) {
            Ok((case, octets)) => {
                j.indice_du_point = indice;
                self.comptes.points = self.comptes.points.saturating_add(1);
                self.vers_garde(&VersRelais::Ecrire {
                    case,
                    octets: &octets,
                });
            }
            Err(_) => self.arreter(Arret::Garde(RefusDeGarde::Scellee(case))),
        }
    }

    fn etat_salle(&mut self) {
        let salle = self.reglages.salle;
        let lien = self.lien;
        let Some(j) = self.jeu() else {
            return;
        };
        let e = EtatSalle {
            salle,
            lien,
            appareils: j
                .fiches
                .iter()
                .map(|f| AppareilVu {
                    appareil: f.appareil,
                    present: f.annonce.unwrap_or(Oui::Non),
                    lien: f.lien,
                })
                .collect(),
            garde: j.ecrivain.dernier_ecrit(),
        };
        if j.dernier_etat.as_ref() == Some(&e) {
            return;
        }
        j.dernier_etat = Some(e.clone());
        self.vers_page(&VersPage::EtatSalle(e));
    }
}

impl<F: Fabrique> EnJeu<F> {
    fn new(
        partie: Partie<F>,
        ouverture: Ouverture,
        fiches: Vec<Fiche>,
        ecrivain: Ecrivain,
        journal_ecrit: usize,
    ) -> EnJeu<F> {
        let horloge = Horloge::de(&ouverture);
        let definitif = partie.definitif();
        EnJeu {
            partie,
            horloge,
            ouverture,
            liens: BTreeMap::new(),
            fiches,
            ecrivain,
            journal_ecrit,
            dernier_journal_us: 0,
            dernier_point_pas: definitif.numero(),
            indice_du_point: 0,
            point_en_cours: None,
            scene: None,
            scene_pas: definitif,
            scene_depuis: definitif,
            prochain_bot_ecran: 0,
            session_suivante: 1,
            dernier_etat: None,
            marques: Vec::new(),
            ecartes: Vec::new(),
            a_ecrire: false,
            venue: Venue::Neuve,
        }
    }
}

/// Le registre tel qu'une case l'écrit si les cases jusqu'à `n` le sont.
fn registre_de(
    fiches: &[Fiche],
    (marques, ecartes): (&[Marque], &[u64]),
    n: Option<u64>,
) -> Registre {
    Registre {
        marques: marques.to_vec(),
        ecartes: ecartes.to_vec(),
        appareils: fiches
            .iter()
            .map(|f| Inscrit {
                jeton: f.jeton,
                appareil: f.appareil,
                onglets: f
                    .onglets
                    .iter()
                    .map(|o| OngletInscrit {
                        onglet: o.onglet,
                        dernier_geste: o.ecrit_si(n),
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// Un coup d'un bot, possédé.
enum CoupPossede {
    Geste(Vec<u8>),
    Tenu(Vec<u8>),
    Ecran(EcranArrivee),
}

impl CoupPossede {
    fn de(c: &CoupFil<'_>) -> Option<CoupPossede> {
        match c {
            CoupFil::Geste(o) => Some(CoupPossede::Geste(o.to_vec())),
            CoupFil::Tenu(o) => Some(CoupPossede::Tenu(o.to_vec())),
            CoupFil::Action(o) => Some(CoupPossede::Ecran(EcranArrivee::Action(o.to_vec()))),
            CoupFil::Touche(t) => Some(CoupPossede::Ecran(EcranArrivee::Touche(*t))),
            CoupFil::Composant(ComposantBotFil::Saisie { champ, texte, fin }) => Some(
                CoupPossede::Ecran(EcranArrivee::Composant(ComposantArrivee::Saisie {
                    champ: *champ,
                    texte: String::from(*texte),
                    fin: *fin,
                })),
            ),
            CoupFil::Composant(ComposantBotFil::Deplace(d)) => Some(CoupPossede::Ecran(
                EcranArrivee::Composant(ComposantArrivee::Deplace(*d)),
            )),
            // Les fichiers passent par le sélecteur de la page (lot 11).
            CoupFil::Composant(ComposantBotFil::Fichiers(_)) | CoupFil::Rien => None,
        }
    }
}

/// Une entrée de l'écran, possédée.
fn ecran_possede(e: &EcranFil<'_>) -> EcranArrivee {
    match e {
        EcranFil::Action(o) => EcranArrivee::Action(o.to_vec()),
        EcranFil::Touche(t) => EcranArrivee::Touche(*t),
        EcranFil::Pointeur(p) => EcranArrivee::Pointeur(*p),
        EcranFil::Composant(ComposantFil::Saisie { champ, texte, fin }) => {
            EcranArrivee::Composant(ComposantArrivee::Saisie {
                champ: *champ,
                texte: String::from(*texte),
                fin: *fin,
            })
        }
        EcranFil::Composant(ComposantFil::Deplace(d)) => {
            EcranArrivee::Composant(ComposantArrivee::Deplace(*d))
        }
        EcranFil::Composant(ComposantFil::Fichiers {
            geste,
            selecteur,
            fichiers,
        }) => EcranArrivee::Composant(ComposantArrivee::Fichiers {
            geste: *geste,
            selecteur: *selecteur,
            fichiers: fichiers.clone(),
        }),
    }
}

/// Une réponse de la page, possédée.
fn reponse_possedee(r: &ReponseFil<'_>) -> ReponseArrivee {
    match r {
        ReponseFil::Image(r) => ReponseArrivee::Image(r.map(<[u8]>::to_vec)),
        ReponseFil::Rendu(r) => {
            ReponseArrivee::Rendu(r.map(|i| (i.octets.to_vec(), i.non_dessinees)))
        }
        ReponseFil::Lu(r) => ReponseArrivee::Lu(r.map(<[u8]>::to_vec)),
        ReponseFil::Fait(r) => ReponseArrivee::Fait(*r),
    }
}

/// Une demande à lancer, sous sa forme sur le fil.
fn demande_fil(d: &DemandeArrivee) -> DemandeFil<'_> {
    match d {
        DemandeArrivee::Image {
            fichier,
            carre,
            format,
        } => DemandeFil::Image {
            fichier: *fichier,
            carre: *carre,
            format: *format,
        },
        DemandeArrivee::Rendu {
            scene,
            echelle,
            format,
        } => DemandeFil::Rendu {
            scene: scene.clone(),
            echelle: *echelle,
            format: *format,
        },
        DemandeArrivee::Lire { fichier, max } => DemandeFil::Lire {
            fichier: *fichier,
            max: *max,
        },
        DemandeArrivee::Telecharger { nom, octets } => DemandeFil::Telecharger { nom, octets },
        DemandeArrivee::Ligne { flux, octets } => DemandeFil::Ligne {
            flux: *flux,
            octets,
        },
    }
}
