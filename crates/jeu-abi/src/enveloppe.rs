//! L'enveloppe (§ 3.5) : la seule porte entre le moteur et le module.
//!
//! Chaque appel est un [`Appel`] encodé, chaque réponse un [`Retour`]. Les types du
//! jeu y voyagent en octets : le moteur les garde, les compare et les renvoie sans
//! les lire. L'ordre des variants est le format : un variant ajouté, retiré ou
//! déplacé change les octets, et le test `forme_figee` le voit (PUN-90). Un tel
//! changement monte [`ABI`].

use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::appareils::Presence;
use crate::declaration::{Accueil, ApplicableFil, DeclarationFil};
use crate::demandes::{DemandeFil, ReponseFil};
use crate::entrees::{CauseTenu, ComposantFil, Deplace, FinSaisie, Pointeur, Touche};
use crate::format::ErreurFil;
use crate::scene::{Cle, Effet, Reglages};
use crate::valeurs::{Manette, Oui, TexteCourt};

/// La version de l'enveloppe. Elle monte à chaque changement de la forme d'un
/// [`Appel`], d'un [`Retour`] ou d'une [`EntreeFil`] : moteur et module parlent la
/// même enveloppe, ou le moteur refuse le module avec les deux numéros (§ 3.8).
///
/// 2 : `Appel::Proprietes` et `Appel::Hypothese`, `Retour::Proprietes` et
/// `Retour::Hypothese`, pour les contrôles E10 et E11 de l'épreuve (§ 7.2). Les
/// variants d'avant gardent leurs octets.
///
/// 3 : [`ProprietesFil`] porte en plus `phases`, le nombre de variantes de `Phase`
/// (E3), et `invariants`, les noms que le jeu déclare exiger (E4). Seuls les octets
/// de `Retour::Proprietes` changent.
pub const ABI: u32 = 3;

/// Un appel du moteur au module.
///
/// Sa sorte ([`Appel::sorte`]) dit si le module peut écrire son état (§ 3.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appel<'a> {
    /// Lecture. Le premier appel, juste après l'instanciation.
    Bonjour,
    /// Lecture. La déclaration.
    Decrire,
    /// Écriture. Crée l'état.
    Creer {
        /// La graine de la partie, neuve à chaque soirée sauf rejeu voulu.
        graine: [u8; 32],
        /// Le pas de la création.
        pas: u64,
        /// L'heure murale de la création, en millisecondes.
        heure: u64,
        /// Le code de salle de la soirée, stable d'une partie à l'autre : le commun le
        /// donne au jeu dans `Creation` (REPONSES-PROVISOIRES.md, question 15).
        salle: TexteCourt,
        /// Les appareils déjà là : un `Vec<Presence>` encodé.
        presents: &'a [u8],
        /// Ce que la partie précédente a gardé, encodé par le jeu.
        garde: Option<&'a [u8]>,
    },
    /// Lecture. Ces octets d'appareil sont-ils bien formés ? Rien de malformé
    /// n'entre au journal.
    Valider {
        /// Ce qu'ils prétendent être.
        sorte: SorteOctets,
        /// Les octets.
        octets: &'a [u8],
    },
    /// Écriture. Une entrée.
    Entree {
        /// Son rang dans le journal de la partie.
        ordre: u64,
        /// Le pas où le commun l'applique.
        place: u64,
        /// Une [`EntreeFil`] encodée.
        entree: &'a [u8],
    },
    /// Lecture. Les vues de ces regards.
    Regarder {
        /// Les regards.
        regards: Vec<RegardFil>,
    },
    /// Lecture. Phase, réveil, attente, accueil, demandes.
    Etat {
        /// Les demandes que le moteur connaît déjà.
        connues: Vec<u64>,
    },
    /// Lecture. Les octets des ressources citées.
    Ressources {
        /// Leurs clés.
        cles: Vec<u64>,
    },
    /// Lecture. La projection pour un regard, par morceaux.
    Projeter {
        /// Le regard.
        regard: RegardFil,
        /// Le morceau voulu, à partir de 0.
        morceau: u32,
    },
    /// Écriture, sur un appareil qui prédit. L'état devient cette projection.
    Charger {
        /// La projection, entière.
        projection: &'a [u8],
    },
    /// Lecture. La forme canonique, secrets compris, par morceaux.
    Canonique {
        /// Le morceau voulu, à partir de 0.
        morceau: u32,
    },
    /// Écriture. L'état depuis une forme canonique de cette forme, ou de la
    /// version précédente.
    DepuisCanonique {
        /// L'empreinte des formes de ces octets.
        forme: u64,
        /// La forme canonique, entière.
        octets: &'a [u8],
    },
    /// Lecture. Ce qu'une nouvelle partie garde.
    Garder,
    /// Lecture, épreuve. Le résumé de l'histoire définitive.
    Resumer,
    /// Lecture, épreuve. Les invariants.
    Verifier,
    /// Lecture, épreuve. La transition entre deux résumés.
    Transition {
        /// Le résumé d'avant, encodé par le jeu.
        avant: &'a [u8],
        /// Le résumé d'après.
        apres: &'a [u8],
    },
    /// Lecture, épreuve. Ce que joue un bot.
    Bot {
        /// Qui il est.
        regard: RegardFil,
        /// Comment il joue.
        style: StyleFil,
        /// La graine de son hasard, le sien et non celui du jeu.
        graine: u64,
        /// Les pas depuis que sa vue a changé.
        depuis_vue: u64,
    },
    /// Écriture, épreuve. L'état devient ce pire cas.
    PireCas {
        /// L'indice du cas dans `Cas::TOUTES`.
        indice: u32,
    },
    /// Lecture. Octets vivants du tas.
    Memoire,
    /// Lecture, épreuve. Les propriétés que le jeu promet (E10, E11).
    Proprietes,
    /// Lecture, épreuve. L'hypothèse de la coupure transparente tient-elle, maintenant,
    /// pour cet appareil (E11, PUN-73) ?
    Hypothese {
        /// L'appareil.
        appareil: u64,
    },
}

/// Lecture ou écriture : ce qu'un appel a le droit de faire à l'état (§ 3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SorteAppel {
    /// L'état ne change pas ; les allocations vont au brouillon.
    Lecture,
    /// L'état peut changer ; les allocations vont au tas.
    Ecriture,
}

impl SorteAppel {
    /// Le paramètre `sorte` de l'export `soiree_boite` : 0 pour une lecture, 1 pour
    /// une écriture (§ 3.3).
    pub const fn code(self) -> i32 {
        match self {
            SorteAppel::Lecture => 0,
            SorteAppel::Ecriture => 1,
        }
    }

    /// La sorte de ce code, s'il en est un.
    pub const fn depuis_code(code: i32) -> Option<SorteAppel> {
        match code {
            0 => Some(SorteAppel::Lecture),
            1 => Some(SorteAppel::Ecriture),
            _ => None,
        }
    }
}

impl Appel<'_> {
    /// La sorte de cet appel : `Creer`, `Entree`, `Charger`, `DepuisCanonique` et
    /// `PireCas` écrivent ; tous les autres lisent (§ 3.4).
    pub const fn sorte(&self) -> SorteAppel {
        match self {
            Appel::Creer { .. }
            | Appel::Entree { .. }
            | Appel::Charger { .. }
            | Appel::DepuisCanonique { .. }
            | Appel::PireCas { .. } => SorteAppel::Ecriture,
            Appel::Bonjour
            | Appel::Decrire
            | Appel::Valider { .. }
            | Appel::Regarder { .. }
            | Appel::Etat { .. }
            | Appel::Ressources { .. }
            | Appel::Projeter { .. }
            | Appel::Canonique { .. }
            | Appel::Garder
            | Appel::Resumer
            | Appel::Verifier
            | Appel::Transition { .. }
            | Appel::Bot { .. }
            | Appel::Memoire
            | Appel::Proprietes
            | Appel::Hypothese { .. } => SorteAppel::Lecture,
        }
    }
}

/// Ce que des octets d'appareil prétendent être, pour `Appel::Valider`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SorteOctets {
    /// Une action de l'écran.
    ActionEcran,
    /// Un geste de téléphone.
    GesteTelephone,
    /// Un état tenu de téléphone.
    TenuTelephone,
}

/// La réponse du module à un appel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Retour<'a> {
    /// À `Bonjour`.
    Bonjour(Bonjour),
    /// À `Decrire`. En boîte : la déclaration est de loin le plus gros des retours.
    Decrit(alloc::boxed::Box<DeclarationFil>),
    /// À `Creer` et `Entree` : ce que le jeu a décidé, et son réveil.
    Issue(#[serde(borrow)] IssueFil<'a>),
    /// À `Valider`.
    Valide(#[serde(borrow)] Validation<'a>),
    /// À `Regarder` : une vue par regard, dans l'ordre demandé.
    Vues(#[serde(borrow)] Vec<VueFil<'a>>),
    /// À `Etat`.
    Etat(#[serde(borrow)] EtatFil<'a>),
    /// À `Ressources` : les octets de chaque clé connue.
    Ressources(#[serde(borrow)] Vec<(u64, &'a [u8])>),
    /// À `Projeter` et `Canonique` : un morceau, et le numéro du suivant s'il y en a un.
    Morceau {
        /// Les octets du morceau.
        octets: &'a [u8],
        /// Le morceau suivant, ou `None` pour le dernier.
        suivant: Option<u32>,
    },
    /// À `Charger` et `DepuisCanonique` : fait, sans rien à dire.
    Fait,
    /// À `Garder` et `Resumer` : des octets du jeu.
    Octets(&'a [u8]),
    /// À `Verifier` et `Transition` : chaque constat, par son nom.
    Verif(#[serde(borrow)] Vec<(&'a str, ConstatFil)>),
    /// À `Bot` : les coups du bot, dans l'ordre.
    Bot(#[serde(borrow)] Vec<CoupFil<'a>>),
    /// À `PireCas` : le cas de l'état construit, encodé par le jeu, pour vérifier
    /// que `cas(pire_cas(c)) = c` (E16).
    PireCas {
        /// Le cas, encodé par le jeu.
        cas: &'a [u8],
    },
    /// À `Memoire`.
    Memoire {
        /// Les octets vivants du tas.
        vivants: u64,
        /// La taille du tas.
        tas: u64,
    },
    /// La frontière a refusé l'appel lui-même : panne de l'outil.
    Illisible(ErreurFil),
    /// À `Proprietes`.
    Proprietes(ProprietesFil),
    /// À `Hypothese`.
    Hypothese(OuiFil),
}

/// Tous les combien l'épreuve lit l'hypothèse de la coupure transparente (E11), dans
/// la soirée sans coupure : une lecture tous ces pas, de chaque début de coupure à la
/// première lecture après sa fin. Un jeu dont l'hypothèse ne tient que si elle est lue
/// assez souvent s'y accorde à la compilation, au lieu de deviner la période de
/// l'épreuve (Sidequest, B10) :
///
/// ```
/// const _: () = assert!(jeu_abi::PERIODE_HYPOTHESE.en_pas() <= 30);
/// ```
///
/// Elle fait partie du contrat de cette version du commun : la changer se dit au
/// `CHANGELOG.md`. Elle n'entre pas dans l'enveloppe : aucun octet ne la porte.
pub const PERIODE_HYPOTHESE: crate::temps::Duree = crate::temps::Duree::pas(10);

/// Les propriétés qu'un jeu promet, que l'épreuve vérifie (E10, E11 ; § 2.13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProprietesFil {
    /// Une coupure plus courte que cette durée, sur un appareil pour lequel
    /// l'hypothèse (`Appel::Hypothese`) a tenu pendant toute la coupure, ne change pas
    /// le résumé (PUN-73, SQ-28).
    pub coupure_transparente: ApplicableFil<crate::temps::Duree>,
    /// Renuméroter les appareils ne change pas le résumé, ni les vues à renumérotation
    /// près (CAT-27).
    pub permutation: ApplicableFil<()>,
    /// Le nombre de variantes de `Phase` (`Phase::TOUTES`), rempli par la colle : E3
    /// vérifie que la partie les parcourt toutes, qu'un pire cas y mène ou non.
    pub phases: u16,
    /// Les noms de ce qu'exigent `invariants` et `transition`
    /// (`Epreuve::INVARIANTS`), dans l'ordre déclaré : E4 vérifie que chacun est
    /// évalué au moins une fois, et qu'aucun autre ne l'est (OUT-01).
    pub invariants: Vec<alloc::string::String>,
}

/// Ce que le module dit de lui-même, au premier appel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bonjour {
    /// La version de l'enveloppe compilée dans le module.
    pub abi: u32,
    /// L'empreinte des formes : enveloppe, état, et les types associés du jeu.
    pub formes: u64,
    /// La boîte d'entrée : adresse, capacité.
    pub boite: (u32, u32),
    /// Le brouillon des lectures : adresse, capacité.
    pub brouillon: (u32, u32),
    /// La zone de panique : adresse, capacité.
    pub panique: (u32, u32),
    /// La version de la crate `jeu` compilée dans le module.
    pub version_jeu: (u16, u16, u16),
}

/// Ce que le jeu a décidé d'une écriture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueFil<'a> {
    /// L'entrée est acceptée : ses sorties, et le réveil lu juste après.
    Acceptee {
        /// Les sorties, dans l'ordre où le jeu les a émises.
        sorties: Vec<SortieFil>,
        /// Quand le jeu veut recevoir le temps.
        reveil: ReveilFil,
    },
    /// L'entrée est refusée : l'avis, encodé par le jeu, va au seul appareil du geste.
    Refusee {
        /// L'avis.
        avis: &'a [u8],
    },
    /// L'entrée est ignorée : personne n'est prévenu.
    Ignoree,
    /// Un secret manque : sur un appareil qui prédit, ce geste attend la
    /// confirmation ; sur la partie qui fait foi, c'est une faute du jeu.
    Attente,
}

/// Une décision ponctuelle d'une étape, exécutée au pas définitif seulement (D10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortieFil {
    /// Oublier un appareil parti : il reviendra comme un neuf (PUN-15, SQ-76).
    Oublier(u64),
    /// Retirer un appareil présent : il peut revenir comme un neuf.
    Retirer(u64),
    /// Une nouvelle partie : une instance neuve, avec cette graine (CAT-73, CAT-75).
    NouvellePartie {
        /// La graine de la nouvelle partie.
        graine: [u8; 32],
    },
}

/// Quand le jeu veut recevoir le temps, sur le fil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReveilFil {
    /// À chaque pas.
    ChaquePas,
    /// Un seul pas, à ce pas, ou au premier pas après s'il est passé.
    A(u64),
    /// Jamais : le jeu n'avance que sur des entrées.
    Jamais,
}

/// Ce que le jeu dit d'octets d'appareil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Validation<'a> {
    /// Bien formés ; pour un tenu, ce que le moteur doit en savoir.
    Valide {
        /// Pour un tenu seulement.
        #[serde(borrow)]
        tenu: Option<TenuFil<'a>>,
    },
    /// Malformés : ils n'entrent pas au journal.
    Malforme(ErreurFil),
}

/// Ce que le moteur doit savoir d'un tenu sans connaître son type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenuFil<'a> {
    /// Sa clé, encodée : un tenu par clé et par appareil.
    pub cle: &'a [u8],
    /// Sa valeur relâchée, encodée, s'il se relâche seul.
    #[serde(borrow)]
    pub relache: Option<&'a [u8]>,
    /// Cette valeur est-elle déjà relâchée ?
    pub est_relache: OuiFil,
}

/// La vue d'un regard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VueFil<'a> {
    /// Le regard.
    pub regard: RegardFil,
    /// Une [`crate::SceneFil`] encodée pour l'écran, une [`MontreFil`] pour un téléphone.
    pub octets: &'a [u8],
}

/// Ce qu'un téléphone reçoit, sur le fil : sa vue encodée par le jeu, ses effets,
/// ses réglages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MontreFil<'a> {
    /// La vue du téléphone, encodée par le jeu.
    pub vue: &'a [u8],
    /// Ses effets.
    pub effets: Vec<Effet>,
    /// Ses réglages.
    pub reglages: Reglages,
}

/// Ce qui se lit dans l'état, après chaque entrée acceptée (D9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EtatFil<'a> {
    /// La phase, encodée par le jeu.
    pub phase: &'a [u8],
    /// L'indice de la phase dans `Phase::TOUTES`.
    pub phase_indice: u16,
    /// La phase est-elle terminale ?
    pub terminale: OuiFil,
    /// Quand le jeu veut recevoir le temps.
    pub reveil: ReveilFil,
    /// Qui la partie attend (CAT-78).
    #[serde(borrow)]
    pub attente: AttenteFil<'a>,
    /// Si la partie accueille.
    pub accueil: Accueil,
    /// L'indice du cas dans `Cas::TOUTES`.
    pub cas_indice: u16,
    /// Toutes les identités de demandes listées.
    pub demandes: Vec<u64>,
    /// Celles que `connues` n'avait pas, avec leur contenu.
    #[serde(borrow)]
    pub nouvelles: Vec<(u64, DemandeFil<'a>)>,
}

/// Qui la partie attend, sur le fil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttenteFil<'a> {
    /// La partie avancera seule à ce pas.
    pub echeance: Option<u64>,
    /// Rien n'avance seul, par nature, et pourquoi.
    #[serde(borrow)]
    pub libre: Option<&'a str>,
    /// Les appareils qui peuvent faire avancer la partie, avec des gestes encodés
    /// dont l'un doit être accepté.
    #[serde(borrow)]
    pub joueurs: Vec<(u64, Vec<&'a [u8]>)>,
    /// Les actions de l'écran, encodées, dont l'une doit être acceptée.
    #[serde(borrow)]
    pub ecran: Vec<&'a [u8]>,
}

/// Qui regarde : l'écran, ou un appareil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RegardFil {
    /// L'écran de l'ordinateur de la soirée.
    Ecran,
    /// Un appareil, par son numéro.
    Appareil(u64),
}

/// Comment joue un bot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StyleFil {
    /// Comme un joueur attentif.
    Sage,
    /// Comme un joueur qui cherche à casser.
    Hostile,
    /// Comme un joueur qui tarde.
    Lent,
}

/// Le résultat d'un invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConstatFil {
    /// Tenu.
    Tenu,
    /// Violé.
    Viole,
}

/// Le oui ou non de l'enveloppe : le même que celui du vocabulaire.
pub type OuiFil = Oui;

/// Un coup d'un bot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoupFil<'a> {
    /// Un geste de téléphone, encodé.
    Geste(&'a [u8]),
    /// Un état tenu de téléphone, encodé.
    Tenu(&'a [u8]),
    /// Une action de l'écran, encodée.
    Action(&'a [u8]),
    /// Une touche de l'écran.
    Touche(Touche),
    /// Un composant de l'écran.
    Composant(#[serde(borrow)] ComposantBotFil<'a>),
    /// Rien.
    Rien,
}

/// Ce qu'un bot fait d'un composant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComposantBotFil<'a> {
    /// Il saisit un texte.
    Saisie {
        /// Le champ.
        champ: Cle,
        /// Le texte.
        texte: &'a str,
        /// Ce qui termine la saisie.
        fin: FinSaisie,
    },
    /// Il déplace un objet.
    Deplace(Deplace),
    /// Il apporte des fichiers.
    Fichiers(#[serde(borrow)] Vec<FichierBotFil<'a>>),
}

/// Un fichier qu'apporte un bot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FichierBotFil<'a> {
    /// Son nom.
    pub nom: &'a str,
    /// Son type.
    pub genre: &'a str,
    /// Ses octets.
    pub octets: &'a [u8],
}

/// Une entrée, sur le fil : ce que porte `Appel::Entree`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntreeFil<'a> {
    /// Le temps : le pas en cours, livré seulement si le réveil le demande.
    Temps {
        /// Les pas jetés après un gel, au-delà du rattrapage ; 0 d'habitude.
        saut: u64,
        /// L'heure murale, en millisecondes.
        heure: u64,
    },
    /// Une entrée de l'écran de l'ordinateur de la soirée.
    Ecran {
        /// Ce qui arrive.
        #[serde(borrow)]
        quoi: EcranFil<'a>,
        /// Quand.
        quand: QuandFil,
    },
    /// Un événement d'un téléphone, compté exactement une fois.
    Geste {
        /// L'appareil.
        de: u64,
        /// Le geste, encodé par le jeu.
        octets: &'a [u8],
        /// Quand.
        quand: QuandFil,
    },
    /// Le changement d'un état tenu d'un téléphone : le dernier gagne.
    Tenu {
        /// L'appareil.
        de: u64,
        /// La valeur, encodée par le jeu.
        octets: &'a [u8],
        /// Pourquoi elle change.
        cause: CauseTenu,
        /// Quand.
        quand: QuandFil,
    },
    /// Le changement de l'état d'une manette.
    Manette {
        /// L'appareil.
        de: u64,
        /// Son état.
        etat: Manette,
        /// Pourquoi il change.
        cause: CauseTenu,
        /// Quand.
        quand: QuandFil,
    },
    /// Un appareil arrive, part, revient ou change.
    Presence(Presence),
    /// La réponse à une demande encore listée.
    Reponse {
        /// La demande.
        id: u64,
        /// Sa réponse.
        #[serde(borrow)]
        reponse: ReponseFil<'a>,
    },
}

/// Quand et sur quoi une entrée a été faite, sur le fil. Le pas où elle est placée
/// est celui de l'`Appel::Entree` qui la porte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuandFil {
    /// Le pas estimé du geste, borné par le commun (§ 4.3).
    pub fait: u64,
    /// La fraction de ce pas, en 1/65 536.
    pub fraction: u16,
    /// Le pas où la partie qui fait foi l'a reçu.
    pub recu: u64,
    /// Le pas de la dernière vue que l'appareil avait reçue.
    pub vu: u64,
    /// L'identité du geste.
    pub id: u64,
}

/// Ce qui arrive de l'écran, sur le fil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EcranFil<'a> {
    /// Une action que la scène portait, encodée par le jeu.
    Action(&'a [u8]),
    /// Une touche brute.
    Touche(Touche),
    /// Un pointeur brut.
    Pointeur(Pointeur),
    /// Le résultat d'un composant.
    Composant(#[serde(borrow)] ComposantFil<'a>),
}
