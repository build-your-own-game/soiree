//! Le contrat entre le cœur et une instance d'un jeu (INTERFACE.md, § 8.6).
//!
//! Le cœur ne voit d'une instance que ces quatre opérations : appeler, prendre un
//! instantané, le restaurer, empreinter. Il ne connaît ni la mémoire d'un module,
//! ni le type d'un jeu natif. Ce contrat est stable : une instance nouvelle
//! (un interpréteur, un moteur de navigateur) l'implémente sans le changer.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use jeu_abi::SorteAppel;

/// Une instance d'un jeu : un module dans un moteur, ou la copie native.
///
/// # Le contrat
///
/// - **Une seule porte.** [`Instance::appeler`] reçoit un [`jeu_abi::Appel`] encodé
///   et rend un [`jeu_abi::Retour`] encodé, copié hors de l'instance. Le cœur décode
///   lui-même le retour, strictement ; l'instance ne l'interprète pas.
/// - **La sorte vient du cœur.** `sorte` vaut `Appel::sorte()` de l'appel encodé ;
///   l'instance s'en sert pour placer l'entrée (boîte ou tas, § 3.3) et ne la
///   revérifie pas forcément. Une lecture ne change pas l'état (D4).
/// - **Après un [`Echec`], l'instance n'est plus sûre.** Sa mémoire peut être à
///   moitié écrite, sa pile non rendue. Le cœur la [`restaurer`](Instance::restaurer)
///   depuis un instantané, ou la jette pour une [`Fabrique::neuve`] (§ 4.6).
/// - **Même histoire, même empreinte.** Deux instances de la même sorte, de la même
///   fabrique, qui ont reçu les mêmes appels dans le même ordre, ont la même
///   [`empreinte`](Instance::empreinte), octet pour octet. Une empreinte ne se compare
///   qu'entre instances de la même sorte : celle d'un module et celle de la copie
///   native ne mesurent pas la même chose.
/// - **Un instantané rend tout.** Après `restaurer(&i)`, l'instance se comporte
///   exactement comme celle sur laquelle `i` a été pris, empreinte comprise, et la
///   taille de sa mémoire est celle de l'instantané : une instance plus grande est
///   remplacée par une neuve, de façon invisible pour le cœur.
pub trait Instance {
    /// Ce que [`Instance::instantane`] prend et que [`Instance::restaurer`] remet :
    /// la mémoire, les globales et la taille d'un module, ou l'enveloppe clonée de la
    /// copie native. Le cœur le garde dans ses points de reprise sans le lire.
    type Instantane: Clone;

    /// Exécute un appel encodé, sous un budget de carburant (§ 3.4).
    ///
    /// `budget` est le carburant de cette sorte d'appel ; une instance sans
    /// carburant (la copie native) l'ignore. Rend les octets du retour.
    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8], budget: u64) -> Result<Vec<u8>, Echec>;

    /// Le carburant consommé par le dernier appel, faute comprise ; `None` pour une
    /// instance sans carburant. L'épreuve en tire la distribution par sorte d'appel
    /// (E19), et le rapport d'une faute le cite (§ 4.6).
    fn carburant(&self) -> Option<u64>;

    /// Un instantané de tout ce qui fait l'état de l'instance (§ 4.5).
    ///
    /// Prend `&mut self` : lire une globale dans wasmtime demande un accès mutable
    /// au magasin, même sans rien changer.
    fn instantane(&mut self) -> Result<Self::Instantane, PanneOutil>;

    /// Remet l'instance dans l'état de l'instantané, taille de mémoire comprise.
    fn restaurer(&mut self, i: &Self::Instantane) -> Result<(), PanneOutil>;

    /// L'empreinte de l'état : BLAKE3 des zones d'état d'un module (§ 3.2), ou de la
    /// forme canonique de la copie native (§ 8.4). Prend `&mut self` pour la même
    /// raison que [`Instance::instantane`].
    fn empreinte(&mut self) -> Result<[u8; 32], PanneOutil>;
}

/// Ce qui fait des instances neuves d'un même module, ou d'un même jeu natif.
pub trait Fabrique {
    /// L'instance fabriquée.
    type I: Instance;

    /// Une instance neuve, avant `Bonjour` : rien n'y a été appelé.
    fn neuve(&mut self) -> Result<Self::I, PanneOutil>;

    /// Une instance neuve pour la tête (§ 4.2) : elle ne recevra que des copies de la
    /// définitive et le rejeu de la fenêtre, jamais `Creer`. Par défaut, une instance
    /// comme les autres ; une fabrique qui observe les appels (l'épreuve) s'en sert
    /// pour ne pas confondre la tête avec la partie qui fait foi.
    fn neuve_tete(&mut self) -> Result<Self::I, PanneOutil> {
        self.neuve()
    }
}

/// Pourquoi un appel n'a pas rendu de retour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Echec {
    /// Le jeu a fauté : c'est sa responsabilité, et l'entrée s'isole (§ 4.6).
    Faute(Faute),
    /// Le commun ou la machine a échoué : jamais une faute du jeu (HIS-06, OUT-11).
    Panne(PanneOutil),
}

impl fmt::Display for Echec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Echec::Faute(faute) => write!(f, "faute du jeu : {faute}"),
            Echec::Panne(panne) => write!(f, "{panne}"),
        }
    }
}

impl From<Faute> for Echec {
    fn from(f: Faute) -> Echec {
        Echec::Faute(f)
    }
}

impl From<PanneOutil> for Echec {
    fn from(p: PanneOutil) -> Echec {
        Echec::Panne(p)
    }
}

/// Une faute du jeu (§ 4.6). Les sept premières viennent d'une instance ; les deux
/// dernières sont constatées par le cœur, sur un retour bien formé, et aucune
/// instance ne les rend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Faute {
    /// Le carburant de l'appel est épuisé : le jeu n'a pas rendu la main.
    Budget,
    /// La profondeur d'appel dépasse son plafond (`soiree_profondeur`).
    Profondeur,
    /// Le jeu a paniqué, avec le message de sa zone de panique.
    Panique {
        /// Le message, tel que le gestionnaire de panique l'a écrit.
        message: String,
    },
    /// La mémoire demandée dépasse le maximum déclaré (`limites.memoire`).
    Memoire,
    /// Un accès hors de la mémoire du module.
    HorsMemoire,
    /// La sortie est hors du brouillon, ou ne se décode pas en [`jeu_abi::Retour`].
    SortieInvalide,
    /// Un autre piège du module, par son nom (division par zéro, `unreachable`…).
    Piege {
        /// Le nom du piège, tel que le moteur le donne.
        code: String,
    },
    /// Constatée par le cœur : l'étape a rendu `Attente` sur la partie qui fait foi,
    /// où aucun secret ne manque.
    Attente,
    /// Constatée par le cœur : une vue rendue ne passe pas sa validation (§ 6.1).
    VueInvalide {
        /// Ce qui ne va pas.
        detail: String,
    },
}

impl fmt::Display for Faute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Faute::Budget => f.write_str("carburant épuisé"),
            Faute::Profondeur => f.write_str("profondeur d'appel dépassée"),
            Faute::Panique { message } => write!(f, "panique : {message}"),
            Faute::Memoire => f.write_str("mémoire au-delà du maximum déclaré"),
            Faute::HorsMemoire => f.write_str("accès hors de la mémoire"),
            Faute::SortieInvalide => f.write_str("sortie hors du brouillon ou indécodable"),
            Faute::Piege { code } => write!(f, "piège : {code}"),
            Faute::Attente => f.write_str("un secret manque sur la partie qui fait foi"),
            Faute::VueInvalide { detail } => write!(f, "vue invalide : {detail}"),
        }
    }
}

/// Une panne de l'outil ou de la machine : jamais une faute du jeu (§ 4.6, HIS-06).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanneOutil {
    /// Ce qui a lâché.
    pub cause: CausePanne,
    /// Ce qu'on en sait, pour le rapport.
    pub detail: String,
}

impl PanneOutil {
    /// Une panne de cette cause.
    pub fn new(cause: CausePanne, detail: impl Into<String>) -> PanneOutil {
        PanneOutil {
            cause,
            detail: detail.into(),
        }
    }

    /// Qui a échoué : le commun ou la machine.
    pub const fn responsable(&self) -> Responsable {
        self.cause.responsable()
    }
}

impl fmt::Display for PanneOutil {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let qui = match self.responsable() {
            Responsable::Outil => "panne de l'outil",
            Responsable::Machine => "panne de la machine",
        };
        write!(f, "{qui} ({:?}) : {}", self.cause, self.detail)
    }
}

/// Ce qui a lâché, hors du jeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CausePanne {
    /// Un appel a dépassé la durée murale du garde-fou : bug du moteur, ou carburant
    /// mal compté (§ 4.6, HIS-06, OUT-11).
    GardeFouMural,
    /// Entre deux appels, `__stack_pointer` n'a pas retrouvé sa valeur initiale, ou
    /// `soiree_profondeur` ne vaut pas 0 (§ 3.2).
    PileNonRendue,
    /// `soiree_boite` a rendu 0 : la boîte ne peut pas recevoir l'appel (§ 3.4).
    BoiteRefusee,
    /// L'appel ne se lit pas : le module a rendu `Retour::Illisible`, ou sa sorte ne
    /// correspond pas à l'appel.
    AppelIllisible,
    /// Un rejeu depuis un point de reprise ne retrouve pas l'empreinte notée (§ 4.5).
    RejeuDivergent,
    /// Une erreur interne du moteur ou de l'instance, hors de ce qui précède.
    Interne,
    /// Un `memory.grow` sous le plafond a échoué : la machine manque de mémoire
    /// (`soiree_croissance_refusee`, § 3.6).
    CroissanceRefusee,
    /// L'instanciation a échoué alors que le module a été accepté.
    Instanciation,
}

impl CausePanne {
    /// Qui a échoué. La seule définition de la frontière entre l'outil et la machine.
    pub const fn responsable(self) -> Responsable {
        match self {
            CausePanne::GardeFouMural
            | CausePanne::PileNonRendue
            | CausePanne::BoiteRefusee
            | CausePanne::AppelIllisible
            | CausePanne::RejeuDivergent
            | CausePanne::Interne => Responsable::Outil,
            CausePanne::CroissanceRefusee | CausePanne::Instanciation => Responsable::Machine,
        }
    }
}

/// Qui a échoué, quand ce n'est pas le jeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Responsable {
    /// Le commun : moteur, cœur, colle, épreuve.
    Outil,
    /// La machine : mémoire, instanciation.
    Machine,
}
