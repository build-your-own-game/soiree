//! La partie qui fait foi (§ 4, § 8.6) : ouvrir, recevoir les arrivées, avancer,
//! reprendre.
//!
//! # La fenêtre et la tête (§ 4.2, § 4.4, D12)
//!
//! Sans fenêtre (`fenetre: SansObjet`, ou nulle), la partie tient une seule
//! instance : la tête est la définitive, sans copie ni rejeu (Punchline, Tier list).
//!
//! Avec une fenêtre de `w` pas, la définitive ne va que jusqu'à `cible − w` : un
//! geste en retard de moins de `w` pas y est encore placé à son instant. La tête est
//! une seconde instance, copiée de la définitive et rejouée jusqu'à la cible ; elle
//! ne sert qu'à la scène de l'écran et à ses effets `Provisoire` (`src/tete.rs`).
//! Le journal, les demandes, les sorties, les vues des téléphones, les sous-arbres
//! `definitif` et les effets `Definitive` ne viennent que de la définitive : un
//! résultat ne s'annonce qu'une fois définitif.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use jeu_abi::{
    ABI, Accueil, Appareil, Appel, ApplicableFil, DeclarationFil, Duree, ErreurFil, IdDemande,
    Illisible, Instant, Oui, Pas, PauseFil, RegardFil, Retour, SceneFil, SorteOctets, TexteCourt,
    decoder, encoder,
};

use crate::arrivee::{Arrivee, DemandeArrivee, EcranArrivee, Placement};
use crate::coeur::EtatCoeur;
use crate::instance::{CausePanne, Echec, Fabrique, Faute, Instance, PanneOutil};
use crate::journal::{ArriveePlacee, Journal, Ligne, QuoiPlace, ValeurTelephone};
use crate::regles::{self, Origine, ReglesTenu};

/// Ce qu'il faut pour ouvrir une partie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ouverture {
    /// La graine de la soirée : celle de la première partie, et la graine d'ordre
    /// qui départage les égalités (§ 4.3). Neuve à chaque soirée, sauf rejeu voulu.
    pub graine: [u8; 32],
    /// Le pas de la création.
    pub pas: u64,
    /// L'heure murale de la création, en millisecondes. L'heure d'un `Temps` s'en
    /// déduit par le pas : le cœur n'a pas d'horloge.
    pub heure: u64,
    /// Le code de salle de la soirée, que `Creer` donne au jeu
    /// (REPONSES-PROVISOIRES.md, question 15).
    pub salle: TexteCourt,
    /// Le carburant de chaque sorte d'appel.
    pub budgets: Budgets,
    /// Les pas entre deux points de reprise (point de départ : 600, 10 s de partie).
    /// 0 : aucun point à intervalle, seulement à la création et aux nouvelles parties.
    pub point_tous_les: u64,
}

/// Le carburant de chaque sorte d'appel (§ 3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budgets {
    /// Une entrée.
    pub entree: u64,
    /// Une lecture : vue, état, validation, ressources, garde.
    pub lecture: u64,
    /// La création d'un état.
    pub creation: u64,
    /// La forme canonique.
    pub canonique: u64,
}

/// La tête de la partie (§ 4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tete {
    /// La tête est la définitive : une seule instance, aucune copie, aucun rejeu.
    /// Toujours le cas sans fenêtre ; avec une fenêtre, tant que la tête n'est pas
    /// en avance (à l'ouverture, après un saut, après une faute de la tête).
    Definitive,
    /// Une seconde instance, copiée de la définitive et rejouée jusqu'à ce pas.
    Separee {
        /// Le dernier pas joué sur la tête.
        pas: Pas,
    },
}

/// Sur quelle instance un pas se joue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// La définitive : le journal, les sorties, les demandes, les points.
    Definitive,
    /// La tête : rien n'est écrit, rien n'est dit, rien ne part (§ 4.4).
    Tete,
}

/// Ce qui sait recevoir un avis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destinataire {
    /// Un appareil.
    Appareil(Appareil),
    /// L'écran de l'ordinateur de la soirée.
    Ecran,
}

/// Ce qu'un appel à [`Partie::avancer`] demande au commun de faire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Avancee {
    /// Le dernier pas définitif.
    pub definitif: Pas,
    /// Les avis des refus, chacun au seul appareil du geste.
    pub avis: Vec<(Appareil, Vec<u8>)>,
    /// Les avis des refus de l'écran, pour sa zone `avertissements`.
    pub avis_ecran: Vec<Vec<u8>>,
    /// Ceux dont une entrée a été écartée sur une faute du jeu : le commun leur dit
    /// « Ce geste n'a pas pu être pris en compte. » (§ 4.6).
    pub ecartes: Vec<Destinataire>,
    /// Les demandes à lancer, chacune une seule fois dans la soirée.
    pub lancer: Vec<(IdDemande, DemandeArrivee)>,
    /// Les demandes à annuler : leur réponse ne sera pas livrée.
    pub annuler: Vec<IdDemande>,
    /// Les appareils que le jeu oublie.
    pub oublier: Vec<Appareil>,
    /// Les appareils que le jeu retire.
    pub retirer: Vec<Appareil>,
    /// Les fautes du jeu constatées depuis l'appel précédent, lectures comprises.
    pub fautes: Vec<RapportFaute>,
    /// Le temps jeté par cet appel, au-delà du rattrapage (§ 4.1).
    pub saut: Duree,
    /// Trois `Temps` écartés de suite : le temps ne passe plus (§ 4.6).
    pub en_panne: Oui,
}

impl Avancee {
    const fn vide(definitif: Pas) -> Avancee {
        Avancee {
            definitif,
            avis: Vec::new(),
            avis_ecran: Vec::new(),
            ecartes: Vec::new(),
            lancer: Vec::new(),
            annuler: Vec::new(),
            oublier: Vec::new(),
            retirer: Vec::new(),
            fautes: Vec::new(),
            saut: Duree::ZERO,
            en_panne: Oui::Non,
        }
    }
}

/// Une vue, validée : celle d'un pas, jamais d'un instant d'arrivée (SQ-86).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vue {
    /// Le regard.
    pub regard: RegardFil,
    /// Le pas dont elle est la vue : le dernier pas définitif pour un téléphone ;
    /// pour l'écran, celui de la tête quand elle est en avance (§ 4.4).
    pub pas: Pas,
    /// Une [`jeu_abi::SceneFil`] encodée pour l'écran, une [`jeu_abi::MontreFil`]
    /// pour un téléphone.
    pub octets: Vec<u8>,
}

/// Ce que la définitive dit d'elle-même.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EtatVisible {
    /// L'indice de la phase dans `Phase::TOUTES`.
    pub phase_indice: u16,
    /// La phase est-elle terminale ?
    pub terminale: Oui,
    /// Qui la partie attend (CAT-78).
    pub attente: AttenteVisible,
    /// Si la partie accueille.
    pub accueil: Accueil,
    /// L'indice du cas dans `Cas::TOUTES`.
    pub cas_indice: u16,
}

/// Qui la partie attend, possédé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttenteVisible {
    /// La partie avancera seule à ce pas.
    pub echeance: Option<Pas>,
    /// Rien n'avance seul, et pourquoi.
    pub libre: Option<String>,
    /// Les appareils qui peuvent faire avancer la partie, avec des gestes encodés.
    pub joueurs: Vec<(Appareil, Vec<Vec<u8>>)>,
    /// Les actions de l'écran, encodées, dont l'une doit être acceptée.
    pub ecran: Vec<Vec<u8>>,
}

/// Où une faute du jeu a eu lieu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LieuFaute {
    /// Une écriture sur la définitive : l'entrée est écartée.
    Entree {
        /// La partie.
        partie: u32,
        /// L'ordre de l'entrée.
        ordre: u64,
        /// Son pas.
        place: Pas,
        /// L'entrée isolée, encodée.
        octets: Vec<u8>,
    },
    /// Une lecture : la dernière vue valide est gardée.
    Lecture {
        /// L'appel, par son nom.
        appel: &'static str,
    },
    /// La création d'une nouvelle partie : l'ancienne continue.
    Creation,
}

/// Le rapport d'une faute du jeu : de quoi la refaire seule sur la machine du
/// développeur (§ 4.6 ; HIS-06, HIS-07, OUT-141). L'empreinte du module, que le
/// cœur ne connaît pas, s'y ajoute chez celui qui a chargé le module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RapportFaute {
    /// La faute, avec le message de panique s'il y en a un.
    pub faute: Faute,
    /// Où.
    pub lieu: LieuFaute,
    /// Le carburant consommé par l'appel fautif, si l'instance en compte.
    pub carburant: Option<u64>,
    /// La graine de la soirée.
    pub graine: [u8; 32],
    /// L'indice, dans le journal, du point de reprise d'où refaire la faute.
    pub point: usize,
    /// Le nombre de lignes du journal jusqu'à la faute, entrée isolée comprise.
    pub jusqu_a: usize,
}

/// Un point de reprise : un instantané de la définitive et l'état du cœur, à un
/// endroit du journal (§ 4.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointDeReprise<S> {
    pub(crate) instantane: S,
    pub(crate) coeur: EtatCoeur,
    pub(crate) indice: usize,
    /// Le point sain que la partie avait à ce point, et son dernier point à
    /// intervalle : une reprise depuis ce point retrouve [`Partie::repartir`].
    pub(crate) sain: Option<Arc<PointDeReprise<S>>>,
    pub(crate) periodique: Option<Arc<PointDeReprise<S>>>,
}

impl<S> PointDeReprise<S> {
    /// Le pas définitif du point.
    pub const fn pas(&self) -> Pas {
        self.coeur.pas
    }

    /// Le nombre de lignes du journal que le point contient déjà.
    pub const fn indice(&self) -> usize {
        self.indice
    }

    /// L'instantané de l'instance.
    pub const fn instantane(&self) -> &S {
        &self.instantane
    }
}

/// Pourquoi une partie ne s'ouvre pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusOuverture {
    /// Le commun ou la machine a échoué.
    Panne(PanneOutil),
    /// Le jeu a fauté à `Bonjour`, à `Decrire` ou à la création.
    Faute(Faute),
    /// Le module ne parle pas l'enveloppe du cœur (§ 3.8).
    Abi {
        /// Celle du module.
        module: u32,
        /// Celle du cœur.
        coeur: u32,
    },
    /// Le module a rendu autre chose que le retour attendu.
    Retour {
        /// L'appel.
        appel: &'static str,
    },
}

impl From<PanneOutil> for RefusOuverture {
    fn from(p: PanneOutil) -> RefusOuverture {
        RefusOuverture::Panne(p)
    }
}

/// Pourquoi une partie ne reprend pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusReprise {
    /// L'instance neuve ne s'ouvre pas.
    Ouverture(RefusOuverture),
    /// Le rejeu a échoué : une entrée, une issue ou une empreinte diverge
    /// ([`CausePanne::RejeuDivergent`]), ou l'outil a lâché.
    Panne(PanneOutil),
    /// Le point est au-delà du journal.
    PointHorsJournal,
}

impl From<PanneOutil> for RefusReprise {
    fn from(p: PanneOutil) -> RefusReprise {
        RefusReprise::Panne(p)
    }
}

impl From<RefusOuverture> for RefusReprise {
    fn from(r: RefusOuverture) -> RefusReprise {
        RefusReprise::Ouverture(r)
    }
}

/// Ce que l'instance a dit d'octets d'appareil.
pub(crate) struct TenuValide {
    pub(crate) cle: Vec<u8>,
    pub(crate) relache: Option<Vec<u8>>,
    pub(crate) est_relache: Oui,
}

/// La partie qui fait foi : une fonction de son ouverture et de ses arrivées,
/// générique sur l'instance qui fait tourner le jeu (§ 4, D33).
pub struct Partie<F: Fabrique> {
    pub(crate) fabrique: F,
    pub(crate) definitive: F::I,
    /// La tête séparée, quand la fenêtre n'est pas nulle (§ 4.2).
    pub(crate) tete: Option<crate::tete::TeteSeparee<F::I>>,
    /// La fenêtre déclarée, en pas ; 0 sans fenêtre.
    pub(crate) fenetre: u64,
    /// Sur quelle instance le pas en cours se joue.
    pub(crate) mode: Mode,
    /// Sur la tête : l'entrée qui vient de fauter, avec son pas.
    pub(crate) tete_fautive: Option<(Pas, Vec<u8>)>,
    /// Sur la tête, le temps d'un rejeu : les entrées qu'elle ne rejoue plus.
    pub(crate) tete_ecartees: Vec<(Pas, Vec<u8>)>,
    /// Le pas que l'horloge a atteint, celui de la tête : la définitive le suit à
    /// `fenetre` pas. Sans fenêtre, celui de la définitive.
    pub(crate) horloge: Pas,
    pub(crate) ouverture: Ouverture,
    pub(crate) declaration: DeclarationFil,
    pub(crate) regles_telephone: Option<ReglesTenu>,
    pub(crate) regles_manette: Option<ReglesTenu>,
    pub(crate) coeur: EtatCoeur,
    pub(crate) journal: Journal,
    /// En rejeu : la prochaine ligne du journal de référence à retrouver.
    pub(crate) curseur: Option<usize>,
    pub(crate) point: PointDeReprise<<F::I as Instance>::Instantane>,
    /// Le dernier point à intervalle après lequel un `Temps` a été accepté : celui
    /// d'où [`Partie::repartir`] repart (§ 4.6).
    pub(crate) point_sain: Option<Arc<PointDeReprise<<F::I as Instance>::Instantane>>>,
    /// Le dernier point à intervalle, pas encore confirmé par un `Temps` accepté.
    pub(crate) periodique: Option<Arc<PointDeReprise<<F::I as Instance>::Instantane>>>,
    /// Les arrivées placées, pas encore jouées, dans l'ordre de leur arrivée.
    pub(crate) en_attente: Vec<ArriveePlacee>,
    /// La dernière scène valide de l'écran : ce que ses entrées doivent viser.
    pub(crate) derniere_scene: Option<SceneFil>,
    /// La dernière vue valide de chaque regard.
    pub(crate) dernieres_vues: Vec<Vue>,
    /// Les images qu'une scène a citées hors des assets, et si le jeu les connaît
    /// (`Ressources`) : chacune n'est demandée qu'une fois.
    pub(crate) images: Vec<(u64, Oui)>,
    /// Ce que le prochain [`Partie::avancer`] rendra.
    pub(crate) sortie: Avancee,
}

impl<F: Fabrique> Partie<F> {
    /// Ouvre une partie : une instance neuve, `Bonjour`, `Decrire`, `Creer`, la
    /// lecture de l'état, et le premier point de reprise.
    pub fn ouvrir(mut f: F, o: Ouverture) -> Result<Partie<F>, RefusOuverture> {
        let mut i = f.neuve()?;
        let declaration = accueillir(&mut i, &o.budgets)?;
        let presents = encoder_vers_vec(&Vec::<jeu_abi::Presence>::new())?;
        let reveil = match creer(&mut i, &o, o.graine, Pas::du_fil(o.pas), &presents, None) {
            Ok(r) => r,
            Err(Echec::Faute(f)) => return Err(RefusOuverture::Faute(f)),
            Err(Echec::Panne(p)) => return Err(RefusOuverture::Panne(p)),
        };
        let coeur = EtatCoeur::neuf(Pas::du_fil(o.pas), reveil);
        let instantane = i.instantane()?;
        let point = PointDeReprise {
            instantane,
            coeur: coeur.clone(),
            indice: 0,
            sain: None,
            periodique: None,
        };
        let mut p = Partie::assembler(f, i, o, declaration, coeur, point);
        p.lire_etat()?;
        p.prendre_point()?;
        // La création est saine : la reprise d'une panne avant le premier `Temps`
        // accepté repart d'elle (§ 4.6).
        p.point_sain = p.periodique.clone();
        Ok(p)
    }

    /// Reprend une partie depuis un point et le journal qui le contient (§ 4.5) :
    /// une instance neuve, le point restauré, puis le cœur rejoué sur le journal
    /// qui suit le point, en sautant les entrées écartées. Chaque entrée, chaque
    /// issue et chaque empreinte notée doit se retrouver ; sinon, c'est une panne
    /// de l'outil, qui nomme la première ligne qui diverge.
    ///
    /// Ce qui n'était pas encore définitif (les arrivées placées à un pas futur, un
    /// tenu retenu par le débit) n'est pas au journal et ne revient pas : les
    /// appareils renvoient leur état complet (REM-13). Chaque appareil est réputé
    /// avoir vu la dernière vue que le journal note pour lui ; ses vues lui sont
    /// renvoyées en entier (SAL-28).
    pub fn reprendre(
        mut fabrique: F,
        ouverture: Ouverture,
        point: &PointDeReprise<<F::I as Instance>::Instantane>,
        journal: &Journal,
    ) -> Result<Partie<F>, RefusReprise> {
        if point.indice > journal.lignes.len() {
            return Err(RefusReprise::PointHorsJournal);
        }
        let mut instance = fabrique.neuve().map_err(RefusOuverture::Panne)?;
        let declaration = accueillir(&mut instance, &ouverture.budgets)?;
        instance.restaurer(&point.instantane)?;
        let coeur = point.coeur.clone();
        let mut partie = Partie::assembler(
            fabrique,
            instance,
            ouverture,
            declaration,
            coeur,
            point.clone(),
        );
        partie.journal = journal.clone();
        partie.curseur = Some(point.indice);
        partie.rejouer()?;
        partie.curseur = None;
        // La tête de la partie arrêtée avait la fenêtre d'avance : la reprise la
        // rejoue sans compter ce temps comme un retard (§ 4.1).
        partie.horloge = Pas::du_fil(partie.coeur.pas.numero().saturating_add(partie.fenetre));
        partie.sortie = Avancee::vide(partie.coeur.pas);
        Ok(partie)
    }

    fn assembler(
        f: F,
        i: F::I,
        o: Ouverture,
        declaration: DeclarationFil,
        coeur: EtatCoeur,
        point: PointDeReprise<<F::I as Instance>::Instantane>,
    ) -> Partie<F> {
        let regles_telephone = match &declaration.telephones {
            ApplicableFil::Oui(t) => Some(ReglesTenu::depuis_fil(&t.tenu)),
            ApplicableFil::SansObjet(_) => None,
        };
        let regles_manette = match &declaration.manettes {
            ApplicableFil::Oui(r) => Some(ReglesTenu::depuis_fil(r)),
            ApplicableFil::SansObjet(_) => None,
        };
        let pas = coeur.pas;
        let fenetre = match declaration.fenetre {
            ApplicableFil::Oui(d) => d.en_pas(),
            ApplicableFil::SansObjet(_) => 0,
        };
        Partie {
            fabrique: f,
            definitive: i,
            tete: None,
            fenetre,
            mode: Mode::Definitive,
            tete_fautive: None,
            tete_ecartees: Vec::new(),
            horloge: pas,
            journal: Journal::vide(pas),
            ouverture: o,
            declaration,
            regles_telephone,
            regles_manette,
            coeur,
            curseur: None,
            point_sain: point.sain.clone(),
            periodique: point.periodique.clone(),
            point,
            en_attente: Vec::new(),
            derniere_scene: None,
            dernieres_vues: Vec::new(),
            images: Vec::new(),
            sortie: Avancee::vide(pas),
        }
    }

    /// La déclaration du jeu, lue par `Decrire` à l'ouverture.
    pub fn declaration(&self) -> &DeclarationFil {
        &self.declaration
    }

    /// Le journal de la soirée.
    pub fn journal(&self) -> &Journal {
        &self.journal
    }

    /// Le dernier pas définitif.
    pub const fn definitif(&self) -> Pas {
        self.coeur.pas
    }

    /// La tête (§ 4.2) : la définitive, ou la seconde instance et son pas.
    pub fn tete(&self) -> Tete {
        match &self.tete {
            Some(t) if t.montrable(self.coeur.pas) => Tete::Separee { pas: t.pas },
            Some(_) | None => Tete::Definitive,
        }
    }

    /// La fenêtre déclarée, en pas ; 0 sans fenêtre.
    pub const fn fenetre(&self) -> u64 {
        self.fenetre
    }

    /// L'état du cœur, pour comparer deux parties (une reprise et l'originale).
    pub const fn coeur(&self) -> &EtatCoeur {
        &self.coeur
    }

    /// L'instance définitive, en lecture : pour les tests et l'épreuve, qui
    /// comparent ce que le jeu garde. Rien ne s'y appelle sans passer par le cœur.
    pub const fn definitive(&self) -> &F::I {
        &self.definitive
    }

    /// La fabrique, en lecture.
    pub const fn fabrique(&self) -> &F {
        &self.fabrique
    }

    /// L'empreinte de la définitive.
    pub fn empreinte(&mut self) -> Result<[u8; 32], PanneOutil> {
        self.definitive.empreinte()
    }

    /// Le plus grand numéro de tenu reçu de cet appareil, toutes clés confondues, qu'il
    /// soit déjà livré ou qu'il attende son pas : ce que l'accueil d'une page rechargée
    /// lui redonne, pour qu'elle reprenne sa numérotation au-dessus (SAL-38, REM-14).
    /// Sans lui, une page qui repart de 1 serait écartée tenu après tenu (`Perimee`),
    /// sans que rien ne le dise. `None` : rien reçu de lui.
    pub fn numero_tenu(&self, de: Appareil) -> Option<u64> {
        let livres = self
            .coeur
            .tenus
            .iter()
            .filter(|t| t.appareil == de)
            .filter_map(|t| t.etat.numero);
        let en_attente = self.en_attente.iter().filter_map(|e| match &e.quoi {
            QuoiPlace::Tenu { de: d, numero, .. } if *d == de => Some(*numero),
            QuoiPlace::Tenu { .. }
            | QuoiPlace::Geste { .. }
            | QuoiPlace::Manette { .. }
            | QuoiPlace::Ecran(_)
            | QuoiPlace::Presence(_)
            | QuoiPlace::Reponse { .. } => None,
        });
        livres.chain(en_attente).max()
    }

    /// Les demandes lancées que le jeu liste encore et dont la réponse n'est pas
    /// arrivée : ce que le commun doit encore exécuter après une reprise.
    pub fn demandes_en_cours(&self) -> Vec<IdDemande> {
        self.coeur
            .demandes
            .iter()
            .filter(|d| d.repondue == Oui::Non)
            .filter_map(|d| self.coeur.exposer(d.id).map(IdDemande::du_fil))
            .collect()
    }

    /// Prend un point de reprise maintenant, et le garde comme dernier point. Il ne
    /// décale pas les points à intervalle, qui ne dépendent que de l'histoire : une
    /// reprise les retrouve aux mêmes pas.
    pub fn point(&mut self) -> Result<PointDeReprise<<F::I as Instance>::Instantane>, PanneOutil> {
        self.instantaner()?;
        Ok(self.point.clone())
    }

    /// Après trois `Temps` écartés de suite (§ 4.6), la partie repart du dernier
    /// point sain, sans les entrées qui le suivent : le dernier point à intervalle
    /// après lequel un `Temps` a été accepté. La définitive y est restaurée ; le
    /// temps perdu depuis est dit au jeu par le saut du prochain `Temps` ; chaque
    /// appareil dont la présence a changé depuis ce point est redit au jeu ; les
    /// tenus sont relivrés. Aucun appareil ne perd son lien, et le journal est gardé :
    /// il note la reprise ([`Ligne::Reprise`]), qu'une reprise du journal retrouve.
    ///
    /// Rend `Oui::Non`, sans rien changer, si la partie n'est pas en panne ou si
    /// aucun point sain n'existe encore. Une reprise du journal depuis un point pris
    /// entre le point sain et la reprise ne la retrouve pas : une panne de l'outil.
    pub fn repartir(&mut self) -> Result<Oui, PanneOutil> {
        if self.coeur.en_panne == Oui::Non || self.curseur.is_some() {
            return Ok(Oui::Non);
        }
        let Some(sain) = self.point_sain.clone() else {
            return Ok(Oui::Non);
        };
        self.noter(Ligne::Reprise {
            apres: self.coeur.pas,
            depuis: sain.indice,
        })?;
        let presences = self.appliquer_reprise(&sain)?;
        let recu = Instant::de(self.coeur.pas);
        for p in presences {
            let _: Placement = self.arriver(Arrivee::Presence(p), recu)?;
        }
        Ok(Oui::Oui)
    }

    /// Commence ou finit une pause du commun (§ 4.1) : plus de `Temps`, et chaque
    /// tenu non relâché reçoit son relâchement, cause `Pause`, au pas suivant.
    ///
    /// Sans effet si le jeu n'a pas déclaré `Pause::DuCommun`, ou si rien ne change ;
    /// rend alors `Oui::Non`.
    pub fn pause(&mut self, en_pause: Oui) -> Result<Oui, PanneOutil> {
        if self.declaration.pause != PauseFil::DuCommun || self.coeur.en_pause == en_pause {
            return Ok(Oui::Non);
        }
        self.noter(Ligne::Pause {
            apres: self.coeur.pas,
            en_pause,
        })?;
        self.appliquer_pause(en_pause);
        Ok(Oui::Oui)
    }

    pub(crate) fn appliquer_pause(&mut self, en_pause: Oui) {
        self.coeur.en_pause = en_pause;
        self.coeur.relacher_pour_pause = en_pause;
    }

    /// Avance la partie jusqu'au pas `cible`, le pas que l'horloge de la soirée a
    /// atteint (§ 4.4). Au-delà du rattrapage déclaré, le numéro de pas saute, et le
    /// premier `Temps` qui suit porte la durée jetée (§ 4.1, D14).
    ///
    /// Sans fenêtre, la définitive va jusqu'à la cible. Avec une fenêtre de `w` pas,
    /// la définitive va jusqu'à `cible − w`, et la tête jusqu'à la cible, rejouée
    /// depuis un instantané de la définitive si un geste est arrivé à une place
    /// qu'elle avait déjà jouée. Un saut vide d'abord la fenêtre : la définitive
    /// rattrape la tête, puis les deux sautent ensemble.
    pub fn avancer(&mut self, cible: Pas) -> Result<Avancee, PanneOutil> {
        let tete = self.horloge.max(self.coeur.pas);
        let (jusqua, saut) =
            regles::cible(self.coeur.pas, tete, cible, self.declaration.rattrapage);
        let borne = if saut > Duree::ZERO {
            jusqua
        } else {
            Pas::du_fil(jusqua.numero().saturating_sub(self.fenetre))
        };
        while self.coeur.pas < borne {
            let s = Pas::du_fil(self.coeur.pas.numero().saturating_add(1));
            self.jouer_pas(s)?;
        }
        if saut > Duree::ZERO {
            let de = self.coeur.pas;
            let a = Pas::du_fil(de.numero().saturating_add(saut.en_pas()));
            self.noter(Ligne::Saut { de, a })?;
            self.appliquer_saut(a);
        }
        self.horloge = jusqua.max(self.coeur.pas);
        if self.fenetre > 0 {
            self.avancer_la_tete(self.horloge)?;
        }
        let mut a = core::mem::replace(&mut self.sortie, Avancee::vide(self.coeur.pas));
        a.definitif = self.coeur.pas;
        a.saut = saut;
        a.en_panne = self.coeur.en_panne;
        Ok(a)
    }

    /// Le numéro de pas saute jusqu'à `a`. Les arrivées placées dans les pas sautés
    /// s'appliquent au premier pas après le saut.
    pub(crate) fn appliquer_saut(&mut self, a: Pas) {
        let saut = Duree::pas(a.numero().saturating_sub(self.coeur.pas.numero()));
        self.coeur.saut = self.coeur.saut.plus(saut);
        self.coeur.pas = a;
        self.journal.pas = self.journal.pas.max(a);
        let apres = Pas::du_fil(a.numero().saturating_add(1));
        for e in &mut self.en_attente {
            e.place = e.place.max(apres);
        }
    }

    /// Une arrivée du transport, déjà dédoublonnée par appareil ; `a_recu` est
    /// l'instant de sa réception sur l'horloge de la partie (§ 4.3).
    ///
    /// Rien de malformé, rien que la scène ne portait, rien de périmé n'entre au
    /// journal. Le reste est placé, et s'appliquera quand la définitive atteindra
    /// son pas.
    pub fn arriver(&mut self, a: Arrivee, a_recu: Instant) -> Result<Placement, PanneOutil> {
        match a {
            Arrivee::Geste {
                de,
                octets,
                fait,
                vu,
                id,
                demi_aller_retour,
            } => self.arriver_geste(de, octets, (fait, vu, demi_aller_retour), id, a_recu),
            Arrivee::Tenu {
                de,
                octets,
                numero,
                fait,
                demi_aller_retour,
            } => self.arriver_tenu(de, octets, numero, (fait, demi_aller_retour), a_recu),
            Arrivee::Manette {
                de,
                etat,
                numero,
                fait,
            } => Ok(self.arriver_manette(de, etat, numero, fait, a_recu)),
            Arrivee::Ecran { quoi, vu } => self.arriver_ecran(quoi, vu, a_recu),
            Arrivee::Presence(p) => {
                let de = appareil_de(&p);
                let origine = Origine::Appareil(self.coeur.rang(de));
                let (fait, place) = self.placer_local(de, a_recu);
                Ok(self.placer_arrivee(
                    place,
                    fait,
                    a_recu,
                    self.coeur.pas,
                    origine,
                    QuoiPlace::Presence(p),
                ))
            }
            Arrivee::Reponse { id, reponse } => {
                // L'identité exposée : une réponse d'une partie passée est périmée.
                let Some(id) = self.coeur.interne(id.fil()) else {
                    return Ok(Placement::Perimee);
                };
                if !self.coeur.attend_reponse(id) {
                    return Ok(Placement::Perimee);
                }
                // Une réponse vient du commun, sur cette machine : à sa réception.
                let (fait, place) =
                    regles::placer(a_recu, a_recu, Duree::ZERO, Duree::ZERO, self.coeur.pas);
                Ok(self.placer_arrivee(
                    place,
                    fait,
                    a_recu,
                    self.coeur.pas,
                    Origine::Commun,
                    QuoiPlace::Reponse { id, reponse },
                ))
            }
        }
    }

    /// Le prochain pas à jouer sur la définitive.
    pub(crate) fn suivant(&self) -> Pas {
        Pas::du_fil(self.coeur.pas.numero().saturating_add(1))
    }

    fn arriver_geste(
        &mut self,
        de: Appareil,
        octets: Vec<u8>,
        (fait, vu, demi): (Instant, Pas, Duree),
        id: jeu_abi::IdGeste,
        a_recu: Instant,
    ) -> Result<Placement, PanneOutil> {
        if let Some(refus) = self.trop_long(&octets) {
            return Ok(refus);
        }
        if let Err(refus) = self.valider(SorteOctets::GesteTelephone, &octets)? {
            return Ok(refus);
        }
        let rang = self.coeur.rang(de);
        let origine = Origine::Appareil(rang);
        let limite = usize::from(self.declaration.limites.file);
        if self
            .en_attente
            .iter()
            .filter(|e| e.origine == origine)
            .count()
            >= limite
        {
            return Ok(Placement::FilePleine);
        }
        let (fait, place) = self.placer_appareil(de, fait, a_recu, demi);
        let vu = self.borner_vu(de, vu);
        Ok(self.placer_arrivee(
            place,
            fait,
            a_recu,
            vu,
            origine,
            QuoiPlace::Geste { de, octets, id },
        ))
    }

    fn arriver_tenu(
        &mut self,
        de: Appareil,
        octets: Vec<u8>,
        numero: u64,
        (fait, demi): (Instant, Duree),
        a_recu: Instant,
    ) -> Result<Placement, PanneOutil> {
        if self.regles_telephone.is_none() {
            return Ok(Placement::Malforme(ErreurFil {
                ou: 0,
                quoi: Illisible::Ordre,
            }));
        }
        if let Some(refus) = self.trop_long(&octets) {
            return Ok(refus);
        }
        let tenu = match self.valider(SorteOctets::TenuTelephone, &octets)? {
            Ok(Some(t)) => t,
            Ok(None) => {
                return Ok(Placement::Malforme(ErreurFil {
                    ou: 0,
                    quoi: Illisible::Forme,
                }));
            }
            Err(refus) => return Ok(refus),
        };
        if self
            .coeur
            .numero_tenu(de, &tenu.cle)
            .is_some_and(|n| numero <= n)
        {
            return Ok(Placement::Perimee);
        }
        let origine = Origine::Appareil(self.coeur.rang(de));
        let (fait, place) = self.placer_appareil(de, fait, a_recu, demi);
        let valeur = ValeurTelephone {
            octets,
            cle: tenu.cle,
            relache: tenu.relache,
            est_relache: tenu.est_relache,
        };
        let vu = self.borner_vu(de, Pas::du_fil(u64::MAX));
        Ok(self.placer_arrivee(
            place,
            fait,
            a_recu,
            vu,
            origine,
            QuoiPlace::Tenu { de, numero, valeur },
        ))
    }

    fn arriver_manette(
        &mut self,
        de: Appareil,
        etat: jeu_abi::Manette,
        numero: u64,
        fait: Instant,
        a_recu: Instant,
    ) -> Placement {
        if self.regles_manette.is_none() {
            return Placement::Malforme(ErreurFil {
                ou: 0,
                quoi: Illisible::Ordre,
            });
        }
        if self.coeur.numero_manette(de).is_some_and(|n| numero <= n) {
            return Placement::Perimee;
        }
        let origine = Origine::Appareil(self.coeur.rang(de));
        // Une manette est branchée à l'ordinateur de la soirée : aucune latence.
        let (fait, place) = self.placer_appareil(de, fait, a_recu, Duree::ZERO);
        self.placer_arrivee(
            place,
            fait,
            a_recu,
            self.coeur.pas,
            origine,
            QuoiPlace::Manette { de, numero, etat },
        )
    }

    fn arriver_ecran(
        &mut self,
        quoi: EcranArrivee,
        vu: Pas,
        a_recu: Instant,
    ) -> Result<Placement, PanneOutil> {
        let admise = self
            .derniere_scene
            .as_ref()
            .map_or(Oui::Non, |s| regles::admise_par_la_scene(s, &quoi));
        if admise == Oui::Non {
            return Ok(Placement::HorsScene);
        }
        if let EcranArrivee::Action(a) = &quoi {
            if let Some(refus) = self.trop_long(a) {
                return Ok(refus);
            }
            if let Err(refus) = self.valider(SorteOctets::ActionEcran, a)? {
                return Ok(refus);
            }
        }
        // L'écran est local : il n'antidate pas.
        let (fait, place) =
            regles::placer(a_recu, a_recu, Duree::ZERO, Duree::ZERO, self.coeur.pas);
        let vu = regles::borner_vu(vu, self.coeur.derniere_vue(RegardFil::Ecran));
        Ok(self.placer_arrivee(
            place,
            fait,
            a_recu,
            vu,
            Origine::Ecran,
            QuoiPlace::Ecran(quoi),
        ))
    }

    fn trop_long(&self, octets: &[u8]) -> Option<Placement> {
        let max = usize::try_from(self.declaration.limites.geste).unwrap_or(usize::MAX);
        (octets.len() > max).then_some(Placement::Malforme(ErreurFil {
            ou: u32::try_from(max).unwrap_or(u32::MAX),
            quoi: Illisible::Limite,
        }))
    }

    /// Place une arrivée d'appareil : `fait` borné par la latence et l'antidatage
    /// permis, puis l'ordre de l'appareil gardé.
    fn placer_appareil(
        &mut self,
        de: Appareil,
        fait: Instant,
        a_recu: Instant,
        demi: Duree,
    ) -> (Instant, Pas) {
        let (fait, place) = regles::placer(
            fait,
            a_recu,
            demi,
            self.declaration.antidatage_max,
            self.coeur.pas,
        );
        self.garder_l_ordre(de, fait, place)
    }

    /// Place une arrivée locale (une présence) : à sa réception.
    fn placer_local(&mut self, de: Appareil, a_recu: Instant) -> (Instant, Pas) {
        let (fait, place) =
            regles::placer(a_recu, a_recu, Duree::ZERO, Duree::ZERO, self.coeur.pas);
        self.garder_l_ordre(de, fait, place)
    }

    fn garder_l_ordre(&mut self, de: Appareil, fait: Instant, place: Pas) -> (Instant, Pas) {
        let precedente = self.coeur.fiche(de).and_then(|f| f.derniere);
        let (fait, place) = regles::suivre(fait, place, precedente);
        if let Some(f) = self.coeur.fiche_mut(de) {
            f.derniere = Some((fait, place));
        }
        (fait, place)
    }

    fn borner_vu(&self, de: Appareil, vu: Pas) -> Pas {
        let derniere = self.coeur.derniere_vue(RegardFil::Appareil(de.fil()));
        regles::borner_vu(vu, derniere)
    }

    fn placer_arrivee(
        &mut self,
        place: Pas,
        fait: Instant,
        recu: Instant,
        vu: Pas,
        origine: Origine,
        quoi: QuoiPlace,
    ) -> Placement {
        let sequence = self.coeur.sequence();
        // Une place que la tête a déjà jouée : elle se rejouera depuis la définitive.
        if let Some(t) = self.tete.as_mut()
            && place <= t.pas
        {
            t.a_rejouer = true;
        }
        self.en_attente.push(ArriveePlacee {
            sequence,
            place,
            fait,
            recu,
            vu,
            origine,
            quoi,
        });
        Placement::Place { pas: place }
    }

    /// Valide des octets d'appareil par la définitive (`Appel::Valider`, lecture).
    /// `Ok(Err(refus))` dit pourquoi ils n'entrent pas.
    fn valider(
        &mut self,
        sorte: SorteOctets,
        octets: &[u8],
    ) -> Result<Result<Option<TenuValide>, Placement>, PanneOutil> {
        let appel = Appel::Valider { sorte, octets };
        match self.lire(&appel, "Valider") {
            Ok(r) => match decoder::<Retour<'_>>(&r) {
                Ok(Retour::Valide(jeu_abi::Validation::Valide { tenu })) => {
                    Ok(Ok(tenu.map(|t| TenuValide {
                        cle: t.cle.to_vec(),
                        relache: t.relache.map(<[u8]>::to_vec),
                        est_relache: t.est_relache,
                    })))
                }
                Ok(Retour::Valide(jeu_abi::Validation::Malforme(e))) => {
                    Ok(Err(Placement::Malforme(e)))
                }
                Ok(Retour::Illisible(e)) => Err(illisible(e)),
                Ok(_) | Err(_) => {
                    self.rapporter_lecture(Faute::SortieInvalide, "Valider");
                    Ok(Err(Placement::Ecartee))
                }
            },
            Err(Lu::Faute(_)) => Ok(Err(Placement::Ecartee)),
            Err(Lu::Panne(p)) => Err(p),
        }
    }
}

/// `Bonjour` puis `Decrire` sur une instance neuve.
pub(crate) fn accueillir<I: Instance>(
    i: &mut I,
    b: &Budgets,
) -> Result<DeclarationFil, RefusOuverture> {
    let r = appeler(i, &Appel::Bonjour, b.lecture).map_err(refus)?;
    let abi = match decoder::<Retour<'_>>(&r) {
        Ok(Retour::Bonjour(b)) => b.abi,
        Ok(Retour::Illisible(e)) => return Err(RefusOuverture::Panne(illisible(e))),
        Ok(_) | Err(_) => return Err(RefusOuverture::Retour { appel: "Bonjour" }),
    };
    if abi != ABI {
        return Err(RefusOuverture::Abi {
            module: abi,
            coeur: ABI,
        });
    }
    let r = appeler(i, &Appel::Decrire, b.lecture).map_err(refus)?;
    let declaration = match decoder::<Retour<'_>>(&r) {
        Ok(Retour::Decrit(d)) => *d,
        Ok(Retour::Illisible(e)) => return Err(RefusOuverture::Panne(illisible(e))),
        Ok(_) | Err(_) => return Err(RefusOuverture::Retour { appel: "Decrire" }),
    };
    Ok(declaration)
}

fn refus(e: Echec) -> RefusOuverture {
    match e {
        Echec::Faute(f) => RefusOuverture::Faute(f),
        Echec::Panne(p) => RefusOuverture::Panne(p),
    }
}

/// `Creer` : rend le réveil de l'état créé.
pub(crate) fn creer<I: Instance>(
    i: &mut I,
    o: &Ouverture,
    graine: [u8; 32],
    pas: Pas,
    presents: &[u8],
    garde: Option<&[u8]>,
) -> Result<jeu_abi::ReveilFil, Echec> {
    let appel = Appel::Creer {
        graine,
        pas: pas.numero(),
        heure: heure_du_pas(o, pas),
        salle: o.salle,
        presents,
        garde,
    };
    let r = appeler(i, &appel, o.budgets.creation)?;
    match decoder::<Retour<'_>>(&r) {
        Ok(Retour::Issue(jeu_abi::IssueFil::Acceptee { reveil, .. })) => Ok(reveil),
        Ok(Retour::Illisible(e)) => Err(Echec::Panne(illisible(e))),
        Ok(Retour::Issue(jeu_abi::IssueFil::Attente)) => Err(Echec::Faute(Faute::Attente)),
        Ok(_) | Err(_) => Err(Echec::Faute(Faute::SortieInvalide)),
    }
}

/// L'heure murale d'un pas, déduite de l'ouverture : le cœur n'a pas d'horloge.
pub(crate) fn heure_du_pas(o: &Ouverture, pas: Pas) -> u64 {
    let ecoule = Duree::pas(pas.numero().saturating_sub(o.pas));
    o.heure.saturating_add(ecoule.en_ms())
}

/// Encode un appel et l'exécute, sous ce budget.
pub(crate) fn appeler<I: Instance>(
    i: &mut I,
    a: &Appel<'_>,
    budget: u64,
) -> Result<Vec<u8>, Echec> {
    let octets = encoder_vers_vec(a)?;
    i.appeler(a.sorte(), &octets, budget)
}

pub(crate) fn encoder_vers_vec<T: serde::Serialize + ?Sized>(v: &T) -> Result<Vec<u8>, PanneOutil> {
    let mut octets = Vec::new();
    encoder(v, &mut octets)
        .map_err(|e| PanneOutil::new(CausePanne::Interne, alloc::format!("{e}")))?;
    Ok(octets)
}

/// La panne d'un appel que la frontière du module a refusé.
pub(crate) fn illisible(e: ErreurFil) -> PanneOutil {
    PanneOutil::new(CausePanne::AppelIllisible, alloc::format!("{e}"))
}

/// L'appareil d'une présence.
pub(crate) const fn appareil_de(p: &jeu_abi::Presence) -> Appareil {
    match *p {
        jeu_abi::Presence::Arrivee { appareil, .. }
        | jeu_abi::Presence::Depart { appareil, .. }
        | jeu_abi::Presence::Retour { appareil, .. }
        | jeu_abi::Presence::Capacites { appareil, .. } => appareil,
    }
}

/// L'échec d'une lecture, après que le cœur a traité une faute.
pub(crate) enum Lu {
    /// Le jeu a fauté ; la définitive est reconstruite et la faute rapportée.
    Faute(Faute),
    /// Le commun a lâché.
    Panne(PanneOutil),
}
