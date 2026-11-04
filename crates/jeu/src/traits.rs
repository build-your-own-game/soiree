//! Les traits d'un jeu (§ 2.11 et § 2.13) : [`Jeu`], ce qui le crée, ce qu'il attend et
//! ce qu'il demande ; [`Epreuve`], ce qu'il donne à l'épreuve.

use alloc::string::String;
use alloc::vec::Vec;

use jeu_abi::{
    Accueil, Appareil, Cle, DemandeFil, Deplace, Duree, FinSaisie, Flux, FormatImage, HeureMurale,
    IdDemande, IdFichier, Oui, Pas, Presence, Prevision, TexteCourt, Touche,
};

use crate::declaration::{Applicable, Declaration, Raison};
use crate::etape::{Avant, Entree, Issue, Tenu};
use crate::forme::{Aucun, Etat, Message};
use crate::hasard::Hasard;
use crate::scene::{Montre, Scene};
use crate::secret::{Absent, Autorite, Regard};
use crate::temps::{Echeance, Maintenant, Reveil};

/// Les variantes d'une enum sans champ, dans l'ordre. `#[derive(Variantes)]` l'écrit.
pub trait Variantes: Sized + 'static {
    /// Toutes les variantes.
    const TOUTES: &'static [Self];
}

/// Où en est la partie. L'épreuve vérifie qu'elle les parcourt toutes (E3).
pub trait Phase: Message + Variantes + Copy + Eq {
    /// Cette phase termine-t-elle la partie ?
    fn terminale(self) -> Oui;
}

/// Un jeu.
pub trait Jeu: Etat + Sized + 'static {
    /// Les commandes de l'écran de l'ordinateur de la soirée. Un téléphone ne peut pas en
    /// envoyer : le décodeur de son canal ne connaît que `GesteTelephone` (SQ-21, SQ-22,
    /// PUN-48).
    type ActionEcran: Message;
    /// Un événement d'un téléphone.
    type GesteTelephone: Message;
    /// Un état tenu d'un téléphone.
    type TenuTelephone: Tenu;
    /// Ce qu'un téléphone affiche.
    type VueTelephone: Message;
    /// Ce que dit un refus, au seul appareil du geste.
    type Avis: Message;
    /// Où en est la partie.
    type Phase: Phase;
    /// Ce qu'une nouvelle partie garde exprès de la précédente.
    type Garde: Etat;
    /// L'état de la version précédente du jeu, ou `Aucun`.
    type Precedent: Etat;

    /// La déclaration.
    const DECLARATION: Declaration;

    /// Crée l'état.
    fn creer(c: Creation<'_, Self>) -> Self;
    /// Une étape : lire, puis accepter ou refuser.
    fn appliquer(avant: Avant<'_, Self>, entree: Entree<'_, Self>) -> Issue<Self>;

    /// La scène de l'écran.
    fn vue_ecran(&self, r: &Regard, s: &mut Scene<Self::ActionEcran>);
    /// Ce que voit un téléphone.
    fn vue_telephone(&self, r: &Regard) -> Montre<Self::VueTelephone>;
    /// Les octets d'une image citée par une scène, que le moteur n'a pas encore.
    fn ressource(&self, cle: Cle) -> Option<&[u8]>;

    /// La phase.
    fn phase(&self) -> Self::Phase;
    /// Quand le jeu veut recevoir le temps.
    fn reveil(&self) -> Reveil;
    /// Qui la partie attend (CAT-78).
    fn attente(&self, a: &mut Attente<'_, Self>);
    /// Si la partie accueille un nouvel appareil.
    fn accueil(&self) -> Accueil;
    /// Les demandes en cours. Une demande qui n'est plus listée est annulée.
    fn demandes<'s>(&'s self, d: &mut Demandes<'s>);

    /// Ce geste peut-il s'afficher avant confirmation ? Lu seulement si `prediction: Oui`.
    fn previsible(g: &Self::GesteTelephone) -> Prevision;
    /// Ce que garde une nouvelle partie.
    fn garder(&self) -> Self::Garde;
    /// L'état depuis celui de la version précédente.
    fn convertir(ancien: Self::Precedent) -> Result<Self, Raison>;
}

/// Ce que reçoit `creer`.
pub struct Creation<'c, J: Jeu> {
    pub(crate) maintenant: Maintenant<'c>,
    pub(crate) heure: HeureMurale,
    pub(crate) autorite: &'c Autorite,
    pub(crate) hasard: Option<&'c mut Hasard>,
    pub(crate) presents: &'c [Presence],
    pub(crate) garde: Option<J::Garde>,
    pub(crate) salle: TexteCourt,
}

impl<J: Jeu> Creation<'_, J> {
    /// L'instant de la création.
    pub fn maintenant(&self) -> Maintenant<'_> {
        self.maintenant.preter()
    }

    /// L'heure murale, pour étiqueter.
    pub fn heure(&self) -> HeureMurale {
        self.heure
    }

    /// Le jeton qui ouvre les secrets.
    pub fn autorite(&self) -> &Autorite {
        self.autorite
    }

    /// Le générateur de la partie, si le jeu l'a déclaré.
    pub fn hasard(&mut self) -> Result<&mut Hasard, Absent> {
        self.hasard.as_deref_mut().ok_or(Absent)
    }

    /// Les appareils déjà là : vide au premier lancement, la salle entière après « On
    /// rejoue ! ».
    pub fn presents(&self) -> &[Presence] {
        self.presents
    }

    /// Ce que la partie précédente a gardé, une fois.
    pub fn garde(&mut self) -> Option<J::Garde> {
        self.garde.take()
    }

    /// Le code de la salle, stable d'une partie à l'autre (REPONSES-PROVISOIRES.md,
    /// question 15).
    pub fn salle(&self) -> TexteCourt {
        self.salle
    }
}

/// Qui la partie attend, pour que le commun vérifie qu'elle ne peut pas se figer (CAT-78).
///
/// Ce que le jeu y écrit ne va qu'au commun, jamais à un appareil : il peut donc lire
/// ses secrets avec [`Attente::autorite`], par exemple pour nommer les votants d'un duel
/// dont les auteurs sont cachés (besoin 7 des jeux, `conception/BESOINS-DES-JEUX.md`).
pub struct Attente<'a, J: Jeu> {
    pub(crate) echeance: Option<Pas>,
    pub(crate) libre: Option<&'static str>,
    pub(crate) joueurs: Vec<(Appareil, Vec<Vec<u8>>)>,
    pub(crate) ecran: Vec<Vec<u8>>,
    pub(crate) autorite: &'a Autorite,
    pub(crate) _jeu: core::marker::PhantomData<&'a J>,
}

fn encoder_tous<T: Message>(v: &[T]) -> Vec<Vec<u8>> {
    v.iter()
        .filter_map(|x| {
            let mut o = Vec::new();
            jeu_abi::encoder(x, &mut o).ok().map(|()| o)
        })
        .collect()
}

impl<J: Jeu> Attente<'_, J> {
    /// Le jeton qui ouvre les secrets. L'attente se lit sur la partie qui fait foi, et
    /// ne sort pas du commun : comme `resume` et `invariants`, elle voit tout.
    pub fn autorite(&self) -> &Autorite {
        self.autorite
    }

    /// Cet appareil peut faire avancer la partie ; l'un de ces gestes doit être accepté.
    pub fn joueur(&mut self, a: Appareil, exemples: &[J::GesteTelephone]) {
        self.joueurs.push((a, encoder_tous(exemples)));
    }

    /// L'écran peut faire avancer la partie ; l'une de ces actions doit être acceptée.
    pub fn ecran(&mut self, exemples: &[J::ActionEcran]) {
        self.ecran.extend(encoder_tous(exemples));
    }

    /// La partie avancera seule à cette échéance. La plus proche compte.
    pub fn echeance(&mut self, e: Echeance) {
        self.echeance = Some(self.echeance.map_or(e.pas(), |p| p.min(e.pas())));
    }

    /// Rien n'avance seul, par nature (un éditeur).
    pub fn libre(&mut self, r: Raison) {
        if self.libre.is_none() {
            self.libre = Some(r.texte());
        }
    }
}

/// Une demande au commun.
#[derive(Debug)]
pub enum Demande<'d> {
    /// Décoder, recadrer au centre, réduire au carré, encoder (TL-60).
    Image {
        /// Le fichier reçu.
        fichier: IdFichier,
        /// Le côté du carré.
        carre: u16,
        /// Le format.
        format: FormatImage,
    },
    /// Dessiner une scène hors écran (TL-57).
    Rendu {
        /// La scène.
        scene: Scene<Aucun>,
        /// Le facteur d'agrandissement.
        echelle: u8,
        /// Le format.
        format: FormatImage,
    },
    /// Lire un fichier reçu, taille vérifiée avant lecture (TL-14).
    Lire {
        /// Le fichier.
        fichier: IdFichier,
        /// La taille la plus grande.
        max: u64,
    },
    /// Donner un fichier à télécharger.
    Telecharger {
        /// Son nom.
        nom: &'d str,
        /// Ses octets.
        octets: &'d [u8],
    },
    /// Ajouter une ligne à un flux de résultats de l'ordinateur de la soirée.
    Ligne {
        /// Le flux.
        flux: Flux,
        /// La ligne.
        octets: &'d [u8],
    },
}

impl<'d> Demande<'d> {
    fn fil(self) -> DemandeFil<'d> {
        match self {
            Demande::Image {
                fichier,
                carre,
                format,
            } => DemandeFil::Image {
                fichier,
                carre,
                format,
            },
            Demande::Rendu {
                scene,
                echelle,
                format,
            } => DemandeFil::Rendu {
                scene: scene.fil(),
                echelle,
                format,
            },
            Demande::Lire { fichier, max } => DemandeFil::Lire { fichier, max },
            Demande::Telecharger { nom, octets } => DemandeFil::Telecharger { nom, octets },
            Demande::Ligne { flux, octets } => DemandeFil::Ligne { flux, octets },
        }
    }
}

/// Les demandes que le jeu liste (D9).
pub struct Demandes<'d> {
    pub(crate) connues: &'d [u64],
    pub(crate) toutes: Vec<u64>,
    pub(crate) nouvelles: Vec<(u64, DemandeFil<'d>)>,
}

impl<'d> Demandes<'d> {
    /// `f` n'est appelée que si le moteur ne connaît pas encore `id`.
    pub fn ajouter(&mut self, id: IdDemande, f: impl FnOnce() -> Demande<'d>) {
        let n = id.fil();
        if self.toutes.contains(&n) {
            return;
        }
        self.toutes.push(n);
        if !self.connues.contains(&n) {
            self.nouvelles.push((n, f().fil()));
        }
    }
}

/// Le résultat d'un invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constat {
    /// Tenu.
    Tenu,
    /// Violé.
    Viole,
}

/// Ce que constatent les invariants et les transitions.
#[derive(Debug, Default)]
pub struct Verif {
    pub(crate) constats: Vec<(&'static str, Constat)>,
}

impl Verif {
    /// Constate un invariant, par son nom.
    pub fn constater(&mut self, nom: &'static str, c: Constat) {
        self.constats.push((nom, c));
    }
}

/// `jeu::exiger!(v, condition, "nom");` : constate `condition` sous ce nom.
#[macro_export]
macro_rules! exiger {
    ($v:expr, $condition:expr, $nom:literal) => {
        $crate::Verif::constater(
            $v,
            $nom,
            if $condition {
                $crate::Constat::Tenu
            } else {
                $crate::Constat::Viole
            },
        )
    };
}

/// Comment joue un bot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Comme un joueur attentif.
    Sage,
    /// Comme un joueur qui cherche à casser.
    Hostile,
    /// Comme un joueur qui tarde.
    Lent,
}

/// Un fichier qu'apporte un bot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FichierBot {
    /// Son nom.
    pub nom: String,
    /// Son type.
    pub genre: String,
    /// Ses octets.
    pub octets: Vec<u8>,
}

/// Ce qu'un bot fait d'un composant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposantBot {
    /// Il saisit un texte.
    Saisie {
        /// Le champ.
        champ: Cle,
        /// Le texte.
        texte: String,
        /// Ce qui termine la saisie.
        fin: FinSaisie,
    },
    /// Il déplace un objet.
    Deplace(Deplace),
    /// Il apporte des fichiers.
    Fichiers(Vec<FichierBot>),
}

/// Un coup d'un bot, avant son encodage.
#[derive(Debug)]
pub(crate) enum Coup {
    Geste(Vec<u8>),
    Tenu(Vec<u8>),
    Action(Vec<u8>),
    Touche(Touche),
    Composant(ComposantBot),
    Rien,
}

/// Le corpus de textes hostiles du commun : emoji composés, zalgo, contrôles, forçages de
/// sens, balisage, gabarits, très long, vide, invisibles (PUN-23, CAT-85).
const TEXTES_HOSTILES: &[&str] = &[
    "Léa",
    "👩‍👩‍👧‍👦",
    "Z̸̢̛͎a̷̡͙l̶̨̛g̵͚o̴̧͝",
    "\u{202E}tnemelc\u{202C}",
    "a\u{200B}b\u{2060}c",
    "<script>alert(1)</script>",
    "{joueur}",
    "",
    "   ",
    "\u{0007}\u{001B}[31m",
    "ßßßßßßßßßßßß",
    "ﬃﬃﬃﬃ",
    "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW",
    "e\u{301}e\u{301}e\u{301}",
    "1\u{FE0F}\u{20E3}🇫🇷",
    "\u{FFF9}\u{FFFA}\u{FFFB}",
];

/// Un joueur simulé. Il ne voit que son regard et sa vue (D29).
pub struct Bot<'b, J: Jeu> {
    pub(crate) regard: Regard,
    pub(crate) style: Style,
    pub(crate) hasard: Hasard,
    pub(crate) depuis_vue: Duree,
    pub(crate) coups: Vec<Coup>,
    pub(crate) _jeu: core::marker::PhantomData<&'b J>,
}

fn encoder_un<T: Message>(v: &T) -> Vec<u8> {
    let mut o = Vec::new();
    if jeu_abi::encoder(v, &mut o).is_err() {
        o.clear();
    }
    o
}

impl<J: Jeu> Bot<'_, J> {
    /// Son regard, pour ouvrir les secrets qu'il a le droit de voir.
    pub fn regard(&self) -> &Regard {
        &self.regard
    }

    /// Comment il joue.
    pub fn style(&self) -> Style {
        self.style
    }

    /// Son hasard, le sien et non celui du jeu.
    pub fn hasard(&mut self) -> &mut Hasard {
        &mut self.hasard
    }

    /// Depuis que sa vue a changé.
    pub fn depuis_vue(&self) -> Duree {
        self.depuis_vue
    }

    /// Un texte du corpus hostile du commun.
    pub fn texte_hostile(&mut self) -> &'static str {
        self.hasard.choisir(TEXTES_HOSTILES).copied().unwrap_or("")
    }

    /// Envoie un geste.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "l'API de § 2.13 prend le coup par valeur, comme un jeu l'écrit"
    )]
    pub fn geste(&mut self, g: J::GesteTelephone) {
        self.coups.push(Coup::Geste(encoder_un(&g)));
    }

    /// Change son état tenu.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "l'API de § 2.13 prend le coup par valeur, comme un jeu l'écrit"
    )]
    pub fn tenir(&mut self, t: J::TenuTelephone) {
        self.coups.push(Coup::Tenu(encoder_un(&t)));
    }

    /// Une action de l'écran.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "l'API de § 2.13 prend le coup par valeur, comme un jeu l'écrit"
    )]
    pub fn action(&mut self, a: J::ActionEcran) {
        self.coups.push(Coup::Action(encoder_un(&a)));
    }

    /// Une touche de l'écran.
    pub fn touche(&mut self, t: Touche) {
        self.coups.push(Coup::Touche(t));
    }

    /// Un composant de l'écran.
    pub fn composant(&mut self, c: ComposantBot) {
        self.coups.push(Coup::Composant(c));
    }

    /// Rien.
    pub fn rien(&mut self) {
        self.coups.push(Coup::Rien);
    }
}

/// Une coupure transparente : plus courte que `moins_de`, sur un appareil pour lequel
/// l'hypothèse a tenu pendant toute la coupure, elle ne change pas `resume` (PUN-73,
/// SQ-28).
pub struct Coupure<J> {
    /// La durée sous laquelle la coupure ne compte pas.
    pub moins_de: Duree,
    /// L'hypothèse. L'épreuve (E11) la lit dans la soirée **sans** coupure, à chaque
    /// pas qu'elle échantillonne de la coupure, de son début à la première lecture
    /// après sa fin : elle doit y tenir partout. Une condition qui ne se sait qu'à la
    /// fin, comme « personne n'a poussé ce joueur pendant la coupure », s'écrit donc
    /// comme une condition de chaque pas : « personne ne le touche » (besoin 8 des
    /// jeux, `conception/BESOINS-DES-JEUX.md`).
    pub hypothese: fn(&J, &Autorite, Appareil) -> Oui,
}

/// Les propriétés que le jeu promet, et que l'épreuve vérifie (E10, E11).
pub struct Proprietes<J: Jeu> {
    /// Une coupure courte ne change pas `resume`.
    pub coupure_transparente: Applicable<Coupure<J>>,
    /// Renuméroter les appareils ne change pas `resume`, ni les vues à renumérotation
    /// près (CAT-27).
    pub permutation: Applicable<()>,
}

/// Ce que reçoit `pire_cas` pour construire un état : des appareils et un instant.
pub struct Construction<'c> {
    pub(crate) maintenant: Maintenant<'c>,
    pub(crate) appareils: u16,
}

impl Construction<'_> {
    /// L'instant de la construction, d'où naissent ses échéances.
    pub fn maintenant(&self) -> Maintenant<'_> {
        self.maintenant.preter()
    }

    /// Des appareils distincts, autant que la déclaration en permet : ils ne valent que
    /// pour ce pire cas.
    pub fn appareils(&self) -> impl Iterator<Item = Appareil> + use<'_> {
        (1..=u64::from(self.appareils))
            .map(|n| Appareil::du_fil(n.wrapping_mul(0x9e37_79b9_7f4a_7c15)))
    }

    /// Des identités de demande distinctes, pour un pire cas qui en montre (un export
    /// en cours) : la `n`-ième est prise tout en haut de l'espace des identités, que
    /// `Sorties::id` n'atteint pas en une soirée. Elles ne valent que pour ce pire cas
    /// (besoin 6 des jeux, `conception/BESOINS-DES-JEUX.md`).
    pub fn demande(&self, n: u16) -> IdDemande {
        IdDemande::du_fil(u64::MAX.saturating_sub(u64::from(n)))
    }
}

/// Ce que le jeu donne à l'épreuve. `jeu::module!` ne compile pas sans.
pub trait Epreuve: Jeu {
    /// Ce qui résume l'histoire définitive.
    type Resume: Message + PartialEq;
    /// Les écrans du jeu, pour la galerie des pires cas.
    type Cas: Variantes + Message + Copy + Eq;

    /// Le résumé de l'histoire définitive.
    fn resume(&self, a: &Autorite) -> Self::Resume;
    /// Les invariants, à chaque pas définitif (E4).
    fn invariants(&self, a: &Autorite, v: &mut Verif);
    /// Une transition entre deux résumés, à chaque entrée acceptée.
    fn transition(avant: &Self::Resume, apres: &Self::Resume, v: &mut Verif);
    /// Un téléphone simulé : il ne voit que ce que voit un humain qui tient ce téléphone
    /// et regarde la télé. `jeu` est la projection de l'état pour son regard.
    fn bot_telephone(jeu: &Self, vue: &Montre<Self::VueTelephone>, b: &mut Bot<'_, Self>);
    /// L'organisateur simulé, à l'écran. `jeu` est la projection pour l'écran.
    fn bot_ecran(jeu: &Self, b: &mut Bot<'_, Self>);
    /// L'écran que montre cet état.
    fn cas(&self) -> Self::Cas;
    /// Le pire cas de chaque écran, construit depuis les limites.
    fn pire_cas(cas: Self::Cas, c: &Construction<'_>) -> Self;
    /// Les propriétés promises.
    fn proprietes() -> Proprietes<Self>;
}
